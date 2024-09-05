use kolmogorov::*;

#[allow(dead_code)]
pub fn polynomials() -> Context {
	use NTerm::*;

	let plus = builtin!(
		N => N => N
		|x, y| => Num(x.int()?+y.int()?)
	);

	let mult = builtin!(
		N => N => N
		|x, y| => {
			if *x == NTerm::Num(0) || *y == NTerm::Num(0) {
				Num(0)
			} else {
				Num(x.int()?*y.int()?)
			}
		}
	);

	let zero = builtin!(
		N
		| | => Num(0)
	);

	let one = builtin!(
		N
		| | => Num(1)
	);

	context! { plus, mult, zero, one }
}

#[allow(dead_code)]
pub fn fib_ctx() -> Context {
	use NTerm::*;

	let lte = builtin!(
		N => N => N => N => N
		|a, b| => if a.int()? <= b.int()? {
			term!(a b -> a)
		} else {
			term!(a b -> b)
		}
	);

	let plus = builtin!(
		N => N => N
		|x, y| => Num(x.int()?+y.int()?)
	);

	let minus = builtin!(
		N => N => N
		|x, y| => Num(x.int()?-y.int()?)
	);

	let one = builtin!(
		N
		| | => Num(1)
	);

	let two = builtin!(
		N
		| | => Num(2)
	);

	context! { lte, plus, minus, one, two }
}

#[allow(dead_code)]
fn main() {
	panic!("This file is not intended to be executed directly.")
}
