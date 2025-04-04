use kolmogorov::*;

mod polynomials;
use polynomials::*;

fn main() {
	let lang = PolynomialLanguage;
	let ty = ty!(N => N);

	for n in 1.. {
		let start = std::time::Instant::now();

		let searcher = search::search(&lang, vec![], &ty, n);

		let count = searcher.count();

		println!(
			"There are {:>6} known-distinct programs of type {} and size {}.",
			count, ty, n
		);

		let end = std::time::Instant::now();

		println!("Time elapsed: {}s", end.duration_since(start).as_secs_f32());
	}
}
