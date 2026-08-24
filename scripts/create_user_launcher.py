#!/usr/bin/env python3
"""Create a non-App, user-visible launcher for an installed InstPlot tree."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex


ROOT = Path(__file__).resolve().parents[1]


def _desktop_entry_quote(value):
    value = str(value)
    escaped = (
        value.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("`", "\\`")
        .replace("$", "\\$")
    )
    return f'"{escaped}"'


def user_launcher_content(project_root, platform_name):
    root = Path(project_root).resolve()
    python = root / ".venv" / "bin" / "python"
    if platform_name == "macos":
        log = root / ".install-logs" / "launcher.log"
        return (
            "#!/bin/sh\n"
            f"mkdir -p {shlex.quote(str(log.parent))}\n"
            f"nohup {shlex.quote(str(python))} -m InstPlot "
            f"</dev/null >>{shlex.quote(str(log))} 2>&1 &\n"
        )
    if platform_name == "linux":
        return (
            "[Desktop Entry]\n"
            "Type=Application\n"
            "Name=InstPlot\n"
            "Comment=Experimental data visualization and preprocessing\n"
            f"Exec={_desktop_entry_quote(python)} -m InstPlot\n"
            f"Path={root}\n"
            f"Icon={root / 'logo.ico'}\n"
            "Terminal=false\n"
            "Categories=Education;Science;\n"
        )
    raise ValueError(f"unsupported user launcher platform: {platform_name}")


def _launcher_path(platform_name, home):
    home = Path(home)
    if platform_name == "macos":
        return home / "Desktop" / "InstPlot.command"
    if platform_name == "linux":
        return home / ".local" / "share" / "applications" / "InstPlot.desktop"
    raise ValueError(f"unsupported user launcher platform: {platform_name}")


def create_user_launcher(project_root=ROOT, platform_name="macos", *, home=None):
    root = Path(project_root).resolve()
    python = root / ".venv" / "bin" / "python"
    if not python.is_file():
        raise RuntimeError(f"installed environment Python not found: {python}")
    home = Path.home() if home is None else Path(home)
    launcher = _launcher_path(platform_name, home)
    expected = user_launcher_content(root, platform_name)

    if launcher.is_symlink():
        return {"state": "conflict", "path": str(launcher)}
    if launcher.exists():
        if not launcher.is_file():
            return {"state": "conflict", "path": str(launcher)}
        try:
            current = launcher.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            return {"state": "conflict", "path": str(launcher)}
        state = "identical" if current == expected else "conflict"
        return {"state": state, "path": str(launcher)}

    launcher.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(launcher, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o755)
    with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
        stream.write(expected)
    launcher.chmod(0o755)
    return {"state": "created", "path": str(launcher)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("macos", "linux"), required=True)
    parser.add_argument("--project-root", type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        result = create_user_launcher(args.project_root, args.platform)
    except (OSError, RuntimeError, ValueError) as error:
        print(json.dumps({"status": "error", "reason": str(error)}, ensure_ascii=False))
        return 1
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
