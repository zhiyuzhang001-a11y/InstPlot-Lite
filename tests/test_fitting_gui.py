import inspect
import threading
import time

import numpy as np
import pandas as pd
from PySide6.QtCore import QEventLoop, QTimer
from PySide6.QtWidgets import QComboBox, QDialog, QLabel, QPushButton, QSpinBox

import InstPlot


def _wait(predicate, timeout_ms=3000):
    loop = QEventLoop()
    timeout = QTimer()
    timeout.setSingleShot(True)
    timeout.timeout.connect(loop.quit)
    poll = QTimer()
    poll.timeout.connect(lambda: loop.quit() if predicate() else None)
    poll.start(1)
    timeout.start(timeout_ms)
    loop.exec()
    poll.stop()
    assert predicate(), "fit callback timed out"


def test_fit_dialog_routes_all_solvers_through_pure_core():
    source = inspect.getsource(InstPlot.PlotApp.open_fit_dialog)
    assert "fit_values(" in source
    assert "curve_fit(" not in source
    assert "np.polyfit(" not in source
    assert "ast.parse(" not in source


def test_polynomial_fit_dialog_publishes_core_result(qapp, monkeypatch):
    window = InstPlot.PlotApp()
    window.loaded_files = [
        ("curve.txt", pd.DataFrame({"x": [-2, -1, 0, 1, 2], "y": [4, 5.5, 4, 2.5, 4]}))
    ]
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.combo_x.addItems(["x", "y"])
    window.combo_y.addItems(["x", "y"])
    window.combo_x.setCurrentText("x")
    window.combo_y.setCurrentText("y")
    observed = {}

    def execute(dialog):
        method = next(
            combo for combo in dialog.findChildren(QComboBox) if combo.findText("多项式") >= 0
        )
        method.setCurrentText("多项式")
        dialog.findChild(QSpinBox).setValue(3)
        button = next(
            child for child in dialog.findChildren(QPushButton) if child.text() == "执行拟合"
        )
        button.click()
        label = next(
            child
            for child in dialog.findChildren(QLabel)
            if child.text().startswith("拟合方程：")
        )
        observed["text"] = label.text()
        return QDialog.Rejected

    monkeypatch.setattr(InstPlot.QDialog, "exec", execute)
    try:
        window.open_fit_dialog()
        assert "R² = 1.000000" in observed["text"]
        assert "点数: 5" in observed["text"]
    finally:
        window.close()


def test_large_fit_uses_worker_and_keeps_event_loop_responsive(qapp, monkeypatch):
    x = np.linspace(0.2, 4.0, 250_000)
    frame = pd.DataFrame({"x": x, "y": 1.8 * x**1.4})
    window = InstPlot.PlotApp()
    window.loaded_files = [("large.txt", frame)]
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.combo_x.addItems(["x", "y"])
    window.combo_y.addItems(["x", "y"])
    window.combo_x.setCurrentText("x")
    window.combo_y.setCurrentText("y")
    started = threading.Event()
    release = threading.Event()
    heartbeat = []
    original = InstPlot.fit_values

    def blocked(*args, **kwargs):
        started.set()
        release.wait(timeout=2)
        return original(*args, **kwargs)

    monkeypatch.setattr(InstPlot, "fit_values", blocked)

    def execute(dialog):
        method = next(
            combo for combo in dialog.findChildren(QComboBox) if combo.findText("幂函数 (y=a*x^b)") >= 0
        )
        method.setCurrentText("幂函数 (y=a*x^b)")
        button = next(
            child for child in dialog.findChildren(QPushButton) if child.text() == "执行拟合"
        )
        started_at = time.perf_counter()
        button.click()
        assert time.perf_counter() - started_at < 0.5
        assert started.wait(timeout=2)
        QTimer.singleShot(0, lambda: heartbeat.append("alive"))
        _wait(lambda: bool(heartbeat))
        release.set()
        _wait(
            lambda: any(
                label.text().startswith("拟合方程：") for label in dialog.findChildren(QLabel)
            )
        )
        return QDialog.Rejected

    monkeypatch.setattr(InstPlot.QDialog, "exec", execute)
    try:
        window.open_fit_dialog()
        assert heartbeat == ["alive"]
        assert window.tasks.active_count == 0
    finally:
        release.set()
        window.close()
