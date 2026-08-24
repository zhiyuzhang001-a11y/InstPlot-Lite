from PySide6.QtCore import QEventLoop, QTimer

from instplot_rendering import InteractiveDrawScheduler


def _wait(milliseconds):
    loop = QEventLoop()
    QTimer.singleShot(milliseconds, loop.quit)
    loop.exec()


def test_interactive_draw_scheduler_coalesces_event_burst(qapp):
    draws = []
    scheduler = InteractiveDrawScheduler(lambda: draws.append("draw"), interval_ms=10)

    for _index in range(20):
        scheduler.request()

    assert draws == []
    _wait(30)
    assert draws == ["draw"]


def test_interactive_draw_scheduler_flushes_once_and_cancels_timer(qapp):
    draws = []
    scheduler = InteractiveDrawScheduler(lambda: draws.append("draw"), interval_ms=20)

    scheduler.request()
    scheduler.flush()
    assert draws == ["draw"]
    _wait(40)
    assert draws == ["draw"]
