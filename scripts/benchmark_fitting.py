#!/usr/bin/env python3
"""Benchmark every GUI-supported fit through the shared pure fitting core."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics
import sys
import time
import tracemalloc

import numpy as np

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from instplot_fitting import fit_values


def _positive(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def _measure(function, runs: int) -> dict:
    function()
    elapsed, peaks, r2_values = [], [], []
    for _run in range(runs):
        tracemalloc.start()
        started = time.perf_counter()
        result = function()
        elapsed.append(time.perf_counter() - started)
        _current, peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()
        peaks.append(peak)
        r2_values.append(result.r2)
    return {
        "elapsed_seconds": elapsed,
        "tracemalloc_peak_bytes": peaks,
        "median_elapsed_seconds": statistics.median(elapsed),
        "median_tracemalloc_peak_bytes": statistics.median(peaks),
        "minimum_r2": min(r2_values),
    }


def run_benchmark(rows: int, runs: int) -> dict:
    if rows <= 3 or runs <= 0:
        raise ValueError("rows must exceed 3 and runs must be positive")
    signed_x = np.linspace(-2.0, 2.0, rows)
    positive_x = np.linspace(0.2, 4.0, rows)
    cases = {
        "polynomial": lambda: fit_values(
            signed_x, 0.5 * signed_x**3 - 2.0 * signed_x + 4.0, "polynomial", degree=3
        ),
        "exponential": lambda: fit_values(
            positive_x, 1.5 * np.exp(0.7 * positive_x), "exponential"
        ),
        "logarithmic": lambda: fit_values(
            positive_x, 2.2 * np.log(positive_x) - 0.3, "logarithmic"
        ),
        "power": lambda: fit_values(positive_x, 1.8 * positive_x**1.4, "power"),
        "custom": lambda: fit_values(
            signed_x,
            2.0 * np.sin(1.0 * signed_x + 0.2),
            "custom",
            expression="a * sin(b * x + c)",
            initial_parameters=[2.0, 1.0, 0.0],
        ),
    }
    return {
        "benchmark": "PlotApp M4.3 fitting matrix",
        "python": sys.version.split()[0],
        "rows": rows,
        "runs": runs,
        "methods": {name: _measure(function, runs) for name, function in cases.items()},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows", type=_positive, default=100_000)
    parser.add_argument("--runs", type=_positive, default=3)
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    result = run_benchmark(args.rows, args.runs)
    output = json.dumps(result, ensure_ascii=False, indent=2)
    print(output)
    if args.json_output:
        args.json_output.write_text(output + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
