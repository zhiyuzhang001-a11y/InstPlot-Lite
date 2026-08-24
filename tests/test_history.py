import ast
import copy
from pathlib import Path

import pandas as pd

import InstPlot
from instplot_history import ColumnPatchCommand, HistoryManager
from scripts.benchmark_history import measure_differential_history, measure_legacy_history


SOURCE_PATH = Path(InstPlot.__file__)
EXPECTED_HISTORY_OWNERS = set()


def _deepcopy_history_owners():
    tree = ast.parse(SOURCE_PATH.read_text(encoding="utf-8"))
    parents = {}
    for node in ast.walk(tree):
        for child in ast.iter_child_nodes(node):
            parents[child] = node

    owners = set()
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        if not isinstance(node.func, ast.Attribute) or node.func.attr != "deepcopy":
            continue
        if "self.loaded_files" not in ast.unparse(node):
            continue
        names = []
        current = node
        while current in parents:
            current = parents[current]
            if isinstance(current, (ast.FunctionDef, ast.AsyncFunctionDef)):
                names.append(current.name)
            if isinstance(current, ast.ClassDef):
                names.append(current.name)
                break
        owners.add(".".join(reversed(names)))
    return owners


def test_gui_history_inventory_has_no_full_snapshots():
    assert _deepcopy_history_owners() == EXPECTED_HISTORY_OWNERS


def test_legacy_deepcopy_snapshot_preserves_paths_order_index_dtype_and_values():
    first = pd.DataFrame(
        {
            "x": pd.Series([1, 2], index=[10, 20], dtype="int64"),
            "y": pd.Series([3.5, 4.5], index=[10, 20], dtype="float64"),
        },
        index=pd.Index([10, 20], name="row"),
    )
    second = pd.DataFrame({"label": pd.Series(["a", "b"], dtype="string")})
    loaded_files = [("重复/样本.txt", first), ("另一路径/样本.txt", second)]

    snapshot = copy.deepcopy(loaded_files)
    loaded_files.reverse()
    first.loc[10, "y"] = 99.0
    second.loc[0, "label"] = "changed"

    assert [path for path, _frame in snapshot] == ["重复/样本.txt", "另一路径/样本.txt"]
    assert snapshot[0][1].index.tolist() == [10, 20]
    assert snapshot[0][1].dtypes.astype(str).tolist() == ["int64", "float64"]
    assert snapshot[0][1].to_dict("list") == {"x": [1, 2], "y": [3.5, 4.5]}
    assert snapshot[1][1]["label"].tolist() == ["a", "b"]


class _StatusBar:
    def __init__(self):
        self.message = ""

    def showMessage(self, message):
        self.message = message


class _UndoHarness:
    def __init__(self, history, loaded_files):
        self.history = history
        self.loaded_files = loaded_files
        self.combo_updates = 0
        self.replots = 0
        self._status = _StatusBar()

    def _update_combo_columns(self):
        self.combo_updates += 1

    def replot_all(self, *args, **kwargs):
        self.replots += 1

    def statusBar(self):
        return self._status

    def _refresh_history_view(self, preserve_view=False):
        self._update_combo_columns()
        self.replot_all(preserve_view=preserve_view)


def test_differential_undo_restores_last_state_and_updates_observers():
    current = [("current.txt", pd.DataFrame({"x": [3], "y": [4.0]}))]
    history = HistoryManager(current)
    history.execute(ColumnPatchCommand.create(history, 0, "y", [0], [9.0]))
    harness = _UndoHarness(history, current)

    InstPlot.PlotApp.undo(harness)

    assert harness.loaded_files[0][1]["y"].tolist() == [4.0]
    assert harness.history.undo_count == 0
    assert harness.combo_updates == 1
    assert harness.replots == 1
    assert harness._status.message == "已撤回上一步操作"


def test_legacy_undo_empty_history_is_noop():
    current = [("current.txt", pd.DataFrame({"x": [3], "y": [4]}))]
    harness = _UndoHarness(HistoryManager(current), current)

    InstPlot.PlotApp.undo(harness)

    assert harness.loaded_files is current
    assert harness.combo_updates == 0
    assert harness.replots == 0
    assert harness._status.message == "没有可撤回的操作"


def test_legacy_history_benchmark_retains_one_full_payload_per_step():
    result = measure_legacy_history(rows=20, files=2, columns=3, steps=4)

    assert result["fixture_bytes"] > 0
    assert result["step_payload_bytes"] == [result["fixture_bytes"]] * 4
    assert result["retained_payload_bytes"] == result["fixture_bytes"] * 4
    assert result["tracemalloc_peak_bytes"] >= result["retained_payload_bytes"]
    assert result["elapsed_seconds"] >= sum(result["step_seconds"])


def test_differential_history_benchmark_meets_payload_gate_and_roundtrips():
    result = measure_differential_history(rows=200, files=2, columns=8, steps=4)

    assert result["roundtrip_verified"] is True
    assert result["undo_count_after_execute"] == 4
    assert result["retained_payload_bytes"] == sum(result["step_payload_bytes"])
    assert result["legacy_payload_bytes"] == result["fixture_bytes"] * 4
    assert result["payload_ratio"] <= 0.35
