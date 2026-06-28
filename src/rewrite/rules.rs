//! Semantics-preserving rewrite rules over the expression DAG.
//!
//! Every rule maps an expression to an algebraically equal one, so any sequence
//! of rules applied to a base expression yields a circuit computing the same
//! value — correctness by construction. The rules are intentionally
//! **non-confluent**: e.g. `FoldSquare` and `CombineMul` rewrite `a·a` to
//! different normal forms (`a²` vs `a^2`), and expanding a power before vs.
//! after factoring reaches different circuits. Order therefore matters, which
//! is what makes the search over rule *sequences* non-trivial.

use std::rc::Rc;

use super::ir::{add, mul, pow, square, Expr, Node};

/// The rule alphabet. Gene value `i` selects `rule_of(i)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Identity (no change) — lets a chromosome encode an effectively shorter
    /// sequence.
    NoOp,
    /// `aᵏ → a^(k-1) · a`.
    ExpandMinusOne,
    /// `aᵏ → (a^⌊k/2⌋)²` (· a if k odd) — the binary-exponentiation step.
    ExpandHalve,
    /// `aᵏ → (aᵖ)^(k/p)` for the smallest prime factor `p` of a composite `k`.
    ExpandFactor,
    /// `a · a → a²`.
    FoldSquare,
    /// Merge multiplied powers of the same base: `aⁱ · aʲ → a^(i+j)`.
    CombineMul,
}

/// Number of rules in the alphabet.
pub const NUM_RULES: usize = 6;

/// Map a gene value to a rule (values are taken mod the alphabet size).
pub fn rule_of(gene: u8) -> Rule {
    match gene as usize % NUM_RULES {
        0 => Rule::NoOp,
        1 => Rule::ExpandMinusOne,
        2 => Rule::ExpandHalve,
        3 => Rule::ExpandFactor,
        4 => Rule::FoldSquare,
        _ => Rule::CombineMul,
    }
}

/// `aᵏ`, or just `a` when `k == 1`.
fn pow_or(a: Node, k: u64) -> Node {
    if k == 1 {
        a
    } else {
        pow(a, k)
    }
}

/// Smallest prime factor of a composite `k ≥ 4`, or `None` if `k` is prime or
/// too small to factor.
fn smallest_factor(k: u64) -> Option<u64> {
    if k < 4 {
        return None;
    }
    let mut p = 2;
    while p * p <= k {
        if k.is_multiple_of(p) {
            return Some(p);
        }
        p += 1;
    }
    None
}

/// Try to apply `rule` at this node only (no recursion). Returns the rewritten
/// node if the rule's pattern matches here.
fn local(rule: Rule, e: &Node) -> Option<Node> {
    match rule {
        Rule::NoOp => None,
        Rule::ExpandMinusOne => {
            if let Expr::Pow(a, k) = &**e {
                if *k >= 2 {
                    return Some(mul(pow_or(a.clone(), k - 1), a.clone()));
                }
            }
            None
        }
        Rule::ExpandHalve => {
            if let Expr::Pow(a, k) = &**e {
                if *k >= 2 {
                    let half = square(pow_or(a.clone(), k / 2));
                    return Some(if k % 2 == 0 {
                        half
                    } else {
                        mul(half, a.clone())
                    });
                }
            }
            None
        }
        Rule::ExpandFactor => {
            if let Expr::Pow(a, k) = &**e {
                if let Some(p) = smallest_factor(*k) {
                    return Some(pow(pow(a.clone(), p), k / p));
                }
            }
            None
        }
        Rule::FoldSquare => {
            if let Expr::Mul(l, r) = &**e {
                if l == r {
                    return Some(square(l.clone()));
                }
            }
            None
        }
        Rule::CombineMul => {
            if let Expr::Mul(l, r) = &**e {
                if let (Expr::Pow(a, i), Expr::Pow(b, j)) = (&**l, &**r) {
                    if a == b {
                        return Some(pow(a.clone(), i + j));
                    }
                }
                if let Expr::Pow(a, i) = &**l {
                    if a == r {
                        return Some(pow(a.clone(), i + 1));
                    }
                }
                if let Expr::Pow(b, j) = &**r {
                    if b == l {
                        return Some(pow(b.clone(), j + 1));
                    }
                }
                if l == r {
                    return Some(pow(l.clone(), 2));
                }
            }
            None
        }
    }
}

/// Apply `f` to the first node, in top-down left-to-right order, where it
/// matches; return the rewritten whole expression, or `None` if nowhere matched.
fn map_first(e: &Node, f: &dyn Fn(&Node) -> Option<Node>) -> Option<Node> {
    if let Some(n) = f(e) {
        return Some(n);
    }
    match &**e {
        Expr::Add(a, b) => {
            if let Some(na) = map_first(a, f) {
                return Some(add(na, b.clone()));
            }
            map_first(b, f).map(|nb| add(a.clone(), nb))
        }
        Expr::Mul(a, b) => {
            if let Some(na) = map_first(a, f) {
                return Some(mul(na, b.clone()));
            }
            map_first(b, f).map(|nb| mul(a.clone(), nb))
        }
        Expr::Square(a) => map_first(a, f).map(square),
        Expr::Pow(a, k) => map_first(a, f).map(|na| pow(na, *k)),
        Expr::Input | Expr::Const(_) => None,
    }
}

/// Apply one rule to an expression, rewriting the first matching node. If the
/// rule does not apply anywhere, the expression is returned unchanged (a no-op
/// gene), which keeps every chromosome valid.
pub fn apply(rule: Rule, e: &Node) -> Node {
    if rule == Rule::NoOp {
        return e.clone();
    }
    map_first(e, &|n| local(rule, n)).unwrap_or_else(|| e.clone())
}

/// Apply a whole sequence of rules (a chromosome) left-to-right.
pub fn apply_sequence(genes: &[u8], base: &Node) -> Node {
    let mut cur = Rc::clone(base);
    for &g in genes {
        cur = apply(rule_of(g), &cur);
    }
    cur
}
