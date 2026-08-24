import threading

from PySide6.QtCore import QEventLoop, QTimer

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
    assert predicate(), "async import timed out"


def _window(qapp):
    window = InstPlot.PlotApp()
    window.loaded_files.clear()
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.plot_selected = lambda: None
    return window


def test_async_import_publishes_frame_and_columns_on_main_thread(qapp, tmp_path):
    path = tmp_path / "async.txt"
    path.write_text("x,y\n1,2\n3,4\n", encoding="utf-8")
    window = _window(qapp)
    try:
        task_id = window.load_file_async(str(path))
        assert task_id
        assert window.loaded_files == []
        _wait(lambda: len(window.loaded_files) == 1)

        assert window.loaded_files[0][1].to_dict("list") == {"x": [1, 3], "y": [2, 4]}
        assert [window.combo_x.itemText(i) for i in range(window.combo_x.count())] == ["x", "y"]
        assert window.tasks.active_count == 0
    finally:
        window.close()


def test_cancelled_async_import_discards_late_result(qapp, monkeypatch):
    window = _window(qapp)
    started = threading.Event()
    release = threading.Event()
    original = InstPlot.read_data_file

    def blocked(path):
        started.set()
        release.wait(timeout=2)
        return original(path)

    monkeypatch.setattr(InstPlot, "read_data_file", blocked)
    try:
        task_id = window.load_file_async(__file__)
        assert started.wait(timeout=2)
        assert window.tasks.cancel(task_id)
        release.set()
        _wait(lambda: window.tasks.active_count == 0)

        assert window.loaded_files == []
        assert "取消" in window.statusBar().currentMessage()
    finally:
        window.close()


def test_main_event_loop_remains_responsive_while_import_is_blocked(qapp, monkeypatch):
    window = _window(qapp)
    started = threading.Event()
    release = threading.Event()
    heartbeat = []

    def blocked(_path):
        started.set()
        release.wait(timeout=2)
        raise RuntimeError("released test task")

    monkeypatch.setattr(InstPlot, "read_data_file", blocked)
    try:
        task_id = window.load_file_async("blocked.txt")
        assert started.wait(timeout=2)
        QTimer.singleShot(0, lambda: heartbeat.append("alive"))
        _wait(lambda: bool(heartbeat))
        assert heartbeat == ["alive"]
        window.tasks.cancel(task_id)
        release.set()
        _wait(lambda: window.tasks.active_count == 0)
    finally:
        window.close()
