use super::*;

use rustc_hash::FxHashMap as HashMap;
use statrs::distribution::Discrete;

// Probability of replacing a variable with another
const REPLACE_VAR: f64 = 0.5;
// Probability of replacing a small (non-variable) subterm with another of equal size
const REPLACE_SMALL: f64 = 0.40;
// Probability of replacing a larger subterm with anohter, potentially of different size
// This is much more computationally expensive and can erase a lot of progress, but also
// allows us to exit local minima (we must calculate g(x'|x) & g(x|x'), involving a census
// of terms we don't even use, so it should be much more unlikely than the others).
#[allow(unused)]
const REPLACE_LARGE: f64 = 1. - REPLACE_VAR - REPLACE_SMALL;

#[derive(Clone, Copy)]
pub struct Options {
    pub print_freq: Option<usize>, // How often we print out progress, if at all
}

impl Default for Options {
    fn default() -> Self {
        Self {
            print_freq: Some(100),
        }
    }
}

// If F returns None, we stop immediately
pub fn metropolis<F: FnMut(&Term) -> Option<f64>, L: Language>(
    lang: &L,
    start: &Term,
    ty: &Type,
    mut scorer: F,
    iterations: usize,
    options: Options,
) -> (usize, Term, Analysis<L>) {
    let mut i = 0;
    let mut candidate = start.clone();

    let mut best_candidate = start.clone();
    let mut best_score = 0.;
    // Technically, we should probably perform some analysis here.
    let mut best_analysis = Analysis::Unique;

    let Some(mut score) = scorer(start) else {
        return (i, candidate, best_analysis);
    };

    let mut cache = SizeCache::default();

    while i < iterations {
        i += 1;

        if let Some(freq) = options.print_freq {
            if i % freq == 0 {
                println!(
                    "Metropolis progress: {}/{}. Size {}",
                    i,
                    iterations,
                    candidate.size()
                );
            }
        }

        // g_ratio = g(x|x') / g(x'|x)
        let Some((proposal, analysis, g_ratio)) =
            mutate(lang, &candidate, ty, &mut cache)
        else {
            continue;
        };

        let Some(proposal_score) = scorer(&proposal) else {
            return (i, proposal, analysis);
        };

        if proposal_score > best_score {
            best_analysis = analysis;
            best_score = proposal_score;
            best_candidate = proposal.clone();
        }

        let score_ratio = proposal_score / score;

        let acceptance_prob = score_ratio * g_ratio;

        if with_probability(acceptance_prob) {
            candidate = proposal;
            score = proposal_score;
        }
    }

    (i, best_candidate, best_analysis)
}

