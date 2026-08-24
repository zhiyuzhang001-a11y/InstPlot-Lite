import logging
from pathlib import Path

from instplot_diagnostics import (
    configure_logging,
    default_log_directory,
    format_exception_details,
    make_exception_hook,
)


def test_default_log_directory_is_platform_specific():
    assert default_log_directory(
        platform_name="darwin", environ={}, home=Path("/Users/tester")
    ) == Path("/Users/tester/Library/Logs/InstPlot")
    assert default_log_directory(
        platform_name="win32",
        environ={"LOCALAPPDATA": r"C:\Users\tester\AppData\Local"},
        home=Path("C:/Users/tester"),
    ) == Path(r"C:\Users\tester\AppData\Local") / "InstPlot" / "Logs"
    assert default_log_directory(
        platform_name="linux",
        environ={"XDG_STATE_HOME": "/state"},
        home=Path("/home/tester"),
    ) == Path("/state/instplot")


def test_configure_logging_writes_rotating_utf8_log(tmp_path):
    log_path = configure_logging(tmp_path)
    logging.getLogger("instplot").info("结构化诊断")
    for handler in logging.getLogger("instplot").handlers:
        handler.flush()

    assert log_path == tmp_path / "instplot.log"
    text = log_path.read_text(encoding="utf-8")
    assert "INFO" in text
    assert "结构化诊断" in text


def test_configure_logging_falls_back_when_requested_directory_is_unusable(tmp_path):
    blocked = tmp_path / "not-a-directory"
    blocked.write_text("occupied", encoding="utf-8")

    log_path = configure_logging(blocked)

    assert log_path.exists()
    assert log_path.parent != blocked


def test_exception_details_and_hook_log_without_consuming_exception(tmp_path):
    log_path = configure_logging(tmp_path)
    presented = []
    try:
        raise ValueError("bad input")
    except ValueError as error:
        details = format_exception_details(error, context="import")
        hook = make_exception_hook(lambda title, message, value: presented.append((title, message, value)))
        hook(type(error), error, error.__traceback__)

    for handler in logging.getLogger("instplot").handlers:
        handler.flush()
    assert "Context: import" in details
    assert "ValueError: bad input" in details
    assert presented and "bad input" in presented[0][2]
    assert "Unhandled exception" in log_path.read_text(encoding="utf-8")
