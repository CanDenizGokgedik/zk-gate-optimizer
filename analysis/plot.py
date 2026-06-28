#!/usr/bin/env python3
"""Plot the CSVs produced by `zk-gate-evo` into `results/*.png`.

Usage:
    python analysis/plot.py            # reads ./results, writes ./results
    python analysis/plot.py RESULTS_DIR

Requires: pandas, matplotlib (see requirements.txt).
"""
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd

RESULTS = Path(sys.argv[1] if len(sys.argv) > 1 else "results")


def convergence(df: pd.DataFrame) -> None:
    """Median best-fitness vs. evaluations, with 10-90 percentile band."""
    fig, ax = plt.subplots(figsize=(8, 5))
    for algo, g in df.groupby("algo"):
        if algo == "exhaustive":
            opt = g["best"].min()
            ax.axhline(opt, ls="--", lw=1, color="black", label="exhaustive optimum")
            continue
        # Align runs on a common evaluation grid via interpolation.
        grid = sorted(df["evaluations"].unique())
        curves = []
        for _, run in g.groupby("run"):
            # Collapse duplicate evaluation counts (best-so-far is monotone).
            s = run.groupby("evaluations")["best"].min().sort_index()
            curves.append(s.reindex(grid, method="ffill"))
        if not curves:
            continue
        m = pd.concat(curves, axis=1)
        ax.plot(grid, m.median(axis=1), label=algo, lw=1.8)
        ax.fill_between(grid, m.quantile(0.1, axis=1), m.quantile(0.9, axis=1), alpha=0.15)
    ax.set_xlabel("fitness evaluations")
    ax.set_ylabel("best fitness (lower is better)")
    ax.set_title("Convergence: GA vs. baselines")
    ax.legend()
    fig.tight_layout()
    out = RESULTS / "convergence.png"
    fig.savefig(out, dpi=150)
    print(f"wrote {out}")


def diversity(df: pd.DataFrame) -> None:
    """Population diversity over evaluations for the population-based methods."""
    pop = df[df["algo"].isin(["ga", "ga_xover_only", "ga_mut_only"])]
    pop = pop.dropna(subset=["diversity"])
    if pop.empty:
        return
    fig, ax = plt.subplots(figsize=(8, 5))
    for algo, g in pop.groupby("algo"):
        med = g.groupby("evaluations")["diversity"].median()
        ax.plot(med.index, med.values, label=algo, lw=1.8)
    ax.set_xlabel("fitness evaluations")
    ax.set_ylabel("mean pairwise Hamming diversity")
    ax.set_title("Population diversity")
    ax.legend()
    fig.tight_layout()
    out = RESULTS / "diversity.png"
    fig.savefig(out, dpi=150)
    print(f"wrote {out}")


def hypervolume() -> None:
    path = RESULTS / "pareto_hv.csv"
    if not path.exists():
        return
    df = pd.read_csv(path)
    fig, ax = plt.subplots(figsize=(8, 5))
    ax.plot(df["evaluations"], df["hypervolume"], lw=1.8)
    ax.set_xlabel("fitness evaluations")
    ax.set_ylabel("hypervolume of first front")
    ax.set_title("NSGA-II hypervolume over time")
    fig.tight_layout()
    out = RESULTS / "hypervolume.png"
    fig.savefig(out, dpi=150)
    print(f"wrote {out}")


def main() -> None:
    conv = RESULTS / "convergence.csv"
    if conv.exists():
        df = pd.read_csv(conv)
        convergence(df)
        diversity(df)
    else:
        print(f"no {conv}; run `cargo run --release -- compare` first")
    hypervolume()


if __name__ == "__main__":
    main()
