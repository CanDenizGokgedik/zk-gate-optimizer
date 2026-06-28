//! NSGA-II: multi-objective evolutionary optimization.
//!
//! Produces a Pareto front trading off the two static objectives
//! (`lde_size`, `num_gate_constraints`). Implements fast non-dominated
//! sorting, crowding-distance assignment, crowded-comparison tournament
//! selection, and (μ+λ) elitist replacement, following Deb et al. (2002).
//! Front quality over time is tracked with the 2-D hypervolume indicator.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::operators::{point_mutation, uniform_crossover};
use super::Oracle;
use crate::strategy::Chromosome;

/// NSGA-II hyperparameters.
#[derive(Clone, Debug)]
pub struct Nsga2Config {
    pub pop_size: usize,
    pub generations: usize,
    pub crossover_rate: f64,
    pub mutation_rate: f64,
}

impl Default for Nsga2Config {
    fn default() -> Self {
        Self {
            pop_size: 60,
            generations: 80,
            crossover_rate: 0.9,
            mutation_rate: 0.1,
        }
    }
}

/// A non-dominated solution.
#[derive(Clone, Debug)]
pub struct ParetoPoint {
    pub chrom: Chromosome,
    pub objectives: Vec<f64>,
}

/// Progress record: hypervolume of the current first front.
#[derive(Clone, Debug)]
pub struct HvRecord {
    pub evaluations: usize,
    pub hypervolume: f64,
    pub front_size: usize,
}

pub struct Nsga2Outcome {
    pub front: Vec<ParetoPoint>,
    pub history: Vec<HvRecord>,
}

/// Whether objective vector `a` Pareto-dominates `b` (minimization).
fn dominates(a: &[f64], b: &[f64]) -> bool {
    let mut strictly_better = false;
    for (x, y) in a.iter().zip(b) {
        if x > y {
            return false;
        }
        if x < y {
            strictly_better = true;
        }
    }
    strictly_better
}

/// Fast non-dominated sort. Returns fronts as lists of indices into `objs`.
fn non_dominated_sort(objs: &[Vec<f64>]) -> Vec<Vec<usize>> {
    let n = objs.len();
    let mut dominated_by: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut domination_count = vec![0usize; n];
    let mut fronts: Vec<Vec<usize>> = vec![Vec::new()];

    for p in 0..n {
        for q in 0..n {
            if p == q {
                continue;
            }
            if dominates(&objs[p], &objs[q]) {
                dominated_by[p].push(q);
            } else if dominates(&objs[q], &objs[p]) {
                domination_count[p] += 1;
            }
        }
        if domination_count[p] == 0 {
            fronts[0].push(p);
        }
    }

    let mut i = 0;
    while !fronts[i].is_empty() {
        let mut next = Vec::new();
        for &p in &fronts[i] {
            for &q in &dominated_by[p] {
                domination_count[q] -= 1;
                if domination_count[q] == 0 {
                    next.push(q);
                }
            }
        }
        i += 1;
        fronts.push(next);
    }
    fronts.pop(); // last is empty
    fronts
}

/// Crowding distance for the members of one front.
fn crowding_distance(front: &[usize], objs: &[Vec<f64>]) -> Vec<f64> {
    let m = front.len();
    let mut distance = vec![0.0; m];
    if m == 0 {
        return distance;
    }
    let n_obj = objs[front[0]].len();
    // `k` indexes several parallel structures, so a range loop is clearest here.
    #[allow(clippy::needless_range_loop)]
    for k in 0..n_obj {
        // Sort the front by objective k.
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|&a, &b| objs[front[a]][k].partial_cmp(&objs[front[b]][k]).unwrap());
        let min = objs[front[order[0]]][k];
        let max = objs[front[order[m - 1]]][k];
        distance[order[0]] = f64::INFINITY;
        distance[order[m - 1]] = f64::INFINITY;
        let range = (max - min).max(1e-12);
        for idx in 1..m - 1 {
            let prev = objs[front[order[idx - 1]]][k];
            let next = objs[front[order[idx + 1]]][k];
            distance[order[idx]] += (next - prev) / range;
        }
    }
    distance
}

/// 2-D hypervolume dominated by a minimization front relative to `reference`
/// (a point worse than every solution). Larger is better.
fn hypervolume_2d(front: &[Vec<f64>], reference: &[f64; 2]) -> f64 {
    // Keep only points that lie inside the reference and are non-dominated.
    let mut pts: Vec<(f64, f64)> = front
        .iter()
        .filter(|o| o[0] <= reference[0] && o[1] <= reference[1])
        .map(|o| (o[0], o[1]))
        .collect();
    if pts.is_empty() {
        return 0.0;
    }
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // Staircase area under the non-dominated lower-left envelope.
    let mut hv = 0.0;
    let mut best_f1 = f64::INFINITY;
    let mut envelope: Vec<(f64, f64)> = Vec::new();
    for (f0, f1) in pts {
        if f1 < best_f1 {
            best_f1 = f1;
            envelope.push((f0, f1));
        }
    }
    for i in 0..envelope.len() {
        let (f0, f1) = envelope[i];
        let next_f0 = if i + 1 < envelope.len() {
            envelope[i + 1].0
        } else {
            reference[0]
        };
        hv += (next_f0 - f0) * (reference[1] - f1);
    }
    hv
}

