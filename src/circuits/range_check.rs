//! Range-check benchmark.
//!
//! The circuit asserts that each of `num_values` field elements lies in
//! `[0, 2^bits)`. Every value is range-checked independently, and the gene for
//! that value selects the **base** of the limb decomposition used to enforce
//! the bound: a value can be split into base-`B` limbs (each constrained by a
//! `BaseSumGate<B>` of polynomial degree `B`) for `B in {2, 4, 8, 16}`. Larger
//! bases use fewer limbs but raise the constraint degree — the classic
//! `BaseSumGate` base trade-off, here epistatically coupled through the
//! circuit's aggregate maximum degree.

use plonky2::field::types::Field;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitConfig;
use rand::RngCore;
use rand_chacha::ChaCha8Rng;

use super::{Built, CircuitFactory};
use crate::field::{C, D, F};
use crate::strategy::{Chromosome, StrategySpace};

/// Limb bases offered as strategies. `bits` must be divisible by each
/// `log2(base)` so that the decomposition covers exactly `bits` bits.
///
/// Base 16 (`BaseSumGate` of degree 16) is excluded: it exceeds the standard
/// recursion config's `max_quotient_degree_factor` of 8 and fails to build.
/// This is itself an instance of the implicit validity constraint discussed in
/// the design notes — not every gate strategy compiles under a given config.
const BASES: [usize; 3] = [2, 4, 8];

/// Range-check `num_values` independent field elements to `bits` bits.
pub struct RangeCheck {
    pub num_values: usize,
    pub bits: usize,
}

impl RangeCheck {
    pub fn new(num_values: usize, bits: usize) -> Self {
        for &b in &BASES {
            let lg = b.trailing_zeros() as usize;
            assert!(
                bits.is_multiple_of(lg),
                "bits ({bits}) must be divisible by log2(base={b})={lg}"
            );
        }
        Self { num_values, bits }
    }

    /// Apply the limb decomposition selected by `gene` to `x`.
    ///
    /// `split_le_base::<B>` introduces a `BaseSumGate<B>` that both range-checks
    /// each limb to `[0, B)` and binds `x` to the weighted sum of its limbs,
    /// which enforces `x < B^num_limbs = 2^bits`.
    fn decompose(&self, builder: &mut CircuitBuilder<F, D>, x: Target, gene: u8) {
        match gene {
            0 => {
                builder.split_le_base::<2>(x, self.bits);
            }
            1 => {
                builder.split_le_base::<4>(x, self.bits / 2);
            }
            2 => {
                builder.split_le_base::<8>(x, self.bits / 3);
            }
            other => unreachable!("range-check gene out of range: {other}"),
        }
    }
}

impl CircuitFactory for RangeCheck {
    fn name(&self) -> &str {
        "range_check"
    }

    fn strategy_space(&self) -> StrategySpace {
        vec![BASES.len(); self.num_values]
    }

    fn build(&self, chrom: &Chromosome) -> Built {
        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);
        let mut inputs = Vec::with_capacity(self.num_values);
        for i in 0..self.num_values {
            let x = builder.add_virtual_target();
            builder.register_public_input(x);
            self.decompose(&mut builder, x, chrom.genes[i]);
            inputs.push(x);
        }
        let gates = builder.num_gates();
        let data = builder.build::<C>();
        Built {
            data,
            inputs,
            gates,
        }
    }

    fn random_inputs(&self, rng: &mut ChaCha8Rng) -> Vec<F> {
        let modulus = 1u64 << self.bits; // bits <= 32 in practice
        (0..self.num_values)
            .map(|_| F::from_canonical_u64(rng.next_u64() % modulus))
            .collect()
    }

    fn reference_public_inputs(&self, inputs: &[F]) -> Vec<F> {
        // The relation is a pure predicate; the circuit exposes its inputs.
        inputs.to_vec()
    }
}
