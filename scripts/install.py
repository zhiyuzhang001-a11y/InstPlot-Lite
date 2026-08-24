#!/usr/bin/env python3
"""Dry-run-first, project-local installer for InstPlot."""

from __future__ import annotations

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_PROJECT_FILES = (
    Path("pyproject.toml"),
    Path("requirements.lock"),
    Path("InstPlot.py"),
    Path("scripts/verify_install.py"),
)
VERSION_PROBE = "import sys; print('.'.join(map(str, sys.version_info[:2])))"
MINIMUM_PYTHON = (3, 10)
MAXIMUM_PYTHON = (3, 15)
PYTHON_REQUEST = ">=3.10,<3.15"
HEALTH_PROBE = (
    "import importlib.metadata as m; "
    "from PySide6 import QtCore, QtGui, QtWidgets, QtSvg; "
    "import InstPlot; "
    "assert m.version('instplot')"
)


class InstallError(RuntimeError):
    def __init__(self, code, reason, log_path=None):
        self.code = code
        self.reason = reason
        self.log_path = log_path
        super().__init__(f"{code}: {reason}")


def _platform_family(platform_name):
    if platform_name.startswith("win"):
        return "windows"
    if platform_name == "darwin":
        return "macos"
    if platform_name.startswith("linux"):
        return "linux"
    raise InstallError("unsupported_platform", f"unsupported platform: {platform_name}")


def _launcher_name(platform_name):
    family = _platform_family(platform_name)
    return {
        "windows": "run_instplot.bat",
        "macos": "run_instplot.command",
        "linux": "run_instplot.sh",
    }[family]


def launcher_content(platform_name):
    family = _platform_family(platform_name)
    if family == "windows":
        return (
            "@echo off\n"
            "setlocal\n"
            "set \"SCRIPT_DIR=%~dp0\"\n"
            "\"%SCRIPT_DIR%.venv\\Scripts\\python.exe\" -m InstPlot %*\n"
            "if errorlevel 1 pause\n"
        )
    return (
        "#!/bin/sh\n"
        "SCRIPT_DIR=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n"
        "exec \"$SCRIPT_DIR/.venv/bin/python\" -m InstPlot \"$@\"\n"
    )


def _validate_project_root(project_root):
    root = Path(project_root).expanduser()
    if not root.exists():
        raise InstallError("invalid_project", f"project directory does not exist: {root}")
    if not root.is_dir():
        raise InstallError("invalid_project", f"project path is not a directory: {root}")
    root = root.resolve(strict=True)
    for relative in REQUIRED_PROJECT_FILES:
        candidate = root / relative
        if candidate.is_symlink() or not candidate.is_file():
            raise InstallError("invalid_project", f"missing or unsafe project file: {relative}")
    return root


def _venv_python(root, platform_name):
    if _platform_family(platform_name) == "windows":
        return root / ".venv" / "Scripts" / "python.exe"
    return root / ".venv" / "bin" / "python"


def _call(runner, command, root):
    try:
        return runner(
            [str(part) for part in command],
            cwd=str(root),
            text=True,
            capture_output=True,
        )
    except OSError as error:
        return type("FailedCommand", (), {"returncode": 127, "stdout": "", "stderr": str(error)})()


def _probe_python(command, root, runner):
    result = _call(runner, list(command) + ["-c", VERSION_PROBE], root)
    if result.returncode != 0:
        return False
    try:
        version = tuple(int(part) for part in result.stdout.strip().split("."))
    except ValueError:
        return False
    return MINIMUM_PYTHON <= version < MAXIMUM_PYTHON


