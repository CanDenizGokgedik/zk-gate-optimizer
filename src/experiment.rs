//! Reproducible experiment runners.
//!
//! Each runner is deterministic given its seed and writes results to `results/`
//! as CSV for downstream analysis (see `analysis/plot.py`). Three experiments:
//! correctness validation, the algorithm comparison (GA vs. baselines), and the
//! NSGA-II Pareto front with measured prover cost.

use std::fs::{self, File};
use std::io::{BufWriter, Write};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::circuits::CircuitFactory;
use crate::ec::ga::{self, GaConfig};
use crate::ec::nsga2::{self, Nsga2Config};
use crate::ec::{baselines, Record, StaticOracle};
use crate::fitness::{cost_real, cost_static, verify_strategy};
use crate::strategy::{space_size, Chromosome};

/// Largest search space the exhaustive baseline will attempt to enumerate.
const EXHAUSTIVE_LIMIT: u128 = 200_000;

fn ensure_results_dir() {
    fs::create_dir_all("results").expect("create results/ directory");
}

/// Print the static cost of each uniform strategy (all genes set to the same
/// value), plus a few random mixes. Useful for checking whether the objectives
/// actually vary with strategy and conflict with one another.
pub fn run_inspect(factory: &dyn CircuitFactory, seed: u64) {
    let space = factory.strategy_space();
    let n_strategies = space.first().copied().unwrap_or(0);
    println!("  uniform strategies (all genes = s): lde_size / num_gate_constraints / degree_bits");
    for s in 0..n_strategies {
        let chrom = Chromosome::new(vec![s as u8; space.len()]);
        let cost = cost_static(&factory.build(&chrom));
        println!(
            "    s={s}: gates={:<5} degree={:<3} lde={:<6} degree_bits={}",
            cost.gate_count, cost.constraint_degree, cost.lde_size, cost.degree_bits
        );
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    println!("  random mixes:");
    for _ in 0..5 {
        let chrom = Chromosome::random(&space, &mut rng);
        let cost = cost_static(&factory.build(&chrom));
        println!(
            "    gates={:<5} degree={:<3} lde={:<6} degree_bits={}",
            cost.gate_count, cost.constraint_degree, cost.lde_size, cost.degree_bits
        );
    }
}

/// Validate that random strategies all compute the reference relation.
pub fn run_correctness(
    factory: &dyn CircuitFactory,
    samples: usize,
    seed: u64,
) -> anyhow::Result<(usize, usize)> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let space = factory.strategy_space();
    let mut passed = 0;
    for _ in 0..samples {
        let chrom = Chromosome::random(&space, &mut rng);
        let inputs = factory.random_inputs(&mut rng);
        if verify_strategy(factory, &chrom, &inputs)? {
            passed += 1;
        }
    }
    Ok((passed, samples))
}

fn write_history<W: Write>(
    w: &mut W,
    benchmark: &str,
    algo: &str,
    run: usize,
    history: &[Record],
) -> std::io::Result<()> {
    for r in history {
        writeln!(
            w,
            "{benchmark},{algo},{run},{},{},{},{}",
            r.evaluations, r.best, r.mean, r.diversity
        )?;
    }
    Ok(())
}

/// Compare the GA (and two ablations) against random search, hill climbing,
/// greedy descent, and — where tractable — exhaustive enumeration. Runs each
/// stochastic method over `runs` independent seeds.
pub fn run_compare(
    factory: &dyn CircuitFactory,
    runs: usize,
    ga_cfg: &GaConfig,
    seed: u64,
) -> anyhow::Result<()> {
    ensure_results_dir();
    let space = factory.strategy_space();
    let budget = ga_cfg.pop_size * (ga_cfg.generations + 1);
    let sample_every = ga_cfg.pop_size.max(1);

    let path = "results/convergence.csv";
    let mut w = BufWriter::new(File::create(path)?);
    writeln!(w, "benchmark,algo,run,evaluations,best,mean,diversity")?;

    // Exhaustive optimum (deterministic), if the space is small enough.
    {
        let mut oracle = StaticOracle::new(factory);
        if let Some(out) = baselines::exhaustive(&space, EXHAUSTIVE_LIMIT, &mut oracle) {
            write_history(&mut w, factory.name(), "exhaustive", 0, &out.history)?;
            println!(
                "  exhaustive global optimum = {:.0} (space size {})",
                out.best_fitness,
                space_size(&space)
            );
        } else {
            println!(
                "  exhaustive skipped: space size {} exceeds limit {}",
                space_size(&space),
                EXHAUSTIVE_LIMIT
            );
        }
    }

    // Uniform-strategy sweep (a cheap domain heuristic), written once.
    {
        let mut oracle = StaticOracle::new(factory);
        let out = baselines::uniform_sweep(&space, &mut oracle);
        write_history(&mut w, factory.name(), "uniform", 0, &out.history)?;
        println!("  uniform-sweep best = {:.4}", out.best_fitness);
    }

    let xover_only = GaConfig {
        mutation_rate: 0.0,
        ..ga_cfg.clone()
    };
    let mut_only = GaConfig {
        crossover_rate: 0.0,
        ..ga_cfg.clone()
    };

    for run in 0..runs {
        let run_seed = seed.wrapping_add(run as u64);

        macro_rules! ga_variant {
            ($cfg:expr, $name:expr) => {{
                let mut rng = ChaCha8Rng::seed_from_u64(run_seed);
                let mut oracle = StaticOracle::new(factory);
                let out = ga::run(&space, $cfg, &mut oracle, &mut rng);
                write_history(&mut w, factory.name(), $name, run, &out.history)?;
            }};
        }

        ga_variant!(ga_cfg, "ga");
        ga_variant!(&xover_only, "ga_xover_only");
        ga_variant!(&mut_only, "ga_mut_only");

        {
            let mut rng = ChaCha8Rng::seed_from_u64(run_seed);
            let mut oracle = StaticOracle::new(factory);
            let out = baselines::random_search(&space, budget, sample_every, &mut oracle, &mut rng);
            write_history(&mut w, factory.name(), "random", run, &out.history)?;
        }
        {
            let mut rng = ChaCha8Rng::seed_from_u64(run_seed);
            let mut oracle = StaticOracle::new(factory);
            let out = baselines::hill_climb(&space, budget, &mut oracle, &mut rng);
            write_history(&mut w, factory.name(), "hillclimb", run, &out.history)?;
        }
        {
            let mut rng = ChaCha8Rng::seed_from_u64(run_seed);
            let mut oracle = StaticOracle::new(factory);
            let out = baselines::greedy(&space, &mut oracle, &mut rng);
            write_history(&mut w, factory.name(), "greedy", run, &out.history)?;
        }
    }

    w.flush()?;
    println!("  wrote {path} ({runs} runs, budget {budget} evals/run)");
    Ok(())
}

