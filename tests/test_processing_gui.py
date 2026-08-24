import numpy as np
import pandas as pd

import InstPlot


def _window(qapp):
    window = InstPlot.PlotApp()
    window.loaded_files.clear()
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.replot_all = lambda *args, **kwargs: None
    return window


def test_center_gui_keeps_partial_success_and_differential_history_boundary(qapp):
    window = _window(qapp)
    valid = pd.DataFrame({"x": [0, 1, 2], "y": [1.0, 3.0, 5.0]})
    missing = pd.DataFrame({"x": [0, 1, 2], "other": [4.0, 5.0, 6.0]})
    window.loaded_files = [("valid.txt", valid), ("missing.txt", missing)]
    window.history.reset(window.loaded_files)
    window.combo_y.addItem("y")
    window.combo_y.setCurrentText("y")
    try:
        window.apply_center()

        np.testing.assert_allclose(window.loaded_files[0][1]["y"], [-2.0, 0.0, 2.0])
        assert missing.columns.tolist() == ["x", "other"]
        assert window.history.undo_count == 1
        assert "对称处理完成" in window.statusBar().currentMessage()
    finally:
        window.close()


def test_center_gui_continues_after_named_core_error(qapp):
    window = _window(qapp)
    invalid = pd.DataFrame({"x": [0, 1], "y": [np.nan, np.nan]})
    valid = pd.DataFrame({"x": [0, 1], "y": [2.0, 4.0]})
    window.loaded_files = [("invalid.txt", invalid), ("valid.txt", valid)]
    window.history.reset(window.loaded_files)
    window.combo_y.addItem("y")
    window.combo_y.setCurrentText("y")
    try:
        window.apply_center()

        np.testing.assert_allclose(invalid["y"], [np.nan, np.nan], equal_nan=True)
        np.testing.assert_allclose(window.loaded_files[1][1]["y"], [-1.0, 1.0])
        assert window.history.undo_count == 1
        assert "对称处理完成" in window.statusBar().currentMessage()
    finally:
        window.close()
