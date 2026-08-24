import copy
import inspect

import numpy as np
import pandas as pd
import pytest

from instplot_history import (
    ColumnPatchCommand,
    CompositeCommand,
    DeleteFilesCommand,
    DeleteRowsCommand,
    HistoryError,
    HistoryManager,
)


def _frame(values=(1.0, 2.0, 3.0, 4.0)):
    return pd.DataFrame(
        {
            "x": [10, 20, 30, 40],
            "y": values,
            "label": ["a", "b", "c", "d"],
        },
        index=pd.Index([5, 5, 9, 12], name="row"),
    ).astype({"x": "int64", "y": "float64", "label": "string"})


def test_history_core_has_no_gui_plot_or_app_imports():
    source = inspect.getsource(__import__("instplot_history"))
    assert "PySide6" not in source
    assert "matplotlib" not in source
    assert "InstPlot" not in source


def test_column_patch_roundtrip_preserves_index_columns_dtype_and_values():
    frame = _frame()
    original = frame.copy(deep=True)
    loaded = [("sample.txt", frame)]
    history = HistoryManager(loaded)
    command = ColumnPatchCommand.create(history, 0, "y", [1, 3], [20.5, 40.5])

    assert history.execute(command)
    np.testing.assert_allclose(loaded[0][1]["y"], [1.0, 20.5, 3.0, 40.5])
    assert history.undo()
    pd.testing.assert_frame_equal(loaded[0][1], original)
    assert history.redo()
    np.testing.assert_allclose(loaded[0][1]["y"], [1.0, 20.5, 3.0, 40.5])
    assert loaded[0][1].index.tolist() == [5, 5, 9, 12]
    assert loaded[0][1].dtypes.astype(str).tolist() == ["int64", "float64", "string"]


def test_column_patch_targets_one_entry_with_duplicate_path_and_shared_frame():
    shared = _frame()
    loaded = [("same.txt", shared), ("same.txt", shared)]
    history = HistoryManager(loaded)
    command = ColumnPatchCommand.create(history, 1, "y", [0], [99.0])

    history.execute(command)

    assert loaded[0][1] is shared
    assert loaded[0][1].iloc[0]["y"] == 1.0
    assert loaded[1][1] is not shared
    assert loaded[1][1].iloc[0]["y"] == 99.0
    history.undo()
    pd.testing.assert_frame_equal(loaded[1][1], shared)


def test_column_patch_noop_does_not_consume_history_and_payload_is_delta_only():
    frame = _frame()
    loaded = [("sample.txt", frame)]
    history = HistoryManager(loaded)
    command = ColumnPatchCommand.create(history, 0, "y", [1], [2.0])

    assert not history.execute(command)
    assert history.undo_count == 0
    assert command.payload_bytes < frame.memory_usage(index=True, deep=True).sum()


def test_delete_rows_roundtrip_handles_noncontiguous_positions_and_duplicate_index():
    frame = _frame()
    original = frame.copy(deep=True)
    loaded = [("rows.txt", frame)]
    history = HistoryManager(loaded)
    command = DeleteRowsCommand.create(history, 0, [0, 2])

    history.execute(command)
    assert loaded[0][1].index.tolist() == [5, 12]
    assert loaded[0][1]["label"].tolist() == ["b", "d"]
    history.undo()
    pd.testing.assert_frame_equal(loaded[0][1], original)
    history.redo()
    assert loaded[0][1].index.tolist() == [5, 12]


def test_delete_all_rows_roundtrip_and_empty_delete_noop():
    frame = _frame()
    original = frame.copy(deep=True)
    loaded = [("rows.txt", frame)]
    history = HistoryManager(loaded)

    assert not history.execute(DeleteRowsCommand.create(history, 0, []))
    command = DeleteRowsCommand.create(history, 0, [0, 1, 2, 3])
    assert history.execute(command)
    assert loaded[0][1].empty
    history.undo()
    pd.testing.assert_frame_equal(loaded[0][1], original)


