import ast
import inspect

import numpy as np
import pandas as pd

import InstPlot
from instplot_history import HistoryManager


def _window(qapp, loaded_files):
    window = InstPlot.PlotApp()
    window.loaded_files = loaded_files
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.replot_all = lambda *args, **kwargs: None
    return window


def test_gui_uses_one_history_manager_redo_action_and_no_legacy_snapshots(qapp):
    window = InstPlot.PlotApp()
    try:
        assert isinstance(window.history, HistoryManager)
        assert window.history.loaded_files is window.loaded_files
        assert any(action.text() == "重做" for action in window.toolbar.actions())
        source = inspect.getsource(InstPlot)
        tree = ast.parse(source)
        legacy = [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "deepcopy"
            and "self.loaded_files" in ast.unparse(node)
        ]
        assert legacy == []
    finally:
        window.close()


def test_center_partial_success_is_one_step_and_roundtrips_without_inplace_publish(qapp):
    valid = pd.DataFrame({"x": [0, 1, 2], "y": [1.0, 3.0, 5.0]})
    original = valid.copy(deep=True)
    missing = pd.DataFrame({"x": [0, 1, 2], "other": [4.0, 5.0, 6.0]})
    window = _window(qapp, [("valid.txt", valid), ("missing.txt", missing)])
    window.combo_y.addItem("y")
    try:
        window.apply_center()

        pd.testing.assert_frame_equal(valid, original)
        np.testing.assert_allclose(window.loaded_files[0][1]["y"], [-2.0, 0.0, 2.0])
        assert window.history.undo_count == 1
        window.undo()
        pd.testing.assert_frame_equal(window.loaded_files[0][1], original)
        window.redo()
        np.testing.assert_allclose(window.loaded_files[0][1]["y"], [-2.0, 0.0, 2.0])
    finally:
        window.close()


def test_noop_and_all_failed_processing_do_not_consume_history(qapp):
    constant = pd.DataFrame({"x": [0, 1], "y": [-1.0, 1.0]})
    window = _window(qapp, [("constant.txt", constant)])
    window.combo_y.addItem("y")
    try:
        window.apply_center()
        assert window.history.undo_count == 0

        window.loaded_files = [("invalid.txt", pd.DataFrame({"x": [0, 1], "y": [np.nan, np.nan]}))]
        window.history.reset(window.loaded_files)
        window.apply_center()
        assert window.history.undo_count == 0
    finally:
        window.close()


def test_row_and_file_commit_helpers_handle_duplicate_identity_and_refresh(qapp):
    first = pd.DataFrame({"x": [0, 1, 2], "y": [1.0, 2.0, 3.0]}, index=[5, 5, 9])
    second = pd.DataFrame({"x": [3], "y": [4.0]})
    window = _window(qapp, [("same.txt", first), ("same.txt", second)])
    try:
        assert window._commit_row_deletions({0: [1]}, preserve_view=True)
        assert window.loaded_files[0][1].index.tolist() == [5, 9]
        window.undo()
        assert window.loaded_files[0][1].index.tolist() == [5, 5, 9]

        assert window._commit_file_deletions([0])
        assert window.loaded_files == [("same.txt", second)]
        window.undo()
        assert window.loaded_files[0][1] is not second
        assert window.loaded_files[1][1] is second
    finally:
        window.close()


def test_new_data_session_clears_undo_and_redo(qapp):
    frame = pd.DataFrame({"x": [0, 1], "y": [1.0, 3.0]})
    window = _window(qapp, [("a.txt", frame)])
    window.combo_y.addItem("y")
    try:
        window.apply_center()
        window.undo()
        assert window.history.redo_count == 1
        window.clear_plot()
        assert window.history.undo_count == 0
        assert window.history.redo_count == 0
        assert window.history.loaded_files is window.loaded_files
    finally:
        window.close()


def test_filtered_processing_targets_positions_with_duplicate_index(qapp):
    frame = pd.DataFrame(
        {"x": [0, 1, 2, 3], "y": [10.0, 2.0, 6.0, 20.0]}, index=[5, 5, 9, 12]
    )
    window = _window(qapp, [("duplicates.txt", frame)])
    window.combo_y.addItem("y")
    window.row_filter_enabled = True
    window.row_filter_mode = "custom"
    window.row_filter_custom_slice = "1:3"
    try:
        window.apply_center()

        assert window.loaded_files[0][1]["y"].tolist() == [10.0, -2.0, 2.0, 20.0]
        window.undo()
        pd.testing.assert_frame_equal(window.loaded_files[0][1], frame)
        window.redo()
        assert window.loaded_files[0][1]["y"].tolist() == [10.0, -2.0, 2.0, 20.0]
    finally:
        window.close()
