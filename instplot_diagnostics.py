"""Cross-platform diagnostics and top-level exception reporting."""

from __future__ import annotations

import logging
from logging.handlers import RotatingFileHandler
import os
from pathlib import Path
import sys
import tempfile
import traceback
from collections.abc import Callable, Mapping


LOGGER_NAME = "instplot"
_LOG_PATH: Path | None = None


def default_log_directory(
    *,
    platform_name: str | None = None,
    environ: Mapping[str, str] | None = None,
    home: Path | None = None,
) -> Path:
    platform_name = platform_name or sys.platform
    environ = os.environ if environ is None else environ
    override = environ.get("INSTPLOT_LOG_DIR")
    if override:
        return Path(override).expanduser()
    home = Path.home() if home is None else home
    if platform_name == "darwin":
        return home / "Library" / "Logs" / "InstPlot"
    if platform_name.startswith("win"):
        root = Path(environ.get("LOCALAPPDATA", str(home / "AppData" / "Local")))
        return root / "InstPlot" / "Logs"
    state_root = environ.get("XDG_STATE_HOME")
    return Path(state_root) / "instplot" if state_root else home / ".local" / "state" / "instplot"


def configure_logging(log_directory: Path | str | None = None) -> Path:
    global _LOG_PATH
    directory = Path(log_directory) if log_directory is not None else default_log_directory()
    logger = logging.getLogger(LOGGER_NAME)
    logger.setLevel(logging.INFO)
    logger.propagate = False
    for handler in list(logger.handlers):
        if getattr(handler, "_instplot_handler", False):
            logger.removeHandler(handler)
            handler.close()
    try:
        directory.mkdir(parents=True, exist_ok=True)
        log_path = directory / "instplot.log"
        handler = RotatingFileHandler(
            log_path,
            maxBytes=1_000_000,
            backupCount=3,
            encoding="utf-8",
        )
    except OSError:
        directory = Path(tempfile.gettempdir()) / "InstPlot"
        directory.mkdir(parents=True, exist_ok=True)
        log_path = directory / "instplot.log"
        handler = RotatingFileHandler(
            log_path,
            maxBytes=1_000_000,
            backupCount=3,
            encoding="utf-8",
        )
    handler._instplot_handler = True
    handler.setFormatter(
        logging.Formatter("%(asctime)s %(levelname)s %(name)s %(message)s")
    )
    logger.addHandler(handler)
    _LOG_PATH = log_path
    logger.info("diagnostics_started python=%s platform=%s", sys.version.split()[0], sys.platform)
    return log_path


def current_log_path() -> Path | None:
    return _LOG_PATH


def format_exception_details(error: BaseException, *, context: str | None = None) -> str:
    lines = []
    if context:
        lines.append(f"Context: {context}\n")
    lines.extend(traceback.format_exception(type(error), error, error.__traceback__))
    return "".join(lines).rstrip()


def make_exception_hook(
    presenter: Callable[[str, str, str], None] | None = None,
    *,
    fallback: Callable | None = None,
):
    fallback = fallback or sys.__excepthook__

    def hook(exception_type, error, exception_traceback):
        if issubclass(exception_type, KeyboardInterrupt):
            fallback(exception_type, error, exception_traceback)
            return
        details = "".join(
            traceback.format_exception(exception_type, error, exception_traceback)
        ).rstrip()
        logging.getLogger(LOGGER_NAME).critical("Unhandled exception\n%s", details)
        if presenter is not None:
            location = str(_LOG_PATH) if _LOG_PATH is not None else "日志尚未初始化"
            presenter(
                "InstPlot 运行错误",
                f"发生未处理错误。诊断日志：{location}",
                details,
            )

    return hook
