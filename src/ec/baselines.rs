//! Baseline optimizers used to judge whether the genetic algorithm earns its
//! keep: random search, hill climbing with restarts, greedy coordinate
//! descent, exhaustive enumeration, and a uniform-strategy sweep (a cheap
//! domain heuristic).

use rand_chacha::ChaCha8Rng;

use super::{Oracle, Record};
use crate::strategy::{enumerate, space_size, Chromosome};

/// Outcome shared by all single-objective baselines.
pub struct SearchOutcome {
    pub best: Chromosome,
    pub best_fitness: f64,
    pub history: Vec<Record>,
}

/// Uniform-strategy sweep: evaluate every chromosome whose genes are all equal
/// to the same value, and keep the best. A cheap, domain-agnostic heuristic
/// (costs only `min cardinality` evaluations) that is a surprisingly strong
/// baseline when the optimum happens to be a near-uniform configuration.
pub fn uniform_sweep(space: &[usize], oracle: &mut dyn Oracle) -> SearchOutcome {
    let card = space.iter().copied().min().unwrap_or(1);
    let mut best: Option<(Chromosome, f64)> = None;
    for v in 0..card as u8 {
        let chrom = Chromosome::new(vec![v; space.len()]);
        let f = oracle.scalar(&chrom);
        match &best {
            Some((_, bf)) if *bf <= f => {}
            _ => best = Some((chrom, f)),
        }
    }
    let (best, best_fitness) = best.expect("non-empty search space");
    let history = vec![Record {
        evaluations: oracle.evaluations(),
        best: best_fitness,
        mean: f64::NAN,
        diversity: f64::NAN,
    }];
    SearchOutcome {
        best,
        best_fitness,
        history,
    }
}

fn push_record(history: &mut Vec<Record>, evals: usize, best: f64) {
    history.push(Record {
        evaluations: evals,
        best,
        mean: f64::NAN,
        diversity: f64::NAN,
    });
}

/// Uniform random sampling for a fixed evaluation budget.
pub fn random_search(
    space: &[usize],
    budget: usize,
    sample_every: usize,
    oracle: &mut dyn Oracle,
    rng: &mut ChaCha8Rng,
) -> SearchOutcome {
    let mut best = Chromosome::random(space, rng);
    let mut best_fitness = oracle.scalar(&best);
    let mut history = Vec::new();
    push_record(&mut history, oracle.evaluations(), best_fitness);

    while oracle.evaluations() < budget {
        let cand = Chromosome::random(space, rng);
        let f = oracle.scalar(&cand);
        if f < best_fitness {
            best_fitness = f;
            best = cand;
        }
        if oracle.evaluations().is_multiple_of(sample_every.max(1)) {
            push_record(&mut history, oracle.evaluations(), best_fitness);
        }
    }
    push_record(&mut history, oracle.evaluations(), best_fitness);
    SearchOutcome {
        best,
        best_fitness,
        history,
    }
}

/// Steepest-ascent hill climbing with random restarts until the budget is hit.
pub fn hill_climb(
    space: &[usize],
    budget: usize,
    oracle: &mut dyn Oracle,
    rng: &mut ChaCha8Rng,
) -> SearchOutcome {
    let mut global = Chromosome::random(space, rng);
    let mut global_fitness = oracle.scalar(&global);
    let mut history = Vec::new();
    push_record(&mut history, oracle.evaluations(), global_fitness);

    while oracle.evaluations() < budget {
        // One climb from a random start.
        let mut current = Chromosome::random(space, rng);
        let mut current_fitness = oracle.scalar(&current);
        loop {
            if oracle.evaluations() >= budget {
                break;
            }
            // Evaluate the full single-gene neighborhood, take the best move.
            let mut improved = false;
            let mut best_neighbor = current.clone();
            let mut best_neighbor_fit = current_fitness;
            for neighbor in current.neighbors(space) {
                let f = oracle.scalar(&neighbor);
                if f < best_neighbor_fit {
                    best_neighbor_fit = f;
                    best_neighbor = neighbor;
                    improved = true;
                }
                if oracle.evaluations() >= budget {
                    break;
                }
            }
            if improved {
                current = best_neighbor;
                current_fitness = best_neighbor_fit;
            } else {
                break; // local optimum
            }
        }
        if current_fitness < global_fitness {
            global_fitness = current_fitness;
            global = current;
        }
        push_record(&mut history, oracle.evaluations(), global_fitness);
    }
    SearchOutcome {
        best: global,
        best_fitness: global_fitness,
        history,
    }
}

/// Greedy coordinate descent: repeatedly sweep the genes, fixing each to its
/// locally best value given the others, until a full sweep yields no change.
/// This mirrors a deterministic "expert heuristic" baseline.
pub fn greedy(space: &[usize], oracle: &mut dyn Oracle, rng: &mut ChaCha8Rng) -> SearchOutcome {
    let mut current = Chromosome::random(space, rng);
    let mut current_fitness = oracle.scalar(&current);
    let mut history = Vec::new();
    push_record(&mut history, oracle.evaluations(), current_fitness);

    loop {
        let mut changed = false;
        for (i, &card) in space.iter().enumerate() {
            let mut best_val = current.genes[i];
            let mut best_fit = current_fitness;
            for v in 0..card as u8 {
                if v == current.genes[i] {
                    continue;
                }
                let mut trial = current.clone();
                trial.genes[i] = v;
                let f = oracle.scalar(&trial);
                if f < best_fit {
                    best_fit = f;
                    best_val = v;
                }
            }
            if best_val != current.genes[i] {
                current.genes[i] = best_val;
                current_fitness = best_fit;
                changed = true;
            }
        }
        push_record(&mut history, oracle.evaluations(), current_fitness);
        if !changed {
            break;
        }
    }
    SearchOutcome {
        best: current,
        best_fitness: current_fitness,
        history,
    }
}

/// Exhaustive enumeration of the entire search space. Returns `None` if the
/// space is larger than `limit` (and therefore intractable to enumerate).
pub fn exhaustive(space: &[usize], limit: u128, oracle: &mut dyn Oracle) -> Option<SearchOutcome> {
    if space_size(space) > limit {
        return None;
    }
    let mut best: Option<(Chromosome, f64)> = None;
    for chrom in enumerate(space) {
        let f = oracle.scalar(&chrom);
        match &best {
            Some((_, bf)) if *bf <= f => {}
            _ => best = Some((chrom, f)),
        }
    }
    let (best, best_fitness) = best.expect("non-empty search space");
    let history = vec![Record {
        evaluations: oracle.evaluations(),
        best: best_fitness,
        mean: f64::NAN,
        diversity: f64::NAN,
    }];
    Some(SearchOutcome {
        best,
        best_fitness,
        history,
    })
}
