//! Phase 3 — rewrite-sequence search.
//!
//! A chromosome is a fixed-length sequence of semantics-preserving rewrite
//! rules applied to a base expression DAG. This is the regime where the search
//! is genuinely hard: the rules are non-confluent (ordering matters), every
//! sequence is correct by construction, and minimizing the resulting circuit's
//! trace length is an addition-chain/common-sub-expression problem with many
//! local optima — exactly where population-based search can beat local search.
//!
//! The encoding reuses [`Chromosome`](crate::strategy::Chromosome) and the
//! whole optimizer suite unchanged: variable-length sequences are emulated by a
//! fixed length with no-op genes and rules that skip when inapplicable.

pub mod factory;
pub mod ir;
pub mod rules;
