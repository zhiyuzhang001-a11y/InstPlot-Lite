#!/usr/bin/env python3
"""Compare retained legacy snapshots with differential history commands."""

from __future__ import annotations

import argparse
import copy
import gc
import json
from pathlib import Path
import sys
import time
import tracemalloc

import numpy as np
import pandas as pd

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from instplot_history import ColumnPatchCommand, CompositeCommand, HistoryManager

try:
    import resource
except ImportError:  # pragma: no cover - unavailable on Windows
    resource = None


def _positive(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be positive")
    return parsed


def dataframe_payload_bytes(frame: pd.DataFrame) -> int:
    return int(frame.memory_usage(index=True, deep=True).sum())


def loaded_files_payload_bytes(loaded_files) -> int:
    return sum(dataframe_payload_bytes(frame) for _path, frame in loaded_files)


def build_fixture(rows: int, files: int, columns: int):
    base = np.arange(rows, dtype=np.float64)
    return [
        (
            f"fixture-{file_index}.txt",
            pd.DataFrame(
                {
                    f"column-{column_index}": base + file_index + column_index / 10.0
                    for column_index in range(columns)
                }
            ),
        )
        for file_index in range(files)
    ]


def measure_legacy_history(rows: int, files: int, columns: int, steps: int):
    loaded_files = build_fixture(rows, files, columns)
    fixture_bytes = loaded_files_payload_bytes(loaded_files)
    history = []
    step_seconds = []
    step_payload_bytes = []

    gc.collect()
    tracemalloc.start()
    started = time.perf_counter()
    for _step in range(steps):
        step_started = time.perf_counter()
        snapshot = copy.deepcopy(loaded_files)
        history.append(snapshot)
        step_seconds.append(time.perf_counter() - step_started)
        step_payload_bytes.append(loaded_files_payload_bytes(snapshot))
    elapsed_seconds = time.perf_counter() - started
    current_bytes, peak_bytes = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    process_max_rss_bytes = None
    if resource is not None:
        max_rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        process_max_rss_bytes = int(max_rss if sys.platform == "darwin" else max_rss * 1024)

    result = {
        "rows": rows,
        "files": files,
        "columns": columns,
        "steps": steps,
        "fixture_bytes": fixture_bytes,
        "step_payload_bytes": step_payload_bytes,
        "retained_payload_bytes": sum(step_payload_bytes),
        "elapsed_seconds": elapsed_seconds,
        "step_seconds": step_seconds,
        "tracemalloc_current_bytes": current_bytes,
        "tracemalloc_peak_bytes": peak_bytes,
        "process_max_rss_bytes": process_max_rss_bytes,
    }
    history.clear()
    loaded_files.clear()
    gc.collect()
    return result


def measure_differential_history(rows: int, files: int, columns: int, steps: int):
    loaded_files = build_fixture(rows, files, columns)
    fixture_bytes = loaded_files_payload_bytes(loaded_files)
    legacy_payload_bytes = fixture_bytes * steps
    history = HistoryManager(loaded_files, max_steps=steps)
    step_seconds = []
    step_payload_bytes = []

    gc.collect()
    tracemalloc.start()
    started = time.perf_counter()
    for _step in range(steps):
        step_started = time.perf_counter()
        commands = []
        for file_position, (_path, frame) in enumerate(loaded_files):
            values = frame.iloc[:, 0].to_numpy(copy=True) + 1.0
            commands.append(
                ColumnPatchCommand.create(
                    history,
                    file_position,
                    frame.columns[0],
                    range(rows),
                    values,
                )
            )
        command = CompositeCommand(commands)
        history.execute(command)
        step_seconds.append(time.perf_counter() - step_started)
        step_payload_bytes.append(command.payload_bytes)
    elapsed_seconds = time.perf_counter() - started
    current_bytes, peak_bytes = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    undo_count_after_execute = history.undo_count
    for _step in range(steps):
        if not history.undo():
            raise AssertionError("differential history ended before all undo steps")
    undo_verified = all(
        np.array_equal(
            frame.iloc[:, 0].to_numpy(),
            np.arange(rows, dtype=np.float64) + file_position,
        )
        for file_position, (_path, frame) in enumerate(loaded_files)
    )
    for _step in range(steps):
        if not history.redo():
            raise AssertionError("differential history ended before all redo steps")
    redo_verified = all(
        np.array_equal(
            frame.iloc[:, 0].to_numpy(),
            np.arange(rows, dtype=np.float64) + file_position + steps,
        )
        for file_position, (_path, frame) in enumerate(loaded_files)
    )
    retained_payload_bytes = sum(step_payload_bytes)
    process_max_rss_bytes = None
    if resource is not None:
        max_rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        process_max_rss_bytes = int(max_rss if sys.platform == "darwin" else max_rss * 1024)
    return {
        "rows": rows,
        "files": files,
        "columns": columns,
        "steps": steps,
        "fixture_bytes": fixture_bytes,
        "legacy_payload_bytes": legacy_payload_bytes,
        "step_payload_bytes": step_payload_bytes,
        "retained_payload_bytes": retained_payload_bytes,
        "payload_ratio": retained_payload_bytes / legacy_payload_bytes,
        "elapsed_seconds": elapsed_seconds,
        "step_seconds": step_seconds,
        "tracemalloc_current_bytes": current_bytes,
        "tracemalloc_peak_bytes": peak_bytes,
        "process_max_rss_bytes": process_max_rss_bytes,
        "undo_count_after_execute": undo_count_after_execute,
        "roundtrip_verified": undo_verified and redo_verified,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rows", type=_positive, default=250_000)
    parser.add_argument("--files", type=_positive, default=4)
    parser.add_argument("--columns", type=_positive, default=8)
    parser.add_argument("--steps", type=_positive, default=10)
    args = parser.parse_args()
    print(json.dumps(measure_differential_history(args.rows, args.files, args.columns, args.steps), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
