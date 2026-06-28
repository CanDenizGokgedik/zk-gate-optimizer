//! Evolutionary and baseline optimizers.
//!
//! All optimizers consume a [`Oracle`], which maps a chromosome to either a
//! scalar fitness (single-objective) or an objective vector (multi-objective)
//! while transparently caching results and counting *true* evaluations
//! (circuit builds). Counting evaluations rather than generations lets every
//! algorithm be compared on the same budget.

use std::collections::HashMap;

use crate::circuits::CircuitFactory;
use crate::fitness::{cost_static, Cost};
use crate::strategy::Chromosome;

pub mod baselines;
pub mod ga;
pub mod nsga2;
pub mod operators;

/// One sampled point in an optimizer's progress trace.
#[derive(Clone, Debug)]
pub struct Record {
    /// Number of true fitness evaluations performed so far.
    pub evaluations: usize,
    /// Best scalar fitness found so far (lower is better).
    pub best: f64,
    /// Mean scalar fitness of the current population (`NaN` for non-population
    /// methods).
    pub mean: f64,
    /// Population diversity in `[0, 1]` (`NaN` for non-population methods).
    pub diversity: f64,
}

/// Fitness oracle with memoization and evaluation counting.
pub trait Oracle {
    /// Scalar fitness (lower is better).
    fn scalar(&mut self, chrom: &Chromosome) -> f64;
    /// Objective vector to be minimized component-wise.
    fn objectives(&mut self, chrom: &Chromosome) -> Vec<f64>;
    /// Number of distinct circuits actually built so far.
    fn evaluations(&self) -> usize;
}

/// Oracle backed by static (non-proving) cost metrics.
pub struct StaticOracle<'a> {
    factory: &'a dyn CircuitFactory,
    cache: HashMap<Chromosome, Cost>,
    evals: usize,
}

impl<'a> StaticOracle<'a> {
    pub fn new(factory: &'a dyn CircuitFactory) -> Self {
        Self {
            factory,
            cache: HashMap::new(),
            evals: 0,
        }
    }

    fn cost(&mut self, chrom: &Chromosome) -> Cost {
        if let Some(cost) = self.cache.get(chrom) {
            return cost.clone();
        }
        let built = self.factory.build(chrom);
        self.evals += 1;
        let cost = cost_static(&built);
        self.cache.insert(chrom.clone(), cost.clone());
        cost
    }
}

impl Oracle for StaticOracle<'_> {
    fn scalar(&mut self, chrom: &Chromosome) -> f64 {
        self.cost(chrom).scalar()
    }

    fn objectives(&mut self, chrom: &Chromosome) -> Vec<f64> {
        self.cost(chrom).objectives()
    }

    fn evaluations(&self) -> usize {
        self.evals
    }
}
