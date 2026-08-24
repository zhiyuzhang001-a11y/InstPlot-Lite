import os
from pathlib import Path

import pytest

from scripts.verify_install import run_smoke


ROOT = Path(__file__).resolve().parents[1]


def test_installed_runtime_smoke_covers_io_plot_and_resources():
    result = run_smoke()
    assert result["status"] == "healthy"
    assert result["svg_count"] == 67
    assert result["txt_rows"] == 2
    assert result["exports"] == ["csv", "png", "xlsx"]


def test_platform_install_entrypoints_are_local_and_non_privileged():
    for name in ["install_windows.bat", "install_macos.command", "install_linux.sh"]:
        path = ROOT / name
        content = path.read_text(encoding="utf-8")
        assert "scripts/install.py" in content or "scripts\\install.py" in content
        assert "--apply" in content
        assert "run_instplot" in content
        assert "\nsudo " not in content.lower()
        assert "curl" not in content.lower()
        if name == "install_windows.bat":
            assert "powershell -NoProfile -ExecutionPolicy Bypass -File" in content
            assert "bootstrap_uv.ps1" in content
            assert "-EncodedCommand" not in content
        else:
            assert "powershell" not in content.lower()


def test_linux_entrypoint_detects_egl_and_prints_distribution_specific_help():
    content = (ROOT / "install_linux.sh").read_text(encoding="utf-8")

    assert "libEGL.so.1" in content
    assert "ldconfig -p" in content
    assert "/etc/os-release" in content
    assert "sudo apt-get install -y libegl1" in content
    assert "sudo dnf install mesa-libEGL" in content
    assert "sudo pacman -S libglvnd" in content
    assert "sudo zypper install Mesa-libEGL1" in content
    assert content.index("libEGL.so.1") < content.index("bootstrap_uv.sh")
    assert "eval " not in content


def test_install_entrypoints_create_non_app_user_launchers():
    windows = (ROOT / "install_windows.bat").read_text(encoding="utf-8")
    macos = (ROOT / "install_macos.command").read_text(encoding="utf-8")
    linux = (ROOT / "install_linux.sh").read_text(encoding="utf-8")

    assert "create_windows_shortcut.ps1" in windows
    assert "create_user_launcher.py" in macos
    assert "--platform macos" in macos
    assert "create_user_launcher.py" in linux
    assert "--platform linux" in linux

    shortcut = (ROOT / "scripts" / "create_windows_shortcut.ps1").read_text(
        encoding="utf-8"
    )
    assert "GetFolderPath(\"Desktop\")" in shortcut
    assert "pythonw.exe" in shortcut
    assert "CreateShortcut" in shortcut
    assert "InstPlot.lnk" in shortcut


@pytest.mark.skipif(os.name == "nt", reason="Windows filesystems do not expose POSIX execute bits")
def test_unix_install_entrypoints_are_executable():
    for name in ["install_macos.command", "install_linux.sh"]:
        assert (ROOT / name).stat().st_mode & 0o111