/// Crowded-comparison tournament: prefer lower rank, then higher crowding.
fn crowded_tournament(rank: &[usize], crowd: &[f64], rng: &mut ChaCha8Rng) -> usize {
    let a = rng.gen_range(0..rank.len());
    let b = rng.gen_range(0..rank.len());
    if rank[a] < rank[b] || (rank[a] == rank[b] && crowd[a] > crowd[b]) {
        a
    } else {
        b
    }
}

/// Run NSGA-II.
pub fn run(
    space: &[usize],
    cfg: &Nsga2Config,
    oracle: &mut dyn Oracle,
    rng: &mut ChaCha8Rng,
) -> Nsga2Outcome {
    let mut pop: Vec<Chromosome> = (0..cfg.pop_size)
        .map(|_| Chromosome::random(space, rng))
        .collect();
    let mut objs: Vec<Vec<f64>> = pop.iter().map(|c| oracle.objectives(c)).collect();

    // Fixed reference point for hypervolume: 10% beyond the worst initial
    // value on each axis, so the metric is comparable across generations.
    let reference = {
        let mut r = [0.0f64; 2];
        for o in &objs {
            r[0] = r[0].max(o[0]);
            r[1] = r[1].max(o[1]);
        }
        [r[0] * 1.1 + 1.0, r[1] * 1.1 + 1.0]
    };

    let mut history = Vec::with_capacity(cfg.generations + 1);

    let record =
        |history: &mut Vec<HvRecord>, pop: &[Chromosome], objs: &[Vec<f64>], evals: usize| {
            let fronts = non_dominated_sort(objs);
            let first: Vec<Vec<f64>> = fronts[0].iter().map(|&i| objs[i].clone()).collect();
            history.push(HvRecord {
                evaluations: evals,
                hypervolume: hypervolume_2d(&first, &reference),
                front_size: fronts[0].len(),
            });
            let _ = pop;
        };
    record(&mut history, &pop, &objs, oracle.evaluations());

    for _ in 0..cfg.generations {
        // Ranks and crowding for the current population (for selection).
        let fronts = non_dominated_sort(&objs);
        let mut rank = vec![0usize; pop.len()];
        let mut crowd = vec![0.0f64; pop.len()];
        for (r, front) in fronts.iter().enumerate() {
            let cd = crowding_distance(front, &objs);
            for (j, &idx) in front.iter().enumerate() {
                rank[idx] = r;
                crowd[idx] = cd[j];
            }
        }

        // Generate offspring.
        let mut offspring: Vec<Chromosome> = Vec::with_capacity(cfg.pop_size);
        while offspring.len() < cfg.pop_size {
            let p1 = crowded_tournament(&rank, &crowd, rng);
            let p2 = crowded_tournament(&rank, &crowd, rng);
            let (mut c1, mut c2) = if rng.gen::<f64>() < cfg.crossover_rate {
                uniform_crossover(&pop[p1], &pop[p2], rng)
            } else {
                (pop[p1].clone(), pop[p2].clone())
            };
            c1 = point_mutation(&c1, space, cfg.mutation_rate, rng);
            c2 = point_mutation(&c2, space, cfg.mutation_rate, rng);
            offspring.push(c1);
            if offspring.len() < cfg.pop_size {
                offspring.push(c2);
            }
        }
        let offspring_objs: Vec<Vec<f64>> =
            offspring.iter().map(|c| oracle.objectives(c)).collect();

        // (μ+λ) elitist merge.
        let mut combined: Vec<Chromosome> = pop;
        combined.extend(offspring);
        let mut combined_objs: Vec<Vec<f64>> = objs;
        combined_objs.extend(offspring_objs);

        let fronts = non_dominated_sort(&combined_objs);
        let mut new_pop: Vec<Chromosome> = Vec::with_capacity(cfg.pop_size);
        let mut new_objs: Vec<Vec<f64>> = Vec::with_capacity(cfg.pop_size);
        for front in &fronts {
            if new_pop.len() + front.len() <= cfg.pop_size {
                for &idx in front {
                    new_pop.push(combined[idx].clone());
                    new_objs.push(combined_objs[idx].clone());
                }
            } else {
                // Partially fill from this front by descending crowding distance.
                let cd = crowding_distance(front, &combined_objs);
                let mut order: Vec<usize> = (0..front.len()).collect();
                order.sort_by(|&a, &b| cd[b].partial_cmp(&cd[a]).unwrap());
                for &j in &order {
                    if new_pop.len() >= cfg.pop_size {
                        break;
                    }
                    new_pop.push(combined[front[j]].clone());
                    new_objs.push(combined_objs[front[j]].clone());
                }
                break;
            }
        }

        pop = new_pop;
        objs = new_objs;
        record(&mut history, &pop, &objs, oracle.evaluations());
    }

    // Extract the final Pareto front.
    let fronts = non_dominated_sort(&objs);
    let mut front: Vec<ParetoPoint> = fronts[0]
        .iter()
        .map(|&i| ParetoPoint {
            chrom: pop[i].clone(),
            objectives: objs[i].clone(),
        })
        .collect();
    front.sort_by(|a, b| a.objectives[0].partial_cmp(&b.objectives[0]).unwrap());
    // Deduplicate identical objective vectors for a clean front.
    front.dedup_by(|a, b| a.objectives == b.objectives);

    Nsga2Outcome { front, history }
}