// Mutates a &Term. Also returns g(x|x') / g(x'|x) [where x' is the proposal]
fn mutate<L: Language>(
    lang: &L,
    term: &Term,
    ty: &Type,
    cache: &mut SizeCache<L>,
) -> Option<(Term, Analysis<L>, f64)> {
    let ctxt = lang.context();

    use MutationTy::*;
    match MutationTy::choose_replacement_kind() {
        HVar => {
            let term_meta = annotate_term(term, &ctxt, ty);

            let (var_node, annotation, _) =
                random_subnode(term, &term_meta, 1, 1);

            let (_, replacement) = cache.sample(
                lang,
                annotation.decls,
                &annotation.ty,
                annotation.size,
            );

            let (new_var, var_analysis) = replacement.unwrap();

            let (candidate, analysis) = replace_subnode(
                lang,
                term,
                &term_meta,
                var_node,
                new_var,
                var_analysis,
            )?;

            Some((candidate, analysis, 1.))
        }
        Small => {
            let term_meta = annotate_term(term, &ctxt, ty);

            let (replacement_node, annotation, _) =
                random_subnode(term, &term_meta, 2, L::SMALL_SIZE);

            let (_, replacement) = cache.sample(
                lang,
                annotation.decls,
                &annotation.ty,
                annotation.size,
            );

            let (new_term, new_analysis) = replacement.unwrap();

            let (proposal, analysis) = replace_subnode(
                lang,
                term,
                &term_meta,
                replacement_node,
                new_term,
                new_analysis,
            )?;

            if !proposal.in_beta_normal_form() {
                return None;
            }

            Some((proposal, analysis, 1.))
        }
        Large => {
            use rand::distributions::Distribution;
            use statrs::distribution::Binomial;

            let term_meta = annotate_term(term, &ctxt, ty);

            let (replacement_node, annotation, subnode_count) =
                random_subnode(term, &term_meta, 2, L::LARGE_SIZE);

            if subnode_count == 0 {
                return None;
            }

            let ratio = annotation.size as f64 / L::LARGE_SIZE as f64;

            let size_distr = Binomial::new(ratio, L::LARGE_SIZE as u64).ok()?;
            let replacement_size: u64 =
                size_distr.sample(&mut rand::thread_rng());
            let replacement_size = replacement_size as usize;

            let (new_count, replacement) = cache.sample(
                lang,
                annotation.decls.clone(),
                &annotation.ty,
                replacement_size,
            );

            let (replacement, replacement_analysis) = replacement?;

            let (proposal, analysis) = replace_subnode(
                lang,
                term,
                &term_meta,
                replacement_node,
                replacement,
                replacement_analysis,
            )?;

            if !proposal.in_beta_normal_form() {
                return None;
            }

            let old_count = cache.query_count(
                lang,
                annotation.decls,
                &annotation.ty,
                annotation.size,
            );

            // g1 = g(x' | x)
            let g1 = g::<L>(
                subnode_count,
                replacement_size,
                annotation.size,
                new_count,
            );

            let prop_meta = annotate_term(&proposal, &ctxt, ty);

            let (_, _, subnode_count) =
                random_subnode(&proposal, &prop_meta, 2, L::LARGE_SIZE);

            if subnode_count == 0 {
                return None;
            }

            //g2 = g(x | x')
            let g2 = g::<L>(
                subnode_count,
                annotation.size,
                replacement_size,
                old_count,
            );

            Some((proposal, analysis, g2 / g1))
        }
    }
}

// g(x2 | x1)
fn g<L: Language>(
    x1_subnode_count: usize,
    delta_size: usize,
    replaced_size: usize,
    x2_num_replacement_terms: usize,
) -> f64 {
    use statrs::distribution::Binomial;

    let prob_subnode_selected = 1. / x1_subnode_count as f64;

    let ratio = replaced_size as f64 / L::LARGE_SIZE as f64;

    let size_distr = Binomial::new(ratio, L::LARGE_SIZE as u64).unwrap();

    let prob_size_selected = size_distr.pmf(delta_size as u64);

    let prob_replacement_generated = 1. / x2_num_replacement_terms as f64;

    prob_subnode_selected * prob_size_selected * prob_replacement_generated
}

pub fn replace_subnode<L: Language>(
    lang: &L,
    dest: &Term,
    dest_meta: &Metadata,
    node_id: usize,
    src: Term,
    src_analysis: Analysis<L>,
) -> Option<(Term, Analysis<L>)> {
    fn helper<L: Language>(
        counter: &mut usize,
        lang: &L,
        dest: &Term,
        dest_meta: &Metadata,
        node_id: usize,
        src: Term,
        src_analysis: Analysis<L>,
    ) -> Option<(Term, Analysis<L>)> {
        *counter += 1;

        if *counter == node_id {
            return Some((src, src_analysis));
        }

        let ptr = dest as *const Term;
        let ty = &dest_meta[&ptr].ty;

        use Term::*;
        match dest {
            Ref(r) => helper(
                counter,
                lang,
                &(**r).borrow(),
                dest_meta,
                node_id,
                src,
                src_analysis,
            ),
            Lam(v, b) => {
                let (body, body_anal) = helper(
                    counter,
                    lang,
                    b,
                    dest_meta,
                    node_id,
                    src,
                    src_analysis,
                )?;
                if body_anal.malformed() {
                    return None;
                }
                Some((Lam(*v, body.into()), lang.slam(*v, body_anal, ty)))
            }
            App(l, r) => {
                let l = &*(**l).borrow();
                let (l, l_analysis) = helper(
                    counter,
                    lang,
                    l,
                    dest_meta,
                    node_id,
                    src.clone(),
                    src_analysis.clone(),
                )?;

                let r = &*(**r).borrow();
                let (r, r_analysis) = helper(
                    counter,
                    lang,
                    r,
                    dest_meta,
                    node_id,
                    src,
                    src_analysis,
                )?;

                if l_analysis.malformed() || r_analysis.malformed() {
                    return None;
                }

                Some((
                    App(l.into(), r.into()),
                    lang.sapp(l_analysis, r_analysis, ty),
                ))
            }
            Val(term_value) => Some((dest.clone(), lang.sval(term_value, ty))),
            Var(identifier) => Some((dest.clone(), lang.svar(*identifier, ty))),
        }
    }

    helper(&mut 0, lang, dest, dest_meta, node_id, src, src_analysis)
}

