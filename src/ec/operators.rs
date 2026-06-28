//! Variation operators for fixed-length strategy chromosomes.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use crate::strategy::Chromosome;

/// Uniform crossover: each gene is independently inherited from either parent.
///
/// Appropriate here because genes are independent categorical choices (no
/// positional/permutation semantics), so swapping individual genes never
/// produces an invalid chromosome.
pub fn uniform_crossover(
    a: &Chromosome,
    b: &Chromosome,
    rng: &mut ChaCha8Rng,
) -> (Chromosome, Chromosome) {
    let mut c1 = a.genes.clone();
    let mut c2 = b.genes.clone();
    for i in 0..a.len() {
        if rng.gen::<bool>() {
            c1[i] = b.genes[i];
            c2[i] = a.genes[i];
        }
    }
    (Chromosome::new(c1), Chromosome::new(c2))
}

/// Point mutation: each gene is, with probability `rate`, reassigned to a
/// *different* valid value for its position.
pub fn point_mutation(
    chrom: &Chromosome,
    space: &[usize],
    rate: f64,
    rng: &mut ChaCha8Rng,
) -> Chromosome {
    let mut genes = chrom.genes.clone();
    for i in 0..genes.len() {
        let card = space[i];
        if card > 1 && rng.gen::<f64>() < rate {
            // Draw a value in [0, card-1) and skip over the current one to
            // guarantee an actual change.
            let mut v = rng.gen_range(0..(card - 1) as u8);
            if v >= genes[i] {
                v += 1;
            }
            genes[i] = v;
        }
    }
    Chromosome::new(genes)
}
