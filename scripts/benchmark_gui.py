#!/usr/bin/env python3
"""Deterministic offscreen baseline for GUI-adjacent PlotApp workloads."""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time
import tracemalloc
from pathlib import Path
from types import SimpleNamespace

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

import numpy as np
import pandas as pd
from PySide6.QtWidgets import QApplication

import InstPlot
from instplot_io import read_data_file
from instplot_processing import center_values, denoise_values


def _positive(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def _measure(function, runs: int, warm_up: bool = True) -> dict:
    if warm_up:
        function()
    elapsed = []
    peaks = []
    for _run in range(runs):
        tracemalloc.start()
        started = time.perf_counter()
        function()
        elapsed.append(time.perf_counter() - started)
        _current, peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()
        peaks.append(peak)
    return {
        "elapsed_seconds": elapsed,
        "tracemalloc_peak_bytes": peaks,
        "median_elapsed_seconds": statistics.median(elapsed),
        "median_tracemalloc_peak_bytes": statistics.median(peaks),
    }


def _cold_start() -> None:
    code = (
        "from PySide6.QtWidgets import QApplication; "
        "from InstPlot import PlotApp; "
        "app=QApplication.instance() or QApplication([]); "
        "window=PlotApp(); window.close(); app.quit()"
    )
    environment = os.environ.copy()
    environment["QT_QPA_PLATFORM"] = "offscreen"
    subprocess.run(
        [sys.executable, "-c", code],
        cwd=PROJECT_ROOT,
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )


def run_benchmark(rows: int, runs: int, startup_runs: int) -> dict:
    if rows <= 0 or runs <= 0 or startup_runs <= 0:
        raise ValueError("rows, runs and startup_runs must be positive")

    x = np.linspace(-5.0, 5.0, rows)
    y = 0.25 * x**3 - 2.0 * x + np.sin(x * 3.0)
    frame = pd.DataFrame({"x": x, "y": y})
    metrics = {"cold_startup": _measure(_cold_start, startup_runs, warm_up=False)}

    with tempfile.TemporaryDirectory(prefix="plotapp-m4-benchmark-") as temporary_directory:
        text_path = Path(temporary_directory) / "fixture.txt"
        frame.to_csv(text_path, sep="\t", index=False)
        metrics["text_import"] = _measure(lambda: read_data_file(text_path), runs)

        def process() -> None:
            centered = center_values(y).values
            denoise_values(centered, window_length=11, polyorder=3)

        metrics["processing"] = _measure(process, runs)
        metrics["polynomial_fit"] = _measure(lambda: np.polyfit(x, y, 3), runs)

        def rectangle_scan() -> None:
            valid = frame["x"].notna() & frame["y"].notna()
            mask = valid & frame["x"].between(-1.0, 1.0) & frame["y"].between(-2.0, 2.0)
            np.flatnonzero(mask.to_numpy())

        metrics["rectangle_scan"] = _measure(rectangle_scan, runs)

        app = QApplication.instance() or QApplication([])
        window = InstPlot.PlotApp()
        try:
            window.loaded_files = [("benchmark.tsv", frame)]
            window.history.reset(window.loaded_files)
            window.combo_x.clear()
            window.combo_y.clear()
            window.combo_x.addItems(["x", "y"])
            window.combo_y.addItems(["x", "y"])
            window.combo_x.setCurrentText("x")
            window.combo_y.setCurrentText("y")

            metrics["full_redraw"] = _measure(
                lambda: window._draw_all_files("x", "y"), runs
            )

            def pan_event() -> None:
                window.dragging = True
                window.last_mouse_pos = (100.0, 100.0)
                event = SimpleNamespace(
                    inaxes=window.ax,
                    x=104.0,
                    y=103.0,
                    xdata=0.0,
                    ydata=0.0,
                )
                window.on_mouse_drag(event)
                window.dragging = False
                window.last_mouse_pos = None

            metrics["pan_event"] = _measure(pan_event, runs)
        finally:
            window.close()
            app.processEvents()

    return {
        "benchmark": "PlotApp M4.1 offscreen baseline",
        "python": sys.version.split()[0],
        "platform": platform.platform(),
        "rows": rows,
        "runs": runs,
        "startup_runs": startup_runs,
        "metrics": metrics,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows", type=_positive, default=100_000)
    parser.add_argument("--runs", type=_positive, default=3)
    parser.add_argument("--startup-runs", type=_positive, default=3)
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    result = run_benchmark(args.rows, args.runs, args.startup_runs)
    output = json.dumps(result, ensure_ascii=False, indent=2)
    print(output)
    if args.json_output:
        args.json_output.write_text(output + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