def _select_python(root, platform_name, runner, explicit=None):
    if explicit:
        command = [str(part) for part in explicit]
        if not _probe_python(command, root, runner):
            raise InstallError(
                "unsupported_python",
                f"Python command does not satisfy {PYTHON_REQUEST}: {command}",
            )
        return command

    candidates = []
    if MINIMUM_PYTHON <= sys.version_info[:2] < MAXIMUM_PYTHON:
        candidates.append([sys.executable])
    if _platform_family(platform_name) == "windows" and shutil.which("py"):
        candidates.append(["py", "-3"])
    for name in (
        "python3.14",
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "python3",
        "python",
    ):
        executable = shutil.which(name)
        if executable:
            candidates.append([executable])
    for command in candidates:
        if _probe_python(command, root, runner):
            return command

    uv = shutil.which("uv")
    if uv:
        located = _call(runner, [uv, "python", "find", PYTHON_REQUEST], root)
        if located.returncode == 0 and located.stdout.strip():
            command = [located.stdout.strip().splitlines()[-1]]
            if _probe_python(command, root, runner):
                return command
    raise InstallError(
        "python_not_found",
        f"No CPython satisfying {PYTHON_REQUEST} was found. Run a system installer entrypoint so uv can provide it.",
    )


def _environment_state(root, platform_name, runner):
    environment = root / ".venv"
    if environment.is_symlink():
        return "conflict"
    if not environment.exists():
        return "missing"
    if not environment.is_dir():
        return "conflict"
    python = _venv_python(root, platform_name)
    if not python.is_file():
        return "repair-needed"
    version = _call(runner, [python, "-I", "-c", VERSION_PROBE], root)
    if version.returncode != 0:
        return "repair-needed"
    try:
        environment_version = tuple(
            int(part) for part in version.stdout.strip().split(".")
        )
    except ValueError:
        return "repair-needed"
    if not MINIMUM_PYTHON <= environment_version < MAXIMUM_PYTHON:
        return "repair-needed"
    dependency_health = _call(runner, [python, "-m", "pip", "check"], root)
    if dependency_health.returncode != 0:
        return "repair-needed"
    health = _call(runner, [python, "-I", "-c", HEALTH_PROBE], root)
    return "healthy" if health.returncode == 0 else "repair-needed"


def _launcher_state(root, platform_name):
    launcher = root / _launcher_name(platform_name)
    if launcher.is_symlink():
        return "conflict"
    if not launcher.exists():
        return "missing"
    if not launcher.is_file():
        return "conflict"
    try:
        current = launcher.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return "conflict"
    return "identical" if current == launcher_content(platform_name) else "conflict"


