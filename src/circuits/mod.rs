//! Parameterized circuit factories.
//!
//! A [`CircuitFactory`] turns a [`Chromosome`] into a compiled Plonky2 circuit.
//! The factory pattern (rather than a generic rewrite engine) keeps every
//! candidate **correct by construction**: each gene selects between
//! implementations that compute the same relation, so only the cost varies.
//!
//! Each sub-computation contributes one gene, so the search space grows as
//! `(#strategies)^(#sub-computations)` — tunable, and quickly far beyond what
//! exhaustive enumeration can reach.

use plonky2::iop::target::Target;
use plonky2::plonk::circuit_data::CircuitData;
use rand_chacha::ChaCha8Rng;

use crate::field::{C, D, F};
use crate::strategy::{Chromosome, StrategySpace};

pub mod exponentiation;
pub mod range_check;

/// A compiled circuit together with the input targets a witness must set.
pub struct Built {
    /// The compiled circuit (cost metrics are read from `data.common`).
    pub data: CircuitData<F, C, D>,
    /// Virtual input targets, in the order [`CircuitFactory::random_inputs`]
    /// produces values.
    pub inputs: Vec<Target>,
    /// Number of gate instances before power-of-two padding — the fine-grained
    /// circuit-size metric (captured from `builder.num_gates()`).
    pub gates: usize,
}

/// Maps strategy chromosomes to circuits and defines the reference semantics
/// used to validate that every strategy computes the same relation.
pub trait CircuitFactory: Sync {
    /// Human-readable benchmark name (used in CSV output).
    fn name(&self) -> &str;

    /// Per-gene cardinalities of the strategy search space.
    fn strategy_space(&self) -> StrategySpace;

    /// Compile the circuit selected by `chrom`.
    fn build(&self, chrom: &Chromosome) -> Built;

    /// Sample a random satisfying input vector (private witness values).
    fn random_inputs(&self, rng: &mut ChaCha8Rng) -> Vec<F>;

    /// The public-input vector a correct proof must expose for `inputs`.
    ///
    /// This is the strategy-independent ground truth: every chromosome must
    /// produce a circuit whose proof exposes exactly these public inputs.
    fn reference_public_inputs(&self, inputs: &[F]) -> Vec<F>;
}
