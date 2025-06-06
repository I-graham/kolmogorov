use kolmogorov::{metro::metropolis, *};

mod languages;
use languages::*;

fn main() {
    let lang = Opaque;

    let examples: Vec<_> =
        (0i32..10).map(|n| (n, 4 * n * n * n + n * n)).collect();

    let lang_ctxt = lang.context();

    const TUNING_PARAM: f64 = 0.5;

    let scorer = |t: &Term| {
        let max_correct = examples.len() as f64;

        let mut num_correct = max_correct;
        for (x, y) in examples.iter().copied() {
            let program = term! {
                [t] [:x]
            };

            let evaled = lang_ctxt.evaluate(&program);

            let output = evaled.get::<i32>();

            if output != y {
                num_correct -= 1.;
            }
        }

        if num_correct == max_correct {
            return None;
        }

        Some((TUNING_PARAM * num_correct).exp())
    };

    let start = term!(n -> n);

    let ty = ty!(N => N);

    let iterations = 50_000;

    let (_, metropolis_search, _) =
        metropolis(&lang, &start, &ty, scorer, iterations, Options::default());

    println!("Best Found: {}", metropolis_search);
}
