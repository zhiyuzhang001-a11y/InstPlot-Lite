#!/usr/bin/env python3
"""Verify release documentation, lock reproducibility, and environment footprint."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 release-test compatibility
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
LOCK_COMMAND = (
    "uv pip compile pyproject.toml --python-version 3.10 --universal "
    "--generate-hashes --output-file requirements.lock"
)


def validate_release_docs(root=ROOT):
    root = Path(root)
    readme = (root / "README.md").read_text(encoding="utf-8")
    metadata = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))
    version = metadata["project"]["version"]
    errors = []
    required = {
        f"version-{version}-blue": "README version badge does not match pyproject.toml",
        "Windows%20%7C%20macOS%20%7C%20Linux": "platform badge must include all three systems",
        "CPython 3.10–3.14": "supported CPython range is missing",
        "无需预装 Python": "zero-Python bootstrap guidance is missing",
        "https://astral.sh/uv/0.12.5/": "pinned uv bootstrap source is missing",
        "至少 1 GB": "minimum free disk guidance is missing",
        "libEGL.so.1": "Linux EGL prerequisite is missing",
        "libegl1": "Ubuntu/Debian EGL package guidance is missing",
        "--repair": "repair instructions are missing",
        ".install-logs": "installer log instructions are missing",
        "PENDING_USER_VALIDATION": "real-instrument sample limitation is missing",
        "https://github.com/zhiyuzhang001-a11y/InstPlot/issues": "real issue tracker link is missing",
        LOCK_COMMAND: "canonical lock regeneration command is missing",
    }
    for marker, reason in required.items():
        if marker not in readme:
            errors.append(reason)
    for placeholder in ("your-email", "yourusername", "example.com"):
        if placeholder in readme.lower():
            errors.append(f"README contains placeholder: {placeholder}")
    expected_issues = "https://github.com/zhiyuzhang001-a11y/InstPlot/issues"
    if metadata["project"].get("urls", {}).get("Issues") != expected_issues:
        errors.append("pyproject.toml issue tracker metadata is missing or incorrect")
    return errors


def environment_size(environment):
    environment = Path(environment)
    if not environment.is_dir():
        raise ValueError(f"environment is not a directory: {environment}")
    total = 0
    pending = [environment]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                if entry.is_symlink():
                    continue
                if entry.is_dir(follow_symlinks=False):
                    pending.append(Path(entry.path))
                elif entry.is_file(follow_symlinks=False):
                    total += entry.stat(follow_symlinks=False).st_size
    return total


def check_lock(root=ROOT, uv_command="uv"):
    root = Path(root)
    executable = shutil.which(uv_command) if not Path(uv_command).is_file() else uv_command
    if not executable:
        raise RuntimeError(f"uv executable not found: {uv_command}")
    executable = str(Path(executable).resolve())
    with tempfile.TemporaryDirectory(prefix="instplot-lock-check-") as temporary:
        temporary_root = Path(temporary)
        shutil.copy2(root / "pyproject.toml", temporary_root / "pyproject.toml")
        command = [
            executable,
            "pip",
            "compile",
            "pyproject.toml",
            "--python-version",
            "3.10",
            "--universal",
            "--generate-hashes",
            "--output-file",
            "requirements.lock",
        ]
        result = subprocess.run(
            command,
            cwd=temporary_root,
            text=True,
            capture_output=True,
        )
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip() or "uv lock compilation failed")
        expected = (root / "requirements.lock").read_bytes()
        regenerated = (temporary_root / "requirements.lock").read_bytes()
        if regenerated != expected:
            raise RuntimeError("requirements.lock is not reproducible from pyproject.toml")


def _environment_python(environment):
    environment = Path(environment)
    if os.name == "nt":
        return environment / "Scripts" / "python.exe"
    return environment / "bin" / "python"


def measure_full_pyside(environment):
    environment = Path(environment).resolve()
    source_python = _environment_python(environment)
    if not source_python.is_file():
        raise RuntimeError(f"environment Python not found: {source_python}")
    with tempfile.TemporaryDirectory(prefix="instplot-footprint-") as temporary:
        measurement_environment = Path(temporary) / environment.name
        shutil.copytree(environment, measurement_environment, symlinks=True)
        python = _environment_python(measurement_environment)
        before = environment_size(measurement_environment)
        version_result = subprocess.run(
            [
                str(python),
                "-I",
                "-c",
                "import importlib.metadata as m; print(m.version('PySide6-Essentials'))",
            ],
            text=True,
            capture_output=True,
        )
        if version_result.returncode != 0:
            raise RuntimeError(
                version_result.stderr.strip() or "PySide6-Essentials is not installed"
            )
        version = version_result.stdout.strip()
        install = subprocess.run(
            [
                str(python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                f"PySide6=={version}",
            ],
            text=True,
            capture_output=True,
        )
        if install.returncode != 0:
            raise RuntimeError(
                install.stderr.strip()
                or install.stdout.strip()
                or "full PySide6 install failed"
            )
        after = environment_size(measurement_environment)
        if after <= before:
            raise RuntimeError("full PySide6 did not increase the environment footprint")
        result = {
            "platform": sys.platform,
            "pyside_version": version,
            "essentials_bytes": before,
            "full_bytes": after,
            "saved_bytes": after - before,
            "saved_percent": round((after - before) * 100 / after, 2),
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-docs", action="store_true")
    parser.add_argument("--check-lock", action="store_true")
    parser.add_argument("--uv", default="uv")
    parser.add_argument("--measure-full-pyside", metavar="ENVIRONMENT")
    args = parser.parse_args()
    selected = args.check_docs or args.check_lock or args.measure_full_pyside
    if not selected:
        args.check_docs = True

    result = {"status": "healthy"}
    try:
        if args.check_docs:
            errors = validate_release_docs(ROOT)
            if errors:
                raise RuntimeError("; ".join(errors))
            result["documentation"] = "healthy"
        if args.check_lock:
            check_lock(ROOT, args.uv)
            result["lock"] = "reproducible"
        if args.measure_full_pyside:
            result["footprint"] = measure_full_pyside(args.measure_full_pyside)
    except (OSError, RuntimeError, ValueError) as error:
        print(json.dumps({"status": "error", "reason": str(error)}, ensure_ascii=False, indent=2))
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
