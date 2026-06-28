//! Integration tests: correctness of the circuit factories and a sanity check
//! that the genetic algorithm recovers the exhaustively-verified optimum on a
//! small instance.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use zk_gate_evo::circuits::exponentiation::Exponentiation;
use zk_gate_evo::circuits::range_check::RangeCheck;
use zk_gate_evo::circuits::CircuitFactory;
use zk_gate_evo::ec::baselines;
use zk_gate_evo::ec::ga::{self, GaConfig};
use zk_gate_evo::ec::StaticOracle;
use zk_gate_evo::fitness::verify_strategy;
use zk_gate_evo::rewrite::factory::PolyRewrite;
use zk_gate_evo::strategy::{enumerate, Chromosome};

/// Every strategy of the exponentiation benchmark must compute the same output
/// and produce a verifying proof.
#[test]
fn exponentiation_strategies_are_equivalent() {
    let factory = Exponentiation::new(3, 7);
    let space = factory.strategy_space();
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let inputs = factory.random_inputs(&mut rng);
    for chrom in enumerate(&space) {
        assert!(
            verify_strategy(&factory, &chrom, &inputs).unwrap(),
            "strategy {:?} did not match the reference output",
            chrom.genes
        );
    }
}

/// Every range-check strategy must verify for in-range inputs.
#[test]
fn range_check_strategies_verify() {
    let factory = RangeCheck::new(3, 24);
    let space = factory.strategy_space();
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    for _ in 0..4 {
        let inputs = factory.random_inputs(&mut rng);
        for chrom in enumerate(&space) {
            assert!(
                verify_strategy(&factory, &chrom, &inputs).unwrap(),
                "range-check strategy {:?} failed to verify",
                chrom.genes
            );
        }
    }
}

/// The GA must reach the global optimum found by exhaustive enumeration on a
/// small, fully enumerable instance.
#[test]
fn ga_matches_exhaustive_optimum() {
    let factory = Exponentiation::new(5, 7);
    let space = factory.strategy_space();

    let mut exo = StaticOracle::new(&factory);
    let exhaustive = baselines::exhaustive(&space, 100_000, &mut exo)
        .expect("space is small enough to enumerate");

    let cfg = GaConfig {
        pop_size: 20,
        generations: 30,
        ..GaConfig::default()
    };
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut oracle = StaticOracle::new(&factory);
    let outcome = ga::run(&space, &cfg, &mut oracle, &mut rng);

    assert_eq!(
        outcome.best_fitness, exhaustive.best_fitness,
        "GA fitness {} did not reach exhaustive optimum {}",
        outcome.best_fitness, exhaustive.best_fitness
    );
}

/// The default (all-zero) chromosome is well-formed for both factories.
#[test]
fn default_chromosome_builds() {
    let exp = Exponentiation::new(4, 7);
    let _ = exp.build(&Chromosome::new(vec![0; 4]));
    let range = RangeCheck::new(4, 24);
    let _ = range.build(&Chromosome::new(vec![0; 4]));
}

/// Phase 3: every sequence of rewrite rules must preserve the polynomial value
/// (correctness by construction).
#[test]
fn rewrite_sequences_preserve_semantics() {
    let factory = PolyRewrite::new(10, 20);
    let space = factory.strategy_space();
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    for _ in 0..15 {
        let chrom = Chromosome::random(&space, &mut rng);
        let inputs = factory.random_inputs(&mut rng);
        assert!(
            verify_strategy(&factory, &chrom, &inputs).unwrap(),
            "rewrite sequence {:?} changed the computed value",
            chrom.genes
        );
    }
}

/// Phase 3: on the rewrite-sequence landscape the GA must beat hill climbing —
/// the multi-modal regime where population-based search earns its keep. Seeded,
/// hence deterministic (not flaky).
#[test]
fn ga_beats_hill_climbing_on_rewrite_landscape() {
    let factory = PolyRewrite::new(12, 24);
    let space = factory.strategy_space();
    let cfg = GaConfig {
        pop_size: 30,
        generations: 30,
        ..GaConfig::default()
    };
    let budget = cfg.pop_size * (cfg.generations + 1);

    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut ga_oracle = StaticOracle::new(&factory);
    let ga = ga::run(&space, &cfg, &mut ga_oracle, &mut rng);

    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut hc_oracle = StaticOracle::new(&factory);
    let hc = baselines::hill_climb(&space, budget, &mut hc_oracle, &mut rng);

    assert!(
        ga.best_fitness < hc.best_fitness,
        "GA ({}) should beat hill climbing ({}) on the rewrite landscape",
        ga.best_fitness,
        hc.best_fitness
    );
}
