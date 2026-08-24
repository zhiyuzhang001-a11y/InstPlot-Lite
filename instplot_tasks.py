"""Qt thread-pool task lifecycle with cooperative cancellation."""

from __future__ import annotations

from dataclasses import dataclass
import threading
import uuid
from typing import Any, Callable

from PySide6.QtCore import QObject, QRunnable, QThreadPool, Signal, Slot


class TaskCancelled(RuntimeError):
    pass


class CancellationToken:
    def __init__(self):
        self._event = threading.Event()

    @property
    def is_cancelled(self) -> bool:
        return self._event.is_set()

    def cancel(self) -> None:
        self._event.set()

    def raise_if_cancelled(self) -> None:
        if self.is_cancelled:
            raise TaskCancelled("task was cancelled")


class _TaskSignals(QObject):
    succeeded = Signal(str, object)
    failed = Signal(str, object)
    cancelled = Signal(str)


class _TaskRunnable(QRunnable):
    def __init__(self, task_id: str, function: Callable, token: CancellationToken, signals):
        super().__init__()
        self.task_id = task_id
        self.function = function
        self.token = token
        self.signals = signals

    @Slot()
    def run(self) -> None:
        try:
            self.token.raise_if_cancelled()
            result = self.function(self.token)
            self.token.raise_if_cancelled()
        except TaskCancelled:
            self.signals.cancelled.emit(self.task_id)
        except Exception as error:
            self.signals.failed.emit(self.task_id, error)
        else:
            self.signals.succeeded.emit(self.task_id, result)


@dataclass
class _TaskRecord:
    token: CancellationToken
    on_success: Callable[[Any], None] | None
    on_failure: Callable[[Exception], None] | None
    on_cancelled: Callable[[], None] | None


class TaskController(QObject):
    def __init__(self, parent: QObject | None = None, max_threads: int | None = None):
        super().__init__(parent)
        self.pool = QThreadPool(self)
        if max_threads is not None:
            if max_threads < 1:
                raise ValueError("max_threads must be positive")
            self.pool.setMaxThreadCount(max_threads)
        self._signals = _TaskSignals(self)
        self._signals.succeeded.connect(self._on_success)
        self._signals.failed.connect(self._on_failure)
        self._signals.cancelled.connect(self._on_cancelled)
        self._tasks: dict[str, _TaskRecord] = {}

    @property
    def active_count(self) -> int:
        return len(self._tasks)

    def submit(
        self,
        function: Callable[[CancellationToken], Any],
        *,
        on_success: Callable[[Any], None] | None = None,
        on_failure: Callable[[Exception], None] | None = None,
        on_cancelled: Callable[[], None] | None = None,
    ) -> str:
        task_id = uuid.uuid4().hex
        token = CancellationToken()
        self._tasks[task_id] = _TaskRecord(token, on_success, on_failure, on_cancelled)
        self.pool.start(_TaskRunnable(task_id, function, token, self._signals))
        return task_id

    def cancel(self, task_id: str) -> bool:
        record = self._tasks.get(task_id)
        if record is None:
            return False
        record.token.cancel()
        return True

    def cancel_all(self) -> None:
        for record in self._tasks.values():
            record.token.cancel()

    def wait_for_done(self, timeout_ms: int = -1) -> bool:
        return bool(self.pool.waitForDone(timeout_ms))

    def _take(self, task_id: str) -> _TaskRecord | None:
        return self._tasks.pop(task_id, None)

    @Slot(str, object)
    def _on_success(self, task_id: str, result: Any) -> None:
        record = self._take(task_id)
        if record is None:
            return
        if record.token.is_cancelled:
            if record.on_cancelled is not None:
                record.on_cancelled()
        elif record.on_success is not None:
            record.on_success(result)

    @Slot(str, object)
    def _on_failure(self, task_id: str, error: Exception) -> None:
        record = self._take(task_id)
        if record is None:
            return
        if record.token.is_cancelled:
            if record.on_cancelled is not None:
                record.on_cancelled()
        elif record.on_failure is not None:
            record.on_failure(error)

    @Slot(str)
    def _on_cancelled(self, task_id: str) -> None:
        record = self._take(task_id)
        if record is not None and record.on_cancelled is not None:
            record.on_cancelled()
