// Third iteration of search implementation, which uses Boxed nodes
// Ideally will be simpler than the stack machine & as fast as the
// iterator enumerator, (while making caching & other optimizations
// easier to implement & maintain)

mod analysis;
mod cache;
mod node;
mod semantics;

pub use analysis::*;
pub use semantics::*;

use super::*;
pub use cache::*;
use node::*;

use std::rc::Rc;

pub fn search<'a, L: Language>(
    lang: &'a L,
    vars: VarsVec,
    targ: &Type,
    size: usize,
) -> Enumerator<'a, L> {
    search_with_cache(lang, vars, targ, size, Cache::new())
}

pub fn search_with_cache<'a, L: Language>(
    lang: &'a L,
    vars: VarsVec,
    targ: &Type,
    size: usize,
    cache: Cache<L>,
) -> Enumerator<'a, L> {
    let ctxt = lang.context();

    let mut vgen = ctxt.vgen();

    for (var, _) in &vars {
        vgen.retire(*var);
    }

    Enumerator {
        search_ctxt: SearchContext {
            lang,
            ctxt,
            vgen,
            args: vars,
            cache,
        },
        root: Node::All {
            targ: Rc::new(targ.clone()),
            size,
            state: None,
            phase: AllPhase::START,
            depth: None,
        },
    }
}

pub struct Enumerator<'a, L: Language> {
    search_ctxt: SearchContext<'a, L>,
    root: Node<L>,
}

impl<L:Language> Enumerator<'_, L> {
    pub fn cache(self) -> Cache<L> {
        self.search_ctxt.cache
    }
}

pub type VarDecl = (Identifier, Rc<Type>);
pub type VarsVec = Vec<VarDecl>;

struct SearchContext<'a, L: Language> {
    lang: &'a L,
    ctxt: Context,
    vgen: VarGen,
    // Variables from abstractions
    args: VarsVec,
    cache: Cache<L>,
}

impl<L: Language> SearchContext<'_, L> {
    fn contains_var_of_type(&self, ty: &Rc<Type>) -> bool {
        let args = self.args.iter().map(|(_, t)| t);
        let ctxt = self.ctxt.iter().map(|(_, b)| &b.ty);

        args.chain(ctxt).any(|v_ty| v_ty == ty)
    }

    fn vars_producing(&mut self, targ: &Rc<Type>) -> VarsVec {
        fn produces(ty: &Type, target: &Type) -> bool {
            let ret_ty_produces = match ty {
                Type::Fun(_, r) => produces(r, target),
                _ => false,
            };

            ret_ty_produces || target == ty
        }

        let var_produces = move |(v, ty): (Identifier, &Rc<Type>)| {
            if produces(ty, targ) {
                Some((v, ty.clone()))
            } else {
                None
            }
        };

        let vars = self
            .ctxt
            .iter()
            .map(|(&v, BuiltIn { ty, .. })| (v, ty))
            .chain(self.args.iter().map(|(v, t)| (*v, t)))
            .filter_map(var_produces)
            .collect();

        vars
    }
}

impl<L: Language> Iterator for Enumerator<'_, L> {
    type Item = (Term, Analysis<L>);

    fn next(&mut self) -> Option<Self::Item> {
        self.root
            .next(&mut self.search_ctxt)
            .map(|(t, a)| (t.deep_clone(), a))
    }
}
