from pathlib import Path
import stat
import tarfile
import zipfile

from scripts.build_release_archives import build_release_archives


ROOT = Path(__file__).resolve().parents[1]


def test_release_archives_are_small_platform_specific_and_installable(tmp_path):
    archives = build_release_archives(ROOT, tmp_path)

    assert {path.name for path in archives} == {
        "InstPlot-1.0.0-windows.zip",
        "InstPlot-1.0.0-macos.zip",
        "InstPlot-1.0.0-linux.tar.gz",
    }
    assert all(path.stat().st_size < 2_000_000 for path in archives)

    windows = tmp_path / "InstPlot-1.0.0-windows.zip"
    with zipfile.ZipFile(windows) as archive:
        names = set(archive.namelist())
        assert "InstPlot-1.0.0/install_windows.bat" in names
        assert "InstPlot-1.0.0/scripts/create_windows_shortcut.ps1" in names
        assert not any("tests/" in name or ".git" in name for name in names)

    macos = tmp_path / "InstPlot-1.0.0-macos.zip"
    with zipfile.ZipFile(macos) as archive:
        member = archive.getinfo("InstPlot-1.0.0/install_macos.command")
        mode = member.external_attr >> 16
        assert mode & stat.S_IXUSR
        assert "InstPlot-1.0.0/scripts/create_user_launcher.py" in archive.namelist()

    linux = tmp_path / "InstPlot-1.0.0-linux.tar.gz"
    with tarfile.open(linux) as archive:
        member = archive.getmember("InstPlot-1.0.0/install_linux.sh")
        assert member.mode & stat.S_IXUSR
        assert "InstPlot-1.0.0/requirements.lock" in archive.getnames()
