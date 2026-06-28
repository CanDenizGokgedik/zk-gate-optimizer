//! Single-objective generational genetic algorithm.
//!
//! Tournament selection, uniform crossover, point mutation, and elitism. The
//! crossover and mutation rates are configurable so that ablations
//! (crossover-only, mutation-only) reduce to setting one rate to zero.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::operators::{point_mutation, uniform_crossover};
use super::{Oracle, Record};
use crate::strategy::{population_diversity, Chromosome};

/// Genetic-algorithm hyperparameters.
#[derive(Clone, Debug)]
pub struct GaConfig {
    pub pop_size: usize,
    pub generations: usize,
    pub tournament_k: usize,
    pub crossover_rate: f64,
    pub mutation_rate: f64,
    pub elitism: usize,
}

impl Default for GaConfig {
    fn default() -> Self {
        Self {
            pop_size: 60,
            generations: 80,
            tournament_k: 3,
            crossover_rate: 0.9,
            mutation_rate: 0.1,
            elitism: 2,
        }
    }
}

/// Result of a GA run.
pub struct GaOutcome {
    pub best: Chromosome,
    pub best_fitness: f64,
    pub history: Vec<Record>,
}

/// Index of the minimum-fitness individual.
fn argmin(fit: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..fit.len() {
        if fit[i] < fit[best] {
            best = i;
        }
    }
    best
}

/// Binary-/k-ary tournament selection: pick `k` random individuals, return the
/// index of the fittest.
fn tournament(fit: &[f64], k: usize, rng: &mut ChaCha8Rng) -> usize {
    let mut best = rng.gen_range(0..fit.len());
    for _ in 1..k {
        let challenger = rng.gen_range(0..fit.len());
        if fit[challenger] < fit[best] {
            best = challenger;
        }
    }
    best
}

fn record(history: &mut Vec<Record>, evals: usize, fit: &[f64], pop: &[Chromosome]) {
    let best = fit.iter().copied().fold(f64::INFINITY, f64::min);
    let mean = fit.iter().sum::<f64>() / fit.len() as f64;
    history.push(Record {
        evaluations: evals,
        best,
        mean,
        diversity: population_diversity(pop),
    });
}

/// Run the genetic algorithm.
pub fn run(
    space: &[usize],
    cfg: &GaConfig,
    oracle: &mut dyn Oracle,
    rng: &mut ChaCha8Rng,
) -> GaOutcome {
    let elitism = cfg.elitism.min(cfg.pop_size);

    let mut pop: Vec<Chromosome> = (0..cfg.pop_size)
        .map(|_| Chromosome::random(space, rng))
        .collect();
    let mut fit: Vec<f64> = pop.iter().map(|c| oracle.scalar(c)).collect();

    let mut best_idx = argmin(&fit);
    let mut best = pop[best_idx].clone();
    let mut best_fitness = fit[best_idx];

    let mut history = Vec::with_capacity(cfg.generations + 1);
    record(&mut history, oracle.evaluations(), &fit, &pop);

    for _ in 0..cfg.generations {
        // Elitism: carry over the fittest individuals unchanged.
        let mut order: Vec<usize> = (0..pop.len()).collect();
        order.sort_by(|&a, &b| fit[a].partial_cmp(&fit[b]).unwrap());

        let mut next: Vec<Chromosome> = order[..elitism].iter().map(|&i| pop[i].clone()).collect();

        while next.len() < cfg.pop_size {
            let p1 = tournament(&fit, cfg.tournament_k, rng);
            let p2 = tournament(&fit, cfg.tournament_k, rng);
            let (mut c1, mut c2) = if rng.gen::<f64>() < cfg.crossover_rate {
                uniform_crossover(&pop[p1], &pop[p2], rng)
            } else {
                (pop[p1].clone(), pop[p2].clone())
            };
            c1 = point_mutation(&c1, space, cfg.mutation_rate, rng);
            c2 = point_mutation(&c2, space, cfg.mutation_rate, rng);
            next.push(c1);
            if next.len() < cfg.pop_size {
                next.push(c2);
            }
        }

        pop = next;
        fit = pop.iter().map(|c| oracle.scalar(c)).collect();

        best_idx = argmin(&fit);
        if fit[best_idx] < best_fitness {
            best_fitness = fit[best_idx];
            best = pop[best_idx].clone();
        }
        record(&mut history, oracle.evaluations(), &fit, &pop);
    }

    GaOutcome {
        best,
        best_fitness,
        history,
    }
}
