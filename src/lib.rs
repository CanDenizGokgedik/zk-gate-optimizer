//! # zk-gate-evo
//!
//! Multi-objective evolutionary optimization of **gate-implementation
//! strategies** in [Plonky2](https://github.com/0xPolygonZero/plonky2)
//! zero-knowledge circuits.
//!
//! Many sub-computations in a PLONKish circuit admit several mathematically
//! equivalent gate encodings (e.g. a range check via a base-2 `BaseSumGate`, a
//! base-B limb decomposition, or an explicit bit split). Each is *correct* but
//! has a different cost profile (trace length, constraint count, polynomial
//! degree, proof size). This crate searches the space of such strategy choices
//! with evolutionary algorithms and validates the result against real proof
//! generation.
//!
//! ## Module layout
//! - [`field`] — Goldilocks / Poseidon type aliases.
//! - [`strategy`] — the [`strategy::Chromosome`] encoding and search-space helpers.
//! - [`circuits`] — parameterized circuit factories (one gene per sub-computation).
//! - [`fitness`] — static cost metrics and real prover-time measurement.
//! - [`ec`] — the genetic algorithm, NSGA-II, and baseline optimizers.
//! - [`experiment`] — reproducible experiment runners that emit CSV.
//!
//! The central abstraction is [`circuits::CircuitFactory`]: it maps a
//! chromosome to a compiled circuit, so that every individual in the population
//! is correct by construction and only the cost varies.

pub mod circuits;
pub mod ec;
pub mod experiment;
pub mod field;
pub mod fitness;
pub mod rewrite;
pub mod strategy;
