//! Field and proof-system type aliases used throughout the crate.
//!
//! We target Plonky2's default configuration: the 64-bit Goldilocks field with
//! a degree-2 extension and the Poseidon-based hash configuration. Fixing these
//! in one place keeps the rest of the crate free of long generic bounds.

use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::plonk::config::PoseidonGoldilocksConfig;

/// Extension-field degree used by the FRI-based proof system.
pub const D: usize = 2;

/// The generic proof configuration (Poseidon hash over Goldilocks).
pub type C = PoseidonGoldilocksConfig;

/// The base field.
pub type F = GoldilocksField;