def _open_log(root):
    directory = root / ".install-logs"
    if directory.is_symlink() or (directory.exists() and not directory.is_dir()):
        raise InstallError("conflict", f"unsafe install log directory: {directory}")
    directory.mkdir(mode=0o755, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    descriptor, name = tempfile.mkstemp(
        prefix=f"install-{timestamp}-",
        suffix=".log",
        dir=directory,
        text=True,
    )
    path = Path(name)
    return path, os.fdopen(descriptor, "w", encoding="utf-8")


def _run_checked(runner, command, root, log, log_path):
    command = [str(part) for part in command]
    log.write(f"$ {shlex.join(command)}\n")
    log.flush()
    result = _call(runner, command, root)
    if result.stdout:
        log.write(result.stdout)
        if not result.stdout.endswith("\n"):
            log.write("\n")
    if result.stderr:
        log.write(result.stderr)
        if not result.stderr.endswith("\n"):
            log.write("\n")
    log.flush()
    if result.returncode != 0:
        raise InstallError(
            "command_failed",
            f"command exited with {result.returncode}: {shlex.join(command)}",
            log_path,
        )


def _write_launcher(root, platform_name):
    launcher = root / _launcher_name(platform_name)
    data = launcher_content(platform_name).encode("utf-8")
    descriptor = os.open(launcher, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o755)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(data)
    if _platform_family(platform_name) != "windows":
        launcher.chmod(0o755)
    return launcher


def install(
    project_root=ROOT,
    *,
    apply=False,
    repair=False,
    python_command=None,
    platform_name=None,
    runner=subprocess.run,
):
    platform_name = platform_name or sys.platform
    root = _validate_project_root(project_root)
    family = _platform_family(platform_name)
    state = _environment_state(root, platform_name, runner)
    launcher_state = _launcher_state(root, platform_name)

    if state == "conflict" or launcher_state == "conflict":
        if not apply:
            raise InstallError("conflict", "unsafe .venv or modified launcher detected")
        log_path, log = _open_log(root)
        with log:
            log.write("conflict: unsafe .venv or modified launcher detected\n")
        raise InstallError("conflict", "unsafe .venv or modified launcher detected", log_path)

    selected_python = None
    if state != "healthy":
        selected_python = _select_python(
            root, platform_name, runner, explicit=python_command
        )

    result = {
        "mode": "apply" if apply else "dry-run",
        "state": state,
        "platform": family,
        "project_root": str(root),
        "python": selected_python,
        "launcher": {"path": _launcher_name(platform_name), "state": launcher_state},
        "actions": [],
    }
    if not apply:
        if state == "missing":
            result["actions"] = ["create-venv", "install-locked", "install-project", "verify"]
        elif state == "repair-needed":
            result["actions"] = ["repair-required"]
        if launcher_state == "missing":
            result["actions"].append("create-launcher")
        return result

    log_path, log = _open_log(root)
    result["log_path"] = str(log_path)
    try:
        with log:
            log.write(
                f"mode=apply platform={family} initial_state={state} project={root}\n"
            )
            if state == "repair-needed" and not repair:
                raise InstallError(
                    "repair_required",
                    "existing environment is unhealthy; rerun with --apply --repair",
                    log_path,
                )
            if state in {"missing", "repair-needed"}:
                venv_command = list(selected_python) + ["-m", "venv"]
                if state == "repair-needed":
                    venv_command.append("--upgrade")
                venv_command.append(str(root / ".venv"))
                _run_checked(runner, venv_command, root, log, log_path)
                result["actions"].append("create-venv" if state == "missing" else "repair-venv")

                python = _venv_python(root, platform_name)
                commands = [
                    [
                        python,
                        "-m",
                        "pip",
                        "install",
                        "--disable-pip-version-check",
                        "--require-hashes",
                        "-r",
                        root / "requirements.lock",
                    ],
                    [
                        python,
                        "-m",
                        "pip",
                        "install",
                        "--disable-pip-version-check",
                        "--no-deps",
                        "--force-reinstall",
                        root,
                    ],
                    [python, "-m", "pip", "check"],
                    [python, "-I", root / "scripts" / "verify_install.py"],
                ]
                for command in commands:
                    _run_checked(runner, command, root, log, log_path)
                result["actions"].extend(["install-locked", "install-project", "verify"])

            if launcher_state == "missing":
                launcher = _write_launcher(root, platform_name)
                result["actions"].append("create-launcher")
                log.write(f"created launcher: {launcher}\n")
    except InstallError:
        raise
    except OSError as error:
        raise InstallError("write_failed", str(error), log_path) from error

    result["state"] = "healthy"
    result["launcher"]["state"] = "identical"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-root", type=Path, default=ROOT)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--repair", action="store_true")
    parser.add_argument("--python", dest="python_path")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    if args.repair and not args.apply:
        parser.error("--repair requires --apply")
    try:
        result = install(
            args.project_root,
            apply=args.apply,
            repair=args.repair,
            python_command=[args.python_path] if args.python_path else None,
        )
    except InstallError as error:
        payload = {"status": "error", "code": error.code, "reason": error.reason}
        if error.log_path:
            payload["log_path"] = str(error.log_path)
        if args.json:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        else:
            print(f"安装失败 [{error.code}]：{error.reason}", file=sys.stderr)
            if error.log_path:
                print(f"安装日志：{error.log_path}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(f"InstPlot installer: {result['mode']} / {result['state']}")
        print(f"项目目录：{result['project_root']}")
        for action in result["actions"]:
            print(f"- {action}")
        if result.get("log_path"):
            print(f"安装日志：{result['log_path']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
