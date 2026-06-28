//! Fixed-exponent exponentiation benchmark.
//!
//! The circuit computes `y_i = x_i^k` for `num_values` inputs and a fixed
//! exponent `k`. Each value's gene selects one of three mathematically
//! equivalent ways to evaluate the power:
//!
//! - `0` — Plonky2's [`exp_u64`](plonky2::plonk::circuit_builder::CircuitBuilder::exp_u64)
//!   gadget (uses the dedicated exponentiation machinery),
//! - `1` — naive repeated multiplication (`k - 1` `ArithmeticGate` multiplies),
//! - `2` — square-and-multiply (≈`log2(k)` squarings plus multiplies).
//!
//! All three yield the identical output, so any chromosome is correct; they
//! differ only in gate count and degree. This is the exponentiation-threshold
//! trade-off from the proposal, made concrete and searchable.

use plonky2::field::types::Field;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitConfig;
use rand::RngCore;
use rand_chacha::ChaCha8Rng;

use super::{Built, CircuitFactory};
use crate::field::{C, D, F};
use crate::strategy::{Chromosome, StrategySpace};

/// Number of equivalent exponentiation strategies.
const N_STRATEGIES: usize = 3;

/// Compute `x_i^exponent` for `num_values` inputs.
pub struct Exponentiation {
    pub num_values: usize,
    pub exponent: u64,
}

impl Exponentiation {
    pub fn new(num_values: usize, exponent: u64) -> Self {
        assert!(exponent >= 1, "exponent must be >= 1");
        Self {
            num_values,
            exponent,
        }
    }

    fn power(&self, builder: &mut CircuitBuilder<F, D>, x: Target, gene: u8) -> Target {
        match gene {
            0 => builder.exp_u64(x, self.exponent),
            1 => {
                // Naive: x * x * ... * x  (exponent - 1 multiplications).
                let mut acc = x;
                for _ in 1..self.exponent {
                    acc = builder.mul(acc, x);
                }
                acc
            }
            2 => {
                // Square-and-multiply (left-to-right binary exponentiation).
                let mut result = builder.one();
                let mut base = x;
                let mut e = self.exponent;
                while e > 0 {
                    if e & 1 == 1 {
                        result = builder.mul(result, base);
                    }
                    e >>= 1;
                    if e > 0 {
                        base = builder.square(base);
                    }
                }
                result
            }
            other => unreachable!("exponentiation gene out of range: {other}"),
        }
    }
}

/// Reference field exponentiation, used to validate every strategy's output.
fn field_pow(x: F, k: u64) -> F {
    let mut result = F::ONE;
    let mut base = x;
    let mut e = k;
    while e > 0 {
        if e & 1 == 1 {
            result *= base;
        }
        e >>= 1;
        if e > 0 {
            base = base * base;
        }
    }
    result
}

impl CircuitFactory for Exponentiation {
    fn name(&self) -> &str {
        "exponentiation"
    }

    fn strategy_space(&self) -> StrategySpace {
        vec![N_STRATEGIES; self.num_values]
    }

    fn build(&self, chrom: &Chromosome) -> Built {
        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);
        let mut inputs = Vec::with_capacity(self.num_values);
        for i in 0..self.num_values {
            let x = builder.add_virtual_target();
            builder.register_public_input(x);
            let y = self.power(&mut builder, x, chrom.genes[i]);
            builder.register_public_input(y);
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
        // Shift right by one bit to stay below the Goldilocks modulus.
        (0..self.num_values)
            .map(|_| F::from_canonical_u64(rng.next_u64() >> 1))
            .collect()
    }

    fn reference_public_inputs(&self, inputs: &[F]) -> Vec<F> {
        // Public inputs are interleaved (x_i, y_i) in registration order.
        let mut out = Vec::with_capacity(inputs.len() * 2);
        for &x in inputs {
            out.push(x);
            out.push(field_pow(x, self.exponent));
        }
        out
    }
}
