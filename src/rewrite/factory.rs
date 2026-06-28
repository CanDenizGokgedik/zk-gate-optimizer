//! Rewrite-sequence benchmark: evaluate a fixed polynomial via an expression
//! DAG, and let the chromosome be a sequence of rewrite rules applied to it.
//!
//! The base expression is `P(x) = Σ_{i=0}^{degree} xⁱ`, with each `xⁱ` an
//! unexpanded `Pow` node. A chromosome is a fixed-length sequence of rule
//! indices (gene `0` = no-op); rules are applied left-to-right. Because the
//! rules are non-confluent, different sequences reach different circuits, and
//! the circuit's trace length is the cost being minimized — an
//! addition-chain/common-sub-expression search that is genuinely multi-modal.

use plonky2::field::types::Field;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitConfig;
use rand::RngCore;
use rand_chacha::ChaCha8Rng;

use super::ir::{add, constant, eval_field, input, pow, to_circuit, Node};
use super::rules::{apply_sequence, NUM_RULES};
use crate::circuits::{Built, CircuitFactory};
use crate::field::{C, D, F};
use crate::strategy::{Chromosome, StrategySpace};

/// Polynomial-evaluation rewrite benchmark.
pub struct PolyRewrite {
    /// Degree of `P(x) = Σ xⁱ`.
    pub degree: usize,
    /// Number of rewrite-rule slots in a chromosome.
    pub seq_len: usize,
}

impl PolyRewrite {
    pub fn new(degree: usize, seq_len: usize) -> Self {
        assert!(degree >= 2, "degree must be >= 2");
        assert!(seq_len >= 1, "seq_len must be >= 1");
        Self { degree, seq_len }
    }

    /// The base expression `Σ_{i=0}^{degree} xⁱ` before any rewriting.
    fn base(&self) -> Node {
        let mut acc = constant(1); // x^0
        acc = add(acc, input()); // x^1
        for i in 2..=self.degree as u64 {
            acc = add(acc, pow(input(), i));
        }
        acc
    }
}

impl CircuitFactory for PolyRewrite {
    fn name(&self) -> &str {
        "poly_rewrite"
    }

    fn strategy_space(&self) -> StrategySpace {
        vec![NUM_RULES; self.seq_len]
    }

    fn build(&self, chrom: &Chromosome) -> Built {
        let expr = apply_sequence(&chrom.genes, &self.base());
        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);
        let x = builder.add_virtual_target();
        builder.register_public_input(x);
        let y = to_circuit(&expr, &mut builder, x);
        builder.register_public_input(y);
        let gates = builder.num_gates();
        let data = builder.build::<C>();
        Built {
            data,
            inputs: vec![x],
            gates,
        }
    }

    fn random_inputs(&self, rng: &mut ChaCha8Rng) -> Vec<F> {
        vec![F::from_canonical_u64(rng.next_u64() >> 1)]
    }

    fn reference_public_inputs(&self, inputs: &[F]) -> Vec<F> {
        let x = inputs[0];
        vec![x, eval_field(&self.base(), x)]
    }
}