// Reservoir sampling, again.
// We return the index of the subnode (using pre-order numbering) & its size
// Returns (node_id, annotation, small_node_count)
pub fn random_subnode(
    term: &Term,
    meta: &Metadata,
    min_size: usize,
    max_size: usize,
) -> (usize, Annotation, usize) {
    let mut selected_id: usize = 0;
    let mut stack = vec![(term.clone(), term as *const Term)];
    let mut counter = 1;
    let mut small_counter = 0;

    let ptr = term as *const Term;
    let mut annotation = meta.get(&ptr).unwrap();

    while let Some((next, ptr)) = stack.pop() {
        let size = next.size();

        if (min_size..=max_size).contains(&size) {
            small_counter += 1;
            if with_probability(1. / small_counter as f64) {
                selected_id = counter;
                annotation = meta.get(&ptr).unwrap();
            }
        }

        use Term::*;
        match next {
            Ref(r) => stack.push((r.borrow().clone(), r.as_ptr())),
            Lam(_, b) => stack.push(((*b).clone(), b.as_ref() as *const Term)),
            App(l, r) => {
                stack.push((r.borrow().clone(), r.as_ptr()));
                stack.push((l.borrow().clone(), l.as_ptr()));
            }
            _ => (),
        }

        counter += 1;
    }

    (selected_id, annotation.clone(), small_counter)
}

#[allow(dead_code)]
fn show_node_id(term: &Term, id: usize) {
    fn helper(term: &Term, id: usize, counter: &mut usize) {
        *counter += 1;

        if *counter == id {
            println!(">>>{}", term);
            return;
        }

        use Term::*;
        match term {
            Ref(r) => helper(&r.borrow(), id, counter),
            Val(_) | Var(_) => (),
            Lam(_, term) => helper(term, id, counter),
            App(left, right) => {
                helper(&left.borrow(), id, counter);
                helper(&right.borrow(), id, counter);
            }
        }
    }

    helper(term, id, &mut 0)
}

#[derive(Clone, Debug)]
pub struct Annotation {
    size: usize,
    ty: Type,
    decls: VarsVec, // Variables in scope
}

type Metadata = HashMap<*const Term, Annotation>;

// Can fail if Term is not in beta-nf
fn annotate_term(term: &Term, ctxt: &Context, ty: &Type) -> Metadata {
    fn annotate(
        term: &Term,
        ctxt: &Context,
        ty: Option<&Type>,
        map: &mut Metadata,
        decls: &VarsVec,
    ) {
        let ptr = term as *const Term;

        if map.contains_key(&ptr) {
            return;
        }

        use Term::*;
        let annotation = match term {
            Ref(r) => {
                annotate(&r.borrow(), ctxt, ty, map, decls);

                let ptr = r.as_ptr() as *const Term;

                map.get(&ptr).unwrap().clone()
            }
            Val(_) => Annotation {
                size: 1,
                decls: decls.clone(),
                ty: ty.unwrap().clone(),
            },
            Var(v) => {
                if let Some((_, v_ty)) = decls.iter().find(|(s, _)| v == s) {
                    Annotation {
                        size: 1,
                        ty: (**v_ty).clone(),
                        decls: decls.clone(),
                    }
                } else if let Some(builtin) = ctxt.get(*v) {
                    Annotation {
                        size: 1,
                        ty: (*builtin.ty).clone(),
                        decls: decls.clone(),
                    }
                } else {
                    panic!("Undeclared variable")
                }
            }
            Lam(v, b) => {
                let ty = ty.unwrap().clone();

                let Type::Fun(arg, ret) = ty.clone() else {
                    unimplemented!()
                };

                let decls = decls.clone();

                let mut body_decls = decls.clone();
                body_decls.push((*v, arg.clone()));

                annotate(b, ctxt, Some(ret.as_ref()), map, &body_decls);

                Annotation {
                    size: term.size(),
                    ty,
                    decls,
                }
            }
            App(l, r) => {
                let f = l.as_ptr() as *const Term;

                annotate(&l.borrow(), ctxt, None, map, decls);

                let f_note = map.get(&f).unwrap().clone();

                let Type::Fun(arg, ret) = f_note.ty else {
                    unreachable!()
                };

                annotate(&r.borrow(), ctxt, Some(&*arg), map, decls);

                Annotation {
                    size: term.size(),
                    ty: (*ret).clone(),
                    decls: f_note.decls,
                }
            }
        };

        map.insert(ptr, annotation);
    }

    let mut map = Metadata::default();
    annotate(term, ctxt, Some(ty), &mut map, &vec![]);
    map
}

