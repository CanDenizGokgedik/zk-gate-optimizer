//! Strategy encoding and search-space utilities.
//!
//! A [`Chromosome`] is a fixed-length vector of small integers; gene `i`
//! selects one of `space[i]` mathematically-equivalent implementation
//! strategies for the `i`-th sub-computation of a circuit. The per-gene
//! cardinalities are reported by
//! [`CircuitFactory::strategy_space`](crate::circuits::CircuitFactory::strategy_space)
//! as a [`StrategySpace`].

use rand::Rng;
use rand_chacha::ChaCha8Rng;

/// Per-gene cardinality of the search space (`space[i]` = number of valid
/// values for gene `i`).
pub type StrategySpace = Vec<usize>;

/// A point in the strategy search space.
///
/// Genes are stored as `u8` because every strategy axis in this project has
/// fewer than 256 alternatives. Two chromosomes are equal iff their genes are
/// identical, which makes [`Chromosome`] usable as a memoization key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chromosome {
    pub genes: Vec<u8>,
}

impl Chromosome {
    /// Wrap a raw gene vector.
    pub fn new(genes: Vec<u8>) -> Self {
        Self { genes }
    }

    /// Sample a uniformly random valid chromosome for `space`.
    pub fn random(space: &[usize], rng: &mut ChaCha8Rng) -> Self {
        let genes = space
            .iter()
            .map(|&card| rng.gen_range(0..card as u8))
            .collect();
        Self { genes }
    }

    /// Number of genes.
    pub fn len(&self) -> usize {
        self.genes.len()
    }

    /// Whether the chromosome has no genes.
    pub fn is_empty(&self) -> bool {
        self.genes.is_empty()
    }

    /// All chromosomes that differ from `self` in exactly one gene.
    ///
    /// Used as the neighborhood for hill climbing.
    pub fn neighbors(&self, space: &[usize]) -> Vec<Chromosome> {
        let mut out = Vec::new();
        for (i, &card) in space.iter().enumerate() {
            for v in 0..card as u8 {
                if v != self.genes[i] {
                    let mut g = self.genes.clone();
                    g[i] = v;
                    out.push(Chromosome::new(g));
                }
            }
        }
        out
    }

    /// Number of genes in which two chromosomes differ (Hamming distance).
    pub fn hamming(&self, other: &Chromosome) -> usize {
        self.genes
            .iter()
            .zip(&other.genes)
            .filter(|(a, b)| a != b)
            .count()
    }
}

/// Total number of points in the search space, as a 128-bit value so we can
/// detect spaces that are too large to enumerate without overflow.
pub fn space_size(space: &[usize]) -> u128 {
    space.iter().map(|&c| c as u128).product()
}

/// Exhaustively enumerate every chromosome in `space`.
///
/// The caller is responsible for checking [`space_size`] first; this is only
/// intended for small instances used as a ground-truth oracle.
pub fn enumerate(space: &[usize]) -> Vec<Chromosome> {
    let mut out = Vec::new();
    if space.contains(&0) {
        return out;
    }
    let mut genes = vec![0u8; space.len()];
    loop {
        out.push(Chromosome::new(genes.clone()));
        // Odometer increment with per-digit base `space[i]`.
        let mut i = 0;
        loop {
            if i == space.len() {
                return out;
            }
            genes[i] += 1;
            if (genes[i] as usize) < space[i] {
                break;
            }
            genes[i] = 0;
            i += 1;
        }
    }
}

/// Average pairwise normalized Hamming distance of a population, in `[0, 1]`.
///
/// This is the diversity metric tracked over generations: a value near `0`
/// signals premature convergence (the population has collapsed onto one point).
pub fn population_diversity(pop: &[Chromosome]) -> f64 {
    if pop.len() < 2 {
        return 0.0;
    }
    let n_genes = pop[0].len().max(1);
    let mut sum = 0.0;
    let mut pairs = 0u64;
    for i in 0..pop.len() {
        for j in (i + 1)..pop.len() {
            sum += pop[i].hamming(&pop[j]) as f64 / n_genes as f64;
            pairs += 1;
        }
    }
    sum / pairs as f64
}
