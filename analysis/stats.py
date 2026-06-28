#!/usr/bin/env python3
"""Statistical significance of the GA's advantage.

Reads `results/convergence.csv` and, for each baseline, runs a one-sided paired
Wilcoxon signed-rank test of "GA final gate count < baseline final gate count".
Runs are paired by seed (the `compare` command uses the same seed across all
algorithms in a given run), so the signed-rank test is the appropriate choice.

Usage:
    python analysis/stats.py [RESULTS_DIR]

Requires: pandas, scipy.
"""
import statistics
import sys
from pathlib import Path

import pandas as pd
from scipy.stats import wilcoxon

RESULTS = Path(sys.argv[1] if len(sys.argv) > 1 else "results")


def finals(df: pd.DataFrame, algo: str) -> dict:
    g = df[df.algo == algo]
    return {
        run: grp.sort_values("evaluations")["best"].iloc[-1]
        for run, grp in g.groupby("run")
    }


def main() -> None:
    df = pd.read_csv(RESULTS / "convergence.csv")
    ga = finals(df, "ga")
    print(
        f"{'baseline':16s} {'runs':>4s} {'GA med':>7s} {'base med':>8s} "
        f"{'GA wins':>8s} {'p (GA<base)':>12s}"
    )
    for b in ["ga_xover_only", "ga_mut_only", "random", "hillclimb", "greedy"]:
        bf = finals(df, b)
        common = sorted(set(ga) & set(bf))
        if not common:
            continue
        a = [ga[r] for r in common]
        c = [bf[r] for r in common]
        wins = sum(1 for x, y in zip(a, c) if x < y)
        try:
            _, p = wilcoxon(a, c, alternative="less")
            pstr = f"{p:.2e}"
        except ValueError:
            pstr = "n/a"
        print(
            f"{b:16s} {len(common):4d} {statistics.median(a):7.2f} "
            f"{statistics.median(c):8.2f} {wins:5d}/{len(common):<2d} {pstr:>12s}"
        )


if __name__ == "__main__":
    main()