def test_delete_files_roundtrip_preserves_order_references_and_duplicate_paths():
    first, second, third = _frame(), _frame((5.0, 6.0, 7.0, 8.0)), _frame((9.0, 10.0, 11.0, 12.0))
    loaded = [("same.txt", first), ("middle.txt", second), ("same.txt", third)]
    history = HistoryManager(loaded)
    command = DeleteFilesCommand.create(history, [0, 2])

    history.execute(command)
    assert loaded == [("middle.txt", second)]
    history.undo()
    assert [path for path, _ in loaded] == ["same.txt", "middle.txt", "same.txt"]
    assert loaded[0][1] is first and loaded[1][1] is second and loaded[2][1] is third
    history.redo()
    assert loaded == [("middle.txt", second)]
    assert command.payload_bytes < first.memory_usage(index=True, deep=True).sum()


def test_composite_command_is_one_history_step_for_multiple_files():
    loaded = [("a.txt", _frame()), ("b.txt", _frame())]
    history = HistoryManager(loaded)
    command = CompositeCommand(
        [
            ColumnPatchCommand.create(history, 0, "y", [0], [10.0]),
            ColumnPatchCommand.create(history, 1, "y", [1], [20.0]),
        ]
    )

    history.execute(command)
    assert history.undo_count == 1
    assert loaded[0][1]["y"].tolist() == [10.0, 2.0, 3.0, 4.0]
    assert loaded[1][1]["y"].tolist() == [1.0, 20.0, 3.0, 4.0]
    history.undo()
    assert loaded[0][1]["y"].tolist() == [1.0, 2.0, 3.0, 4.0]
    assert loaded[1][1]["y"].tolist() == [1.0, 2.0, 3.0, 4.0]


class _InjectedFailure:
    payload_bytes = 0
    is_noop = False

    def redo(self, _history):
        raise HistoryError("injected_failure", "deterministic test failure")

    def undo(self, _history):
        raise AssertionError("failed command must never be undone")


def test_composite_execute_failure_rolls_back_visible_state_and_stacks():
    frame = _frame()
    original = frame.copy(deep=True)
    loaded = [("a.txt", frame)]
    history = HistoryManager(loaded)
    command = CompositeCommand(
        [ColumnPatchCommand.create(history, 0, "y", [0], [10.0]), _InjectedFailure()]
    )

    with pytest.raises(HistoryError, match="injected_failure"):
        history.execute(command)

    pd.testing.assert_frame_equal(loaded[0][1], original)
    assert history.undo_count == 0 and history.redo_count == 0


def test_history_limit_ten_step_roundtrip_and_new_execute_clears_redo():
    loaded = [("a.txt", _frame())]
    history = HistoryManager(loaded, max_steps=10)
    for value in range(1, 13):
        command = ColumnPatchCommand.create(history, 0, "y", [0], [float(value)])
        history.execute(command)

    assert history.undo_count == 10
    for _ in range(10):
        assert history.undo()
    assert loaded[0][1].iloc[0]["y"] == 2.0
    for _ in range(10):
        assert history.redo()
    assert loaded[0][1].iloc[0]["y"] == 12.0
    history.undo()
    history.execute(ColumnPatchCommand.create(history, 0, "y", [0], [100.0]))
    assert history.redo_count == 0


def test_state_drift_is_named_and_reset_clears_both_stacks():
    loaded = [("a.txt", _frame())]
    history = HistoryManager(loaded)
    command = ColumnPatchCommand.create(history, 0, "y", [0], [5.0])
    loaded[0][1].iloc[1, loaded[0][1].columns.get_loc("y")] = 200.0

    with pytest.raises(HistoryError) as raised:
        history.execute(command)
    assert raised.value.code == "state_mismatch"

    history.reset(loaded)
    history.execute(ColumnPatchCommand.create(history, 0, "y", [0], [5.0]))
    history.undo()
    assert history.redo_count == 1
    history.reset(loaded)
    assert history.undo_count == 0 and history.redo_count == 0


@pytest.mark.parametrize(
    "factory",
    [
        lambda history: ColumnPatchCommand.create(history, 0, "missing", [0], [1.0]),
        lambda history: ColumnPatchCommand.create(history, 0, "y", [99], [1.0]),
        lambda history: DeleteRowsCommand.create(history, 0, [99]),
        lambda history: DeleteFilesCommand.create(history, [99]),
    ],
)
def test_invalid_targets_have_named_errors(factory):
    history = HistoryManager([("a.txt", _frame())])
    with pytest.raises(HistoryError):
        factory(history)
