#!/usr/bin/env python3
"""Measure the current GUI import path on a deterministic temporary CSV."""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import sys
import tempfile
import time
import tracemalloc
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

import pandas as pd
from PySide6.QtWidgets import QApplication

import InstPlot


ROW = b"1234567,7654321\n"


def write_csv(path: Path, size_mib: int) -> tuple[int, int]:
    """Create an approximately sized, two-column CSV without retaining it in memory."""
    target_bytes = size_mib * 1024 * 1024
    row_count = 0
    written = 0
    with path.open("wb") as handle:
        header = b"x,y\n"
        handle.write(header)
        written += len(header)
        while written + len(ROW) <= target_bytes:
            handle.write(ROW)
            written += len(ROW)
            row_count += 1
    return written, row_count


def measure_load(window: InstPlot.PlotApp, path: Path) -> dict[str, float]:
    window.loaded_files.clear()
    tracemalloc.start()
    started = time.perf_counter()
    window.load_file(str(path))
    elapsed_seconds = time.perf_counter() - started
    _current, peak_bytes = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    if len(window.loaded_files) != 1:
        raise RuntimeError(window.statusBar().currentMessage())
    frame = window.loaded_files[0][1]
    if frame.shape[1] != 2:
        raise RuntimeError(f"expected two columns, got {frame.shape[1]}")
    window.loaded_files.clear()
    return {
        "elapsed_seconds": elapsed_seconds,
        "tracemalloc_peak_bytes": peak_bytes,
        "rows": int(frame.shape[0]),
        "columns": int(frame.shape[1]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--size-mib", type=int, required=True)
    parser.add_argument("--runs", type=int, required=True)
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    if args.size_mib <= 0 or args.runs <= 0:
        parser.error("--size-mib and --runs must both be positive")

    with tempfile.TemporaryDirectory(prefix="plotapp-m2-benchmark-") as temporary_directory:
        path = Path(temporary_directory) / "synthetic.csv"
        file_bytes, expected_rows = write_csv(path, args.size_mib)
        app = QApplication.instance() or QApplication([])
        window = InstPlot.PlotApp()
        try:
            measure_load(window, path)  # warm-up; intentionally excluded from measured results
            results = [measure_load(window, path) for _ in range(args.runs)]
        finally:
            window.close()
            if QApplication.instance() is app:
                app.processEvents()

    summary = {
        "benchmark": "PlotApp.load_file pre-refactor baseline",
        "python": sys.version.split()[0],
        "pandas": pd.__version__,
        "platform": platform.platform(),
        "size_mib_requested": args.size_mib,
        "file_bytes": file_bytes,
        "expected_rows": expected_rows,
        "runs": results,
        "median_elapsed_seconds": statistics.median(result["elapsed_seconds"] for result in results),
        "median_tracemalloc_peak_bytes": statistics.median(
            result["tracemalloc_peak_bytes"] for result in results
        ),
    }
    output = json.dumps(summary, ensure_ascii=False, indent=2)
    print(output)
    if args.json_output:
        args.json_output.write_text(output + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
