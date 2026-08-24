import threading

from PySide6.QtCore import QEventLoop, QThread, QTimer

from instplot_tasks import TaskController


def _wait(qapp, predicate, timeout_ms=3000):
    loop = QEventLoop()
    timer = QTimer()
    timer.setSingleShot(True)
    timer.timeout.connect(loop.quit)
    poll = QTimer()
    poll.timeout.connect(lambda: loop.quit() if predicate() else None)
    poll.start(1)
    timer.start(timeout_ms)
    loop.exec()
    poll.stop()
    assert predicate(), "task callback timed out"


def test_success_runs_off_main_thread_but_publishes_on_main_thread(qapp):
    controller = TaskController()
    observed = {}
    main_thread = QThread.currentThread()

    def work(token):
        token.raise_if_cancelled()
        return QThread.currentThread()

    controller.submit(
        work,
        on_success=lambda worker_thread: observed.update(
            worker_thread=worker_thread, callback_thread=QThread.currentThread()
        ),
    )
    _wait(qapp, lambda: "callback_thread" in observed)

    assert observed["worker_thread"] is not main_thread
    assert observed["callback_thread"] is main_thread
    assert controller.active_count == 0


def test_failure_is_delivered_without_success(qapp):
    controller = TaskController()
    observed = []

    def fail(_token):
        raise ValueError("deterministic failure")

    controller.submit(
        fail,
        on_success=lambda _result: observed.append("success"),
        on_failure=lambda error: observed.append((type(error).__name__, str(error))),
    )
    _wait(qapp, lambda: bool(observed))
    assert observed == [("ValueError", "deterministic failure")]


def test_cancel_discards_late_result_and_emits_cancelled(qapp):
    controller = TaskController()
    started = threading.Event()
    release = threading.Event()
    observed = []

    def work(token):
        started.set()
        release.wait(timeout=2)
        token.raise_if_cancelled()
        return "late"

    task_id = controller.submit(
        work,
        on_success=lambda result: observed.append(result),
        on_cancelled=lambda: observed.append("cancelled"),
    )
    assert started.wait(timeout=2)
    assert controller.cancel(task_id)
    release.set()
    _wait(qapp, lambda: bool(observed))

    assert observed == ["cancelled"]
    assert controller.active_count == 0
    assert controller.wait_for_done(1000)


def test_cancel_all_also_completes_queued_tasks(qapp):
    controller = TaskController(max_threads=1)
    started = threading.Event()
    release = threading.Event()
    observed = []

    def blocking(token):
        started.set()
        release.wait(timeout=2)
        token.raise_if_cancelled()

    controller.submit(blocking, on_cancelled=lambda: observed.append("first"))
    controller.submit(lambda token: token.raise_if_cancelled(), on_cancelled=lambda: observed.append("second"))
    assert started.wait(timeout=2)
    controller.cancel_all()
    release.set()
    _wait(qapp, lambda: len(observed) == 2)

    assert sorted(observed) == ["first", "second"]
    assert controller.active_count == 0
    assert controller.wait_for_done(1000)
