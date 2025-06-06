use super::*;
use kolmogorov::*;

#[derive(Clone, Copy, Debug)]
pub struct CondPolyLang;

type Comparison = Sum;
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Cond {
    eqzs: Vec<Comparison>,
    poss: Vec<Comparison>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Program {
    cases: Vec<(Cond, Sum)>,
    default: Sum,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CondPolySems {
    Func(Identifier, Box<CondPolySems>),
    Case(Cond),
    Poly(Sum),
    Prog(Program),
    Appl(Identifier, Vec<CondPolySems>), // For in-progress expressions
}

impl Language for CondPolyLang {
    type Semantics = CondPolySems;

    const SMALL_SIZE: usize = 10;
    const LARGE_SIZE: usize = 20;

    fn context(&self) -> Context {
        let plus = builtin!(
            Poly => Poly => Poly
            |x, y| => Term::val(x.get::<i32>().wrapping_add(y.get::<i32>()))
        );

        let sub = builtin!(
            Poly => Poly => Poly
            |x, y| => Term::val(x.get::<i32>().wrapping_sub(y.get::<i32>()))
        );

        let mult = builtin!(
            Poly => Poly => Poly
            |x, y| => Term::val(x.get::<i32>().wrapping_mul(y.get::<i32>()))
        );

        let one = builtin!(
            Poly
            | | => Term::val(1i32)
        );

        let zero = builtin!(
            Poly
            | | => Term::val(0i32)
        );

        let case = builtin!(
            Cond => Poly => Cases => Cases
            |c| => if c.get::<bool>() {
                term!(p q -> p)
            } else {
                term!(p q -> q)
            }
        );

        let orelse = builtin!(
            Poly => Cases
            |p| => Term::val(p.get::<i32>())
        );

        let eval = builtin!(
            Cases => N
            |c| => Term::val(c.get::<i32>())
        );

        let eqz = builtin!(
            Poly => Cond
            |p| => Term::val(p.get::<i32>() == 0)
        );

        let pos = builtin!(
            Poly => Cond
            |p| => Term::val(p.get::<i32>() > 0)
        );

        let and = builtin!(
            Cond => Cond => Cond
            |a, b| => Term::val(a.get::<bool>() && b.get::<bool>())
        );

        let def = builtin!(
            Poly => (Poly => N) => N
            |p, f| => term!([f] [p])
        );

        Context::new([
            ("(+)".into(), plus),
            ("(-)".into(), sub),
            ("(*)".into(), mult),
            ("'1'".into(), one),
            ("'0'".into(), zero),
            ("case".into(), case),
            ("eval".into(), eval),
            ("orelse".into(), orelse),
            ("eqz".into(), eqz),
            ("pos".into(), pos),
            ("and".into(), and),
            ("def".into(), def),
        ])
    }

    fn sval(&self, _: &Value, _ty: &Type) -> Analysis<Self> {
        unimplemented!()
    }

    fn svar(&self, v: Identifier, _ty: &Type) -> Analysis<Self> {
        use Analysis::*;
        use CondPolySems::*;

        let names = [
            "(+)", "(-)", "(*)", "'1'", "'0'", "case", "eval", "orelse", "eqz",
            "pos", "and", "def",
        ];

        match v.as_str() {
            "'1'" => Canonical(Poly(Sum::from(1))),
            "'0'" => Canonical(Poly(Sum::from(0))),
            s if names.contains(&s) => Canonical(Appl(v, vec![])),
            _ => Canonical(Poly(Sum::from(v))),
        }
    }

    fn slam(
        &self,
        ident: Identifier,
        body: Analysis<Self>,
        _ty: &Type,
    ) -> Analysis<Self> {
        use Analysis::*;
        use CondPolySems::*;

        let Canonical(body) = body else {
            unreachable!()
        };

        Canonical(Func(ident, body.into()))
    }

    fn sapp(
        &self,
        fun: Analysis<Self>,
        arg: Analysis<Self>,
        _ty: &Type,
    ) -> Analysis<Self> {
        use Analysis::*;
        use CondPolySems::*;

        let (Canonical(fun), Canonical(arg)) = (fun, arg) else {
            unreachable!()
        };

        match fun {
            Appl(v, mut args)
                if args.len() == 1
                    && ["(+)", "(-)", "(*)"].contains(&v.as_str()) =>
            {
                let (Poly(a), Poly(b)) = (args.remove(0), arg) else {
                    unreachable!()
                };

                let mut poly = match v.as_str() {
                    "(+)" => a.add(&b),
                    "(-)" => a.add(&b.mul(&Sum::from(-1))),
                    "(*)" => a.mul(&b),
                    _ => unreachable!(),
                };

                poly.normalize();
                Canonical(Poly(poly))
            }
            Appl(v, mut args)
                if ["eqz", "pos"].contains(&v.as_str()) && args.len() == 1 =>
            {
                let Poly(p) = args.remove(0) else {
                    unreachable!()
                };

                let mut eqzs = vec![];
                let mut poss = vec![];

                let kind = match v.as_str() {
                    "eqz" => &mut eqzs,
                    "pos" => &mut poss,
                    _ => unreachable!(),
                };

                kind.push(p);

                Canonical(Case(Cond { eqzs, poss }))
            }
            Appl(v, args) if v.as_str() == "eval" => {
                assert!(args.is_empty());

                Canonical(arg)
            }
            Appl(v, args) if v.as_str() == "orelse" => {
                assert!(args.is_empty());
                let Poly(p) = arg else { unreachable!() };

                Canonical(Prog(Program {
                    cases: vec![],
                    default: p,
                }))
            }
            Appl(v, mut args) if v.as_str() == "case" && args.len() == 2 => {
                let (Prog(Program { mut cases, default }), Poly(p), Case(c)) =
                    (arg, args.pop().unwrap(), args.pop().unwrap())
                else {
                    unreachable!()
                };

                cases.push((c, p));

                Canonical(Prog(Program { cases, default }))
            }
            Appl(v, mut args) if v.as_str() == "and" && args.len() == 1 => {
                let (
                    Case(Cond {
                        eqzs: mut es1,
                        poss: mut ps1,
                    }),
                    Case(Cond {
                        eqzs: es2,
                        poss: ps2,
                    }),
                ) = (arg, args.pop().unwrap())
                else {
                    unreachable!()
                };

                es1.extend_from_slice(&es2);
                ps1.extend_from_slice(&ps2);

                es1.sort();
                ps1.sort();

                Canonical(Case(Cond {
                    eqzs: es1,
                    poss: ps1,
                }))
            }
            Appl(v, mut args) if v.as_str() == "def" && args.len() == 1 => {
                let Func(ident, body) = arg else {
                    unreachable!()
                };

                let (
                    Prog(Program {
                        mut cases,
                        mut default,
                    }),
                    Poly(p),
                ) = (*body, args.pop().unwrap())
                else {
                    unreachable!()
                };

                for (case, poly) in &mut cases {
                    for eq in &mut case.eqzs {
                        *eq = eq.sub(ident, p.clone());
                    }
                    for ps in &mut case.poss {
                        *ps = ps.sub(ident, p.clone());
                    }
                    *poly = poly.sub(ident, p.clone());
                }

                default = default.sub(ident, p);

                Canonical(Prog(Program { cases, default }))
            }
            Appl(v, mut args) => {
                args.push(arg);
                Canonical(Appl(v, args))
            }
            _ => {
                unreachable!()
            }
        }
    }
}

impl std::fmt::Display for CondPolySems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CondPolySems::*;

        match self {
            Func(_, _) => {
                let mut next = self;
                while let Func(v, body) = next {
                    next = body;
                    write!(f, "{} ", v)?;
                }
                write!(f, "-> {}", next)
            }
            Case(cond) => {
                write!(f, "{}", cond)
            }
            Poly(sum) => write!(f, "{}", sum),
            Prog(program) => {
                for (cond, poly) in &program.cases {
                    write!(f, "({})=>{};", cond, poly)?;
                }
                write!(f, "else=>{}", program.default)
            }
            Appl(identifier, items) => {
                write!(f, "{}", identifier)?;
                for item in items {
                    write!(f, "({})", item)?
                }
                Ok(())
            }
        }
    }
}

impl std::fmt::Display for Cond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0={:?} && 0<{:?}", self.eqzs, self.poss)
    }
}
