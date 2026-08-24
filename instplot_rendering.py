"""Small Qt rendering helpers used by PlotApp interactions."""

from __future__ import annotations

from collections.abc import Callable

from PySide6.QtCore import QObject, QTimer


class InteractiveDrawScheduler(QObject):
    """Coalesce high-frequency interaction updates into bounded redraws."""

    def __init__(
        self,
        draw: Callable[[], None],
        parent: QObject | None = None,
        *,
        interval_ms: int = 16,
    ):
        super().__init__(parent)
        if interval_ms < 1:
            raise ValueError("interval_ms must be positive")
        self._draw = draw
        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.setInterval(interval_ms)
        self._timer.timeout.connect(self._run)

    @property
    def pending(self) -> bool:
        return self._timer.isActive()

    def request(self) -> None:
        if not self._timer.isActive():
            self._timer.start()

    def flush(self) -> None:
        if self._timer.isActive():
            self._timer.stop()
        self._run()

    def cancel(self) -> None:
        self._timer.stop()

    def _run(self) -> None:
        self._draw()
