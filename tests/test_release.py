import os
from pathlib import Path
from types import SimpleNamespace
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

import scripts.verify_release as release
import InstPlot
from scripts.verify_release import environment_size, validate_release_docs


ROOT = Path(__file__).resolve().parents[1]


def test_release_documentation_has_no_placeholders_and_matches_metadata():
    assert validate_release_docs(ROOT) == []


def test_project_supports_current_cpython_range():
    metadata = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))

    assert metadata["project"]["requires-python"] == ">=3.10,<3.15"


def test_environment_size_counts_regular_files_without_following_symlinks(tmp_path):
    environment = tmp_path / "environment"
    environment.mkdir()
    (environment / "first.bin").write_bytes(b"1234")
    nested = environment / "nested"
    nested.mkdir()
    (nested / "second.bin").write_bytes(b"123456")
    outside = tmp_path / "outside.bin"
    outside.write_bytes(b"12345678")
    if os.name != "nt":
        (environment / "outside-link").symlink_to(outside)

    assert environment_size(environment) == 10


def test_release_workflow_checks_lock_and_cross_platform_footprint():
    workflow = (ROOT / ".github" / "workflows" / "native-install.yml").read_text(
        encoding="utf-8"
    )
    assert "verify_release.py --check-docs --check-lock" in workflow
    assert "uv==0.12.5" in workflow
    assert "--measure-full-pyside" in workflow
    assert workflow.index("--measure-full-pyside") < workflow.index(
        "-m pip install pytest==8.4.2"
    )


def test_lock_check_resolves_relative_uv_before_changing_directory(tmp_path, monkeypatch):
    root = tmp_path / "project"
    root.mkdir()
    (root / "pyproject.toml").write_text("[project]\nname = 'sample'\n", encoding="utf-8")
    (root / "requirements.lock").write_text("locked\n", encoding="utf-8")
    uv = root / ".installer" / "uv" / "uv"
    uv.parent.mkdir(parents=True)
    uv.touch()
    commands = []

    def fake_run(command, **kwargs):
        commands.append(command)
        (Path(kwargs["cwd"]) / "requirements.lock").write_text("locked\n", encoding="utf-8")
        return SimpleNamespace(returncode=0, stdout="", stderr="")

    monkeypatch.chdir(root)
    monkeypatch.setattr(release.subprocess, "run", fake_run)

    release.check_lock(root, ".installer/uv/uv")

    assert Path(commands[0][0]).is_absolute()


def test_full_pyside_measurement_uses_disposable_environment(tmp_path, monkeypatch):
    environment = tmp_path / ".venv"
    python = release._environment_python(environment)
    python.parent.mkdir(parents=True)
    python.touch()
    sizes = iter((100, 250))
    commands = []

    def fake_run(command, **kwargs):
        commands.append(command)
        if "import importlib.metadata" in " ".join(command):
            return SimpleNamespace(returncode=0, stdout="6.9.1\n", stderr="")
        return SimpleNamespace(returncode=0, stdout="", stderr="")

    monkeypatch.setattr(release, "environment_size", lambda ignored: next(sizes))
    monkeypatch.setattr(release.subprocess, "run", fake_run)

    result = release.measure_full_pyside(environment)

    assert result["essentials_bytes"] == 100
    assert result["full_bytes"] == 250
    assert all(command[0] != str(python) for command in commands)
    assert not any("uninstall" in command for command in commands)


def test_qt6_entrypoint_does_not_enable_deprecated_high_dpi_attributes():
    source = (ROOT / "InstPlot.py").read_text(encoding="utf-8")

    assert "AA_EnableHighDpiScaling" not in source
    assert "AA_UseHighDpiPixmaps" not in source


def test_font_fallbacks_skip_unavailable_named_fonts():
    assert InstPlot._font_families(
        "Times New Roman",
        available={"Times New Roman", "Heiti TC"},
    ) == ["Times New Roman", "Heiti TC", "sans-serif"]
    assert InstPlot._font_families(
        "Helvetica",
        available=set(),
    ) == ["sans-serif"]
