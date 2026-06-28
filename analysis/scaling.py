#!/usr/bin/env python3
"""Scaling study: how the GA's advantage over baselines grows with problem size.

Runs the `compare` experiment for several polynomial degrees on the
rewrite-sequence benchmark and plots the median final gate count per optimizer
against degree. Writes `results/scaling.csv` and `results/scaling.png`.

Usage:
    python analysis/scaling.py

Requires: pandas, matplotlib (see requirements.txt). Invokes `cargo run`.
"""
import subprocess
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd

DEGREES = [6, 10, 14, 18, 22]
RUNS = 6
ALGOS = ["uniform", "ga", "random", "hillclimb", "greedy"]
RESULTS = Path("results")


def median_final_per_algo(df: pd.DataFrame) -> dict:
    out: dict = {}
    for (algo, _run), g in df.groupby(["algo", "run"]):
        best = g.sort_values("evaluations")["best"].iloc[-1]
        out.setdefault(algo, []).append(best)
    return {a: pd.Series(v).median() for a, v in out.items()}


def main() -> None:
    rows = []
    for d in DEGREES:
        print(f"running degree {d} ...", flush=True)
        subprocess.run(
            [
                "cargo", "run", "--release", "-q", "--", "compare",
                "--benchmark", "poly", "--degree", str(d),
                "--seq-len", str(2 * d), "--runs", str(RUNS),
                "--pop", "30", "--gens", "25",
            ],
            check=True,
        )
        med = median_final_per_algo(pd.read_csv(RESULTS / "convergence.csv"))
        for algo in ALGOS:
            if algo in med:
                rows.append({"degree": d, "algo": algo, "median_gates": med[algo]})

    scaling = pd.DataFrame(rows)
    scaling.to_csv(RESULTS / "scaling.csv", index=False)

    fig, ax = plt.subplots(figsize=(8, 5))
    for algo, g in scaling.groupby("algo"):
        g = g.sort_values("degree")
        ax.plot(g["degree"], g["median_gates"], marker="o", lw=1.8, label=algo)
    ax.set_xlabel("polynomial degree (problem size)")
    ax.set_ylabel("median final gate count (lower is better)")
    ax.set_title("Scaling: GA vs. baselines and the uniform heuristic")
    ax.legend()
    fig.tight_layout()
    fig.savefig(RESULTS / "scaling.png", dpi=150)
    print("wrote results/scaling.png and results/scaling.csv")


if __name__ == "__main__":
    main()