/// Run NSGA-II and measure real prover cost on the resulting Pareto front,
/// alongside the all-default ("hand-written") strategy for comparison.
pub fn run_pareto(
    factory: &dyn CircuitFactory,
    nsga_cfg: &Nsga2Config,
    seed: u64,
    real_repeats: usize,
) -> anyhow::Result<()> {
    ensure_results_dir();
    let space = factory.strategy_space();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut oracle = StaticOracle::new(factory);
    let outcome = nsga2::run(&space, nsga_cfg, &mut oracle, &mut rng);

    // Hypervolume trace.
    {
        let mut w = BufWriter::new(File::create("results/pareto_hv.csv")?);
        writeln!(w, "benchmark,evaluations,hypervolume,front_size")?;
        for r in &outcome.history {
            writeln!(
                w,
                "{},{},{},{}",
                factory.name(),
                r.evaluations,
                r.hypervolume,
                r.front_size
            )?;
        }
        w.flush()?;
    }

    // Front members + the default strategy, each with measured prover cost.
    let mut w = BufWriter::new(File::create("results/pareto_front.csv")?);
    writeln!(
        w,
        "benchmark,kind,lde_size,num_gate_constraints,degree_bits,quotient_degree_factor,prove_time_ms,proof_bytes"
    )?;

    let measure = |chrom: &Chromosome, rng: &mut ChaCha8Rng| -> anyhow::Result<_> {
        let built = factory.build(chrom);
        let cost = cost_static(&built);
        let inputs = factory.random_inputs(rng);
        let real = cost_real(&built, &inputs, real_repeats)?;
        Ok((cost, real))
    };

    // Default = strategy 0 everywhere (the conventional/library choice).
    let default = Chromosome::new(vec![0u8; space.len()]);
    let (dcost, dreal) = measure(&default, &mut rng)?;
    let default_ms = dreal.prove_time.as_secs_f64() * 1e3;
    writeln!(
        w,
        "{},default,{},{},{},{},{:.3},{}",
        factory.name(),
        dcost.lde_size,
        dcost.num_gate_constraints,
        dcost.degree_bits,
        dcost.quotient_degree_factor,
        default_ms,
        dreal.proof_bytes
    )?;

    let mut best_front_ms = f64::INFINITY;
    let mut best_front_lde = usize::MAX;
    for p in &outcome.front {
        let (cost, real) = measure(&p.chrom, &mut rng)?;
        let ms = real.prove_time.as_secs_f64() * 1e3;
        best_front_ms = best_front_ms.min(ms);
        best_front_lde = best_front_lde.min(cost.lde_size);
        writeln!(
            w,
            "{},front,{},{},{},{},{:.3},{}",
            factory.name(),
            cost.lde_size,
            cost.num_gate_constraints,
            cost.degree_bits,
            cost.quotient_degree_factor,
            ms,
            real.proof_bytes
        )?;
    }
    w.flush()?;

    println!("  Pareto front: {} point(s)", outcome.front.len());
    // Primary (deterministic) result: reduction in the dominant FFT term.
    if best_front_lde < usize::MAX && dcost.lde_size > 0 {
        let ratio = dcost.lde_size as f64 / best_front_lde as f64;
        println!(
            "  static LDE size: default {} -> best front {} ({:.2}x smaller)",
            dcost.lde_size, best_front_lde, ratio
        );
    }
    // Secondary (noisy at small scale): wall-clock prover time.
    println!(
        "  wall-clock prover time (noisy at small circuit size): default {:.2} ms, best front {:.2} ms",
        default_ms, best_front_ms
    );
    println!("  wrote results/pareto_front.csv and results/pareto_hv.csv");
    Ok(())
}
