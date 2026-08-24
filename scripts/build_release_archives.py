#!/usr/bin/env python3
"""Build compact, platform-specific InstPlot source installers."""

from __future__ import annotations

import argparse
import gzip
from io import BytesIO
from pathlib import Path
import stat
import tarfile
import zipfile

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 compatibility
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]
COMMON_FILES = (
    "InstPlot.py",
    "instplot_diagnostics.py",
    "instplot_dialogs.py",
    "instplot_fitting.py",
    "instplot_history.py",
    "instplot_io.py",
    "instplot_processing.py",
    "instplot_rendering.py",
    "instplot_tasks.py",
    "LICENSE",
    "README.md",
    "logo.ico",
    "pyproject.toml",
    "requirements.lock",
    "requirements.txt",
    "scripts/install.py",
    "scripts/verify_install.py",
)
PLATFORM_FILES = {
    "windows": (
        "install_windows.bat",
        "scripts/bootstrap_uv.ps1",
        "scripts/create_windows_shortcut.ps1",
    ),
    "macos": (
        "install_macos.command",
        "scripts/bootstrap_uv.sh",
        "scripts/create_user_launcher.py",
    ),
    "linux": (
        "install_linux.sh",
        "scripts/bootstrap_uv.sh",
        "scripts/create_user_launcher.py",
    ),
}
EXECUTABLE_FILES = {
    Path("install_linux.sh"),
    Path("install_macos.command"),
    Path("scripts/bootstrap_uv.sh"),
}


def _archive_mode(relative):
    return 0o755 if relative in EXECUTABLE_FILES else 0o644


def _manifest(root, platform_name):
    relative_paths = [Path(name) for name in COMMON_FILES + PLATFORM_FILES[platform_name]]
    relative_paths.extend(
        path.relative_to(root)
        for path in sorted((root / "symbol_icons").rglob("*"))
        if path.is_file() and not path.is_symlink()
    )
    for relative in relative_paths:
        source = root / relative
        if source.is_symlink() or not source.is_file():
            raise RuntimeError(f"missing or unsafe release input: {relative}")
    return sorted(set(relative_paths), key=lambda path: path.as_posix())


def _zip_archive(root, destination, prefix, files):
    with zipfile.ZipFile(
        destination, mode="x", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for relative in files:
            source = root / relative
            info = zipfile.ZipInfo(f"{prefix}/{relative.as_posix()}")
            info.date_time = (1980, 1, 1, 0, 0, 0)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            mode = stat.S_IFREG | _archive_mode(relative)
            info.external_attr = mode << 16
            archive.writestr(info, source.read_bytes(), compresslevel=9)


def _tar_archive(root, destination, prefix, files):
    with destination.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for relative in files:
                    source = root / relative
                    data = source.read_bytes()
                    info = tarfile.TarInfo(f"{prefix}/{relative.as_posix()}")
                    info.size = len(data)
                    info.mode = _archive_mode(relative)
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    archive.addfile(info, BytesIO(data))


def build_release_archives(project_root=ROOT, output_directory=None):
    root = Path(project_root).resolve()
    output = root / "dist" if output_directory is None else Path(output_directory)
    output.mkdir(parents=True, exist_ok=True)
    metadata = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))
    version = metadata["project"]["version"]
    prefix = f"InstPlot-{version}"
    destinations = []
    for platform_name in ("windows", "macos", "linux"):
        suffix = ".tar.gz" if platform_name == "linux" else ".zip"
        destination = output / f"{prefix}-{platform_name}{suffix}"
        files = _manifest(root, platform_name)
        if platform_name == "linux":
            _tar_archive(root, destination, prefix, files)
        else:
            _zip_archive(root, destination, prefix, files)
        destinations.append(destination)
    return destinations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        archives = build_release_archives(args.project_root, args.output)
    except (OSError, RuntimeError, KeyError, ValueError) as error:
        parser.exit(1, f"archive build failed: {error}\n")
    for archive in archives:
        print(archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
