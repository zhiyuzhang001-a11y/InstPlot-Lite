import os
from pathlib import Path

import pytest

from scripts.create_user_launcher import create_user_launcher, user_launcher_content


@pytest.mark.parametrize(
    ("platform_name", "relative_path"),
    [
        ("macos", Path("Desktop/InstPlot.command")),
        ("linux", Path(".local/share/applications/InstPlot.desktop")),
    ],
)
def test_user_launcher_is_created_without_overwriting(platform_name, relative_path, tmp_path):
    root = tmp_path / "项目 folder"
    home = tmp_path / "user home"
    python = root / ".venv" / "bin" / "python"
    python.parent.mkdir(parents=True)
    python.touch()

    created = create_user_launcher(root, platform_name, home=home)
    launcher = home / relative_path

    assert created["state"] == "created"
    assert Path(created["path"]) == launcher
    assert launcher.read_text(encoding="utf-8") == user_launcher_content(root, platform_name)
    if os.name != "nt":
        assert launcher.stat().st_mode & 0o111

    assert create_user_launcher(root, platform_name, home=home)["state"] == "identical"

    launcher.write_text("user content\n", encoding="utf-8")
    assert create_user_launcher(root, platform_name, home=home)["state"] == "conflict"
    assert launcher.read_text(encoding="utf-8") == "user content\n"


def test_macos_launcher_detaches_without_building_an_app(tmp_path):
    content = user_launcher_content(tmp_path / "project", "macos")

    assert "nohup" in content
    assert "-m InstPlot" in content
    assert "</dev/null" in content
    assert ".app" not in content


def test_linux_launcher_appears_in_application_menu(tmp_path):
    content = user_launcher_content(tmp_path / "project", "linux")

    assert "[Desktop Entry]" in content
    assert "Type=Application" in content
    assert "Terminal=false" in content
    assert "Categories=Education;Science;" in content


def test_unknown_launcher_platform_is_rejected(tmp_path):
    with pytest.raises(ValueError):
        user_launcher_content(tmp_path, "windows")
