//! Command-line entry point for `zk-gate-evo`.
//!
//! Subcommands:
//! - `verify`  — check that random strategies all compute the reference relation,
//! - `compare` — GA (and ablations) vs. random / hill-climb / greedy / exhaustive,
//! - `pareto`  — NSGA-II Pareto front with measured prover cost.

use clap::{Args, Parser, Subcommand, ValueEnum};

use zk_gate_evo::circuits::exponentiation::Exponentiation;
use zk_gate_evo::circuits::range_check::RangeCheck;
use zk_gate_evo::circuits::CircuitFactory;
use zk_gate_evo::ec::ga::GaConfig;
use zk_gate_evo::ec::nsga2::Nsga2Config;
use zk_gate_evo::experiment;
use zk_gate_evo::rewrite::factory::PolyRewrite;
use zk_gate_evo::strategy::space_size;

#[derive(Parser)]
#[command(
    name = "zk-gate-evo",
    version,
    about = "Evolutionary gate-strategy optimization in Plonky2"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate that every strategy computes the same relation.
    Verify {
        #[command(flatten)]
        bench: BenchmarkArgs,
        /// Number of random (strategy, input) pairs to check.
        #[arg(long, default_value_t = 32)]
        samples: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Compare the genetic algorithm against baseline optimizers.
    Compare {
        #[command(flatten)]
        bench: BenchmarkArgs,
        #[arg(long, default_value_t = 30)]
        runs: usize,
        #[arg(long, default_value_t = 60)]
        pop: usize,
        #[arg(long, default_value_t = 80)]
        gens: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Print per-strategy static costs (diagnostic).
    Inspect {
        #[command(flatten)]
        bench: BenchmarkArgs,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Run NSGA-II and measure prover cost on the Pareto front.
    Pareto {
        #[command(flatten)]
        bench: BenchmarkArgs,
        #[arg(long, default_value_t = 60)]
        pop: usize,
        #[arg(long, default_value_t = 80)]
        gens: usize,
        /// Proof repetitions per individual when measuring prover time.
        #[arg(long, default_value_t = 5)]
        repeats: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum Benchmark {
    /// Range-check N values to a fixed bit width (base-{2,4,8} limbs).
    Range,
    /// Raise N values to a fixed exponent (gadget / naive / square-and-multiply).
    Exp,
    /// Evaluate a fixed polynomial via a sequence of semantics-preserving
    /// rewrite rules (Phase 3 — the genuinely hard, multi-modal landscape).
    Poly,
}

#[derive(Args, Clone)]
struct BenchmarkArgs {
    #[arg(long, value_enum, default_value_t = Benchmark::Exp)]
    benchmark: Benchmark,
    /// Number of independent sub-computations (one strategy gene each).
    #[arg(long, default_value_t = 16)]
    num_values: usize,
    /// Bit width for the range-check benchmark (divisible by 6).
    #[arg(long, default_value_t = 24)]
    bits: usize,
    /// Exponent for the exponentiation benchmark.
    #[arg(long, default_value_t = 7)]
    exponent: u64,
    /// Polynomial degree for the poly benchmark.
    #[arg(long, default_value_t = 12)]
    degree: usize,
    /// Rewrite-sequence length (number of rule slots) for the poly benchmark.
    #[arg(long, default_value_t = 24)]
    seq_len: usize,
}

impl BenchmarkArgs {
    fn build(&self) -> Box<dyn CircuitFactory> {
        match self.benchmark {
            Benchmark::Range => Box::new(RangeCheck::new(self.num_values, self.bits)),
            Benchmark::Exp => Box::new(Exponentiation::new(self.num_values, self.exponent)),
            Benchmark::Poly => Box::new(PolyRewrite::new(self.degree, self.seq_len)),
        }
    }
}

fn report_space(factory: &dyn CircuitFactory) {
    let space = factory.strategy_space();
    println!(
        "Benchmark '{}': {} genes, search-space size {}",
        factory.name(),
        space.len(),
        space_size(&space)
    );
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Verify {
            bench,
            samples,
            seed,
        } => {
            let factory = bench.build();
            report_space(factory.as_ref());
            let (passed, total) = experiment::run_correctness(factory.as_ref(), samples, seed)?;
            println!("Correctness: {passed}/{total} strategies verified.");
            if passed != total {
                anyhow::bail!("correctness check failed");
            }
        }
        Command::Compare {
            bench,
            runs,
            pop,
            gens,
            seed,
        } => {
            let factory = bench.build();
            report_space(factory.as_ref());
            let ga_cfg = GaConfig {
                pop_size: pop,
                generations: gens,
                ..GaConfig::default()
            };
            experiment::run_compare(factory.as_ref(), runs, &ga_cfg, seed)?;
        }
        Command::Inspect { bench, seed } => {
            let factory = bench.build();
            report_space(factory.as_ref());
            experiment::run_inspect(factory.as_ref(), seed);
        }
        Command::Pareto {
            bench,
            pop,
            gens,
            repeats,
            seed,
        } => {
            let factory = bench.build();
            report_space(factory.as_ref());
            let nsga_cfg = Nsga2Config {
                pop_size: pop,
                generations: gens,
                ..Nsga2Config::default()
            };
            experiment::run_pareto(factory.as_ref(), &nsga_cfg, seed, repeats)?;
        }
    }
    Ok(())
}
