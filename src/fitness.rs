//! Cost metrics and evaluation.
//!
//! Two fidelities are provided:
//! - [`cost_static`] reads the four circuit-cost metrics straight from the
//!   compiled [`CommonCircuitData`](plonky2::plonk::circuit_data::CommonCircuitData)
//!   **without proving**. This is cheap and drives the evolutionary search.
//! - [`cost_real`] actually generates a proof and measures wall-clock prover
//!   time and serialized proof size. This is expensive and is reserved for the
//!   final Pareto-front individuals, to confirm the static proxy tracks reality.

use std::time::{Duration, Instant};

use plonky2::iop::witness::{PartialWitness, WitnessWrite};

use crate::circuits::{Built, CircuitFactory};
use crate::field::F;
use crate::strategy::Chromosome;

/// Cost metrics of a compiled circuit.
#[derive(Clone, Debug, PartialEq)]
pub struct Cost {
    /// Gate instances before power-of-two padding — the fine-grained circuit
    /// size and the primary optimization objective ("gate golfing").
    pub gate_count: usize,
    /// `log2` of the padded execution-trace row count.
    pub degree_bits: usize,
    /// Low-degree-extension size `2^(degree_bits + rate_bits)`; the dominant
    /// term in prover FFT cost and the best single static proxy.
    pub lde_size: usize,
    /// Largest number of constraints imposed by any gate in the circuit.
    pub num_gate_constraints: usize,
    /// Degree factor of the PLONK quotient polynomial.
    pub quotient_degree_factor: usize,
    /// Maximum constraint degree over the gates actually used — the second,
    /// genuinely conflicting objective (lower degree means a smaller quotient
    /// polynomial but usually more gates).
    pub constraint_degree: usize,
}

impl Cost {
    /// Objective vector for the multi-objective search. Note: under Plonky2's
    /// standard config the only metric that varies with gate strategy is the
    /// circuit size; `constraint_degree`, `num_gate_constraints`, and
    /// `quotient_degree_factor` are config constants. The two components here
    /// (`gate_count`, `lde_size`) are therefore *aligned*, which is why the
    /// NSGA-II front collapses to a point — reported as a finding, not hidden.
    pub fn objectives(&self) -> Vec<f64> {
        vec![self.gate_count as f64, self.lde_size as f64]
    }

    /// Single-objective scalarization (lower is better): minimize gate count,
    /// breaking ties by padded LDE size so the optimizer prefers circuits that
    /// sit further below the next power-of-two boundary.
    pub fn scalar(&self) -> f64 {
        self.gate_count as f64 + self.lde_size as f64 / 1e6
    }
}

/// Read the static cost metrics from a compiled circuit (no proving).
pub fn cost_static(built: &Built) -> Cost {
    let common = &built.data.common;
    Cost {
        gate_count: built.gates,
        degree_bits: common.degree_bits(),
        lde_size: common.lde_size(),
        num_gate_constraints: common.num_gate_constraints,
        quotient_degree_factor: common.quotient_degree_factor,
        constraint_degree: common.constraint_degree(),
    }
}

/// Measured prover cost for a single circuit.
#[derive(Clone, Debug)]
pub struct RealCost {
    /// Median prover wall-clock time over the repeated runs.
    pub prove_time: Duration,
    /// Size of the serialized proof in bytes.
    pub proof_bytes: usize,
}

/// Build a witness assigning `inputs` to the circuit's input targets.
fn make_witness(built: &Built, inputs: &[F]) -> PartialWitness<F> {
    let mut pw = PartialWitness::new();
    for (t, v) in built.inputs.iter().zip(inputs) {
        pw.set_target(*t, *v);
    }
    pw
}

/// Generate proofs and report the **median** prover time and the proof size.
///
/// One untimed warm-up proof is generated first to amortize allocator and
/// CPU-cache cold-start effects, then `repeats` timed proofs are taken and the
/// median is reported. For reproducible numbers, pin the thread count
/// (`RAYON_NUM_THREADS=1`) — Plonky2 parallelizes with Rayon. Note that for
/// very small circuits the wall-clock signal is dominated by fixed overhead and
/// is inherently noisy; the deterministic static metrics in [`Cost`] are the
/// reliable cost signal at that scale.
pub fn cost_real(built: &Built, inputs: &[F], repeats: usize) -> anyhow::Result<RealCost> {
    let repeats = repeats.max(1);

    // Warm-up (untimed).
    let warmup = make_witness(built, inputs);
    let proof = built.data.prove(warmup)?;
    let mut proof_bytes = proof.to_bytes().len();

    let mut times = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let pw = make_witness(built, inputs);
        let start = Instant::now();
        let proof = built.data.prove(pw)?;
        times.push(start.elapsed());
        proof_bytes = proof.to_bytes().len();
    }
    times.sort();
    Ok(RealCost {
        prove_time: times[times.len() / 2],
        proof_bytes,
    })
}

/// Validate that a chromosome produces a circuit computing the reference
/// relation: the proof must verify and expose exactly the reference public
/// inputs. Returns `Ok(true)` on success.
pub fn verify_strategy(
    factory: &dyn CircuitFactory,
    chrom: &Chromosome,
    inputs: &[F],
) -> anyhow::Result<bool> {
    let built = factory.build(chrom);
    let pw = make_witness(&built, inputs);
    let proof = built.data.prove(pw)?;
    let reference = factory.reference_public_inputs(inputs);
    let public_ok = proof.public_inputs == reference;
    built.data.verify(proof)?;
    Ok(public_ok)
}
