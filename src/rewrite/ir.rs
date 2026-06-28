//! Expression-DAG intermediate representation for rewrite-sequence search.
//!
//! An [`Expr`] is an arithmetic expression over a single input `x` and integer
//! constants. We use reference-counted nodes; because `Rc<T>` derives equality
//! and hashing structurally, identical sub-expressions compare equal and can be
//! memoized. The circuit emitter ([`to_circuit`]) memoizes by sub-expression,
//! so structurally shared sub-DAGs become **common sub-expressions** in the
//! compiled circuit — exactly the sharing a rewrite is trying to create.

use std::collections::HashMap;
use std::rc::Rc;

use plonky2::field::types::Field;
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;

use crate::field::{D, F};

/// An arithmetic expression over the input `x`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expr {
    /// The circuit input `x`.
    Input,
    /// A field constant `c` (canonical `u64`).
    Const(u64),
    Add(Rc<Expr>, Rc<Expr>),
    Mul(Rc<Expr>, Rc<Expr>),
    /// `a²`.
    Square(Rc<Expr>),
    /// `aᵏ` for `k ≥ 2` — an unexpanded power, the main target of rewrites.
    Pow(Rc<Expr>, u64),
}

pub type Node = Rc<Expr>;

pub fn input() -> Node {
    Rc::new(Expr::Input)
}
pub fn constant(c: u64) -> Node {
    Rc::new(Expr::Const(c))
}
pub fn add(a: Node, b: Node) -> Node {
    Rc::new(Expr::Add(a, b))
}
pub fn mul(a: Node, b: Node) -> Node {
    Rc::new(Expr::Mul(a, b))
}
pub fn square(a: Node) -> Node {
    Rc::new(Expr::Square(a))
}
pub fn pow(a: Node, k: u64) -> Node {
    Rc::new(Expr::Pow(a, k))
}

/// Reference semantics: evaluate the expression in the field at `x`.
pub fn eval_field(e: &Node, x: F) -> F {
    fn go(e: &Node, x: F, memo: &mut HashMap<Node, F>) -> F {
        if let Some(v) = memo.get(e) {
            return *v;
        }
        let v = match &**e {
            Expr::Input => x,
            Expr::Const(c) => F::from_canonical_u64(*c),
            Expr::Add(a, b) => go(a, x, memo) + go(b, x, memo),
            Expr::Mul(a, b) => go(a, x, memo) * go(b, x, memo),
            Expr::Square(a) => {
                let t = go(a, x, memo);
                t * t
            }
            Expr::Pow(a, k) => go(a, x, memo).exp_u64(*k),
        };
        memo.insert(e.clone(), v);
        v
    }
    go(e, x, &mut HashMap::new())
}

/// Emit the expression into `builder`, returning the output target. Sharing of
/// structurally-equal sub-expressions is realized via memoization, so the gate
/// for a repeated sub-DAG is emitted exactly once.
pub fn to_circuit(e: &Node, builder: &mut CircuitBuilder<F, D>, x: Target) -> Target {
    fn go(
        e: &Node,
        builder: &mut CircuitBuilder<F, D>,
        x: Target,
        memo: &mut HashMap<Node, Target>,
    ) -> Target {
        if let Some(t) = memo.get(e) {
            return *t;
        }
        let t = match &**e {
            Expr::Input => x,
            Expr::Const(c) => builder.constant(F::from_canonical_u64(*c)),
            Expr::Add(a, b) => {
                let ta = go(a, builder, x, memo);
                let tb = go(b, builder, x, memo);
                builder.add(ta, tb)
            }
            Expr::Mul(a, b) => {
                let ta = go(a, builder, x, memo);
                let tb = go(b, builder, x, memo);
                builder.mul(ta, tb)
            }
            Expr::Square(a) => {
                let ta = go(a, builder, x, memo);
                builder.square(ta)
            }
            Expr::Pow(a, k) => {
                let ta = go(a, builder, x, memo);
                builder.exp_u64(ta, *k)
            }
        };
        memo.insert(e.clone(), t);
        t
    }
    go(e, builder, x, &mut HashMap::new())
}

/// Number of distinct (shared) interior operations — a cheap structural proxy
/// for circuit size, used only for diagnostics.
pub fn node_count(e: &Node) -> usize {
    fn go(e: &Node, seen: &mut std::collections::HashSet<Node>) {
        if !seen.insert(e.clone()) {
            return;
        }
        match &**e {
            Expr::Add(a, b) | Expr::Mul(a, b) => {
                go(a, seen);
                go(b, seen);
            }
            Expr::Square(a) | Expr::Pow(a, _) => go(a, seen),
            _ => {}
        }
    }
    let mut seen = std::collections::HashSet::new();
    go(e, &mut seen);
    seen.len()
}