#[derive(Debug)]
enum MutationTy {
    HVar,
    Small,
    Large,
}

impl MutationTy {
    pub fn choose_replacement_kind() -> Self {
        let rand = random();

        if rand < REPLACE_VAR {
            Self::HVar
        } else if rand < REPLACE_VAR + REPLACE_SMALL {
            Self::Small
        } else {
            Self::Large
        }
    }
}

type CtxtCache<L> = HashMap<(Type, usize), CacheEntry<L>>;

struct SizeCache<L: Language> {
    map: HashMap<VarsVec, CtxtCache<L>>,
}

#[derive(Debug)]
enum CacheEntry<L: Language> {
    Explicit(Vec<(Term, Analysis<L>)>),
    Count(usize),
}

impl<L: Language> SizeCache<L> {
    const MAX_IN_MEM: usize = 32;

    pub fn sample(
        &mut self,
        lang: &L,
        mut decls: VarsVec,
        ty: &Type,
        size: usize,
    ) -> (usize, Option<(Term, Analysis<L>)>) {
        use CacheEntry::*;

        decls.sort();
        let query = (ty.clone(), size);

        let map = self.map.entry(decls.clone()).or_default();
        if let Some(cache_entry) = map.get(&query) {
            match cache_entry {
                Count(0) => {
                    return (0, None);
                }
                Explicit(explicit) => {
                    let len = explicit.len();
                    let id = (random() * len as f64) as usize;

                    return (explicit.len(), Some(explicit[id].clone()));
                }
                _ => (),
            }
        }

        let mut explicit = Vec::with_capacity(Self::MAX_IN_MEM);

        let mut search = search(lang, decls.clone(), ty, size);

        while explicit.len() < Self::MAX_IN_MEM {
            if let Some(next) = search.next() {
                explicit.push(next);
            } else if !explicit.is_empty() {
                let len = explicit.len();
                let id = (random() * len as f64) as usize;

                map.entry(query)
                    .or_insert_with(|| Explicit(explicit.clone()));

                return (len, Some(explicit.swap_remove(id)));
            } else {
                map.entry(query).or_insert(Count(0));
                return (0, None);
            }
        }

        let (rest_count, selected) = reservoir_sample(search);

        if selected.is_none() {
            let len = explicit.len();
            let id = (random() * len as f64) as usize;

            map.entry(query)
                .or_insert_with(|| Explicit(explicit.clone()));

            return (len, Some(explicit.swap_remove(id)));
        }

        let total_count = Self::MAX_IN_MEM + rest_count;
        let prob = rest_count as f64 / total_count as f64;

        map.entry(query).or_insert(Count(total_count));

        if with_probability(prob) {
            let len = explicit.len();
            let id = (random() * len as f64) as usize;
            return (total_count, Some(explicit.swap_remove(id)));
        }

        (total_count, selected)
    }

    pub fn query_count(
        &mut self,
        lang: &L,
        mut decls: VarsVec,
        ty: &Type,
        size: usize,
    ) -> usize {
        use CacheEntry::*;
        let query = (ty.clone(), size);

        decls.sort();

        let map = self.map.entry(decls.clone()).or_default();

        if let Some(entry) = map.get(&query) {
            return match entry {
                Count(count) => *count,
                Explicit(v) => v.len(),
            };
        }

        let count = search(lang, decls, ty, size).count();

        map.insert(query, Count(count));

        count
    }
}

impl<L: Language> Default for SizeCache<L> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}
