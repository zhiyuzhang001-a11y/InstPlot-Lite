import os
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts.install import InstallError, _open_log, _probe_python, install, launcher_content


def _project(tmp_path):
    root = tmp_path / "中文 project path"
    (root / "scripts").mkdir(parents=True)
    for relative in ["pyproject.toml", "requirements.lock", "InstPlot.py"]:
        (root / relative).write_text("fixture\n", encoding="utf-8")
    (root / "scripts" / "verify_install.py").write_text("pass\n", encoding="utf-8")
    return root


class FakeRunner:
    def __init__(self, root):
        self.root = root
        self.commands = []

    def __call__(self, command, **_kwargs):
        command = [str(part) for part in command]
        self.commands.append(command)
        if command[-2:] == ["-c", "import sys; print('.'.join(map(str, sys.version_info[:2])))"]:
            return SimpleNamespace(returncode=0, stdout="3.12\n", stderr="")
        if command[1:3] == ["-m", "venv"]:
            python = self.root / ".venv" / "bin" / "python"
            python.parent.mkdir(parents=True)
            python.write_text("", encoding="utf-8")
            python.chmod(0o755)
        return SimpleNamespace(returncode=0, stdout="ok\n", stderr="")


def test_install_logs_are_atomically_unique(tmp_path):
    first_path, first_log = _open_log(tmp_path)
    second_path, second_log = _open_log(tmp_path)
    first_log.close()
    second_log.close()

    assert first_path != second_path
    assert first_path.is_file()
    assert second_path.is_file()


def test_dry_run_is_zero_write_and_preserves_unicode_space_path(tmp_path):
    root = _project(tmp_path)
    runner = FakeRunner(root)

    result = install(
        root,
        python_command=["python3.12"],
        platform_name="darwin",
        runner=runner,
    )

    assert result["mode"] == "dry-run"
    assert result["state"] == "missing"
    assert result["project_root"] == str(root.resolve())
    assert not (root / ".venv").exists()
    assert not (root / "run_instplot.command").exists()


def test_apply_uses_argument_lists_and_creates_owned_launcher(tmp_path):
    root = _project(tmp_path)
    runner = FakeRunner(root)

    result = install(
        root,
        apply=True,
        python_command=["python3.12"],
        platform_name="darwin",
        runner=runner,
    )

    assert result["state"] == "healthy"
    assert any(command[1:3] == ["-m", "venv"] for command in runner.commands)
    assert any("--require-hashes" in command and str(root / "requirements.lock") in command for command in runner.commands)
    launcher = root / "run_instplot.command"
    assert launcher.read_text(encoding="utf-8") == launcher_content("darwin")
    if os.name != "nt":
        assert launcher.stat().st_mode & 0o111


def test_broken_environment_requires_explicit_repair(tmp_path):
    root = _project(tmp_path)
    (root / ".venv").mkdir()

    with pytest.raises(InstallError) as raised:
        install(
            root,
            apply=True,
            python_command=["python3.12"],
            platform_name="darwin",
            runner=FakeRunner(root),
        )

    assert raised.value.code == "repair_required"


def test_symlink_environment_and_modified_launcher_are_conflicts(tmp_path):
    root = _project(tmp_path)
    outside = tmp_path / "outside"
    outside.mkdir()
    (root / ".venv").symlink_to(outside, target_is_directory=True)
    with pytest.raises(InstallError) as raised:
        install(root, apply=True, platform_name="darwin", runner=FakeRunner(root))
    assert raised.value.code == "conflict"

    (root / ".venv").unlink()
    (root / "run_instplot.command").write_text("user content\n", encoding="utf-8")
    with pytest.raises(InstallError) as raised:
        install(root, apply=True, platform_name="darwin", runner=FakeRunner(root))
    assert raised.value.code == "conflict"


@pytest.mark.parametrize(
    ("platform_name", "launcher_name", "anchor"),
    [
        ("win32", "run_instplot.bat", "%~dp0"),
        ("darwin", "run_instplot.command", "SCRIPT_DIR"),
        ("linux", "run_instplot.sh", "SCRIPT_DIR"),
    ],
)
def test_launcher_templates_are_relative_to_their_own_location(platform_name, launcher_name, anchor):
    content = launcher_content(platform_name)
    assert anchor in content
    assert ".venv" in content
    assert "-m InstPlot" in content
    assert launcher_name.endswith((".bat", ".command", ".sh"))


@pytest.mark.parametrize("version", ["3.10", "3.11", "3.12", "3.13", "3.14"])
def test_all_supported_python_versions_are_accepted(tmp_path, version):
    def probe(command, **_kwargs):
        return SimpleNamespace(returncode=0, stdout=f"{version}\n", stderr="")

    assert _probe_python(["python"], tmp_path, probe)


@pytest.mark.parametrize("version", ["3.9", "3.15", "4.0"])
def test_unsupported_python_version_is_rejected(tmp_path, version):
    root = _project(tmp_path)
    runner = FakeRunner(root)

    def wrong_version(command, **kwargs):
        result = runner(command, **kwargs)
        if command[-2:] == ["-c", "import sys; print('.'.join(map(str, sys.version_info[:2])))"]:
            result.stdout = f"{version}\n"
        return result

    with pytest.raises(InstallError) as raised:
        install(
            root,
            python_command=["python"],
            platform_name="linux",
            runner=wrong_version,
        )
    assert raised.value.code == "unsupported_python"


def test_dependency_check_failure_marks_existing_environment_for_repair(tmp_path):
    root = _project(tmp_path)
    python = root / ".venv" / "bin" / "python"
    python.parent.mkdir(parents=True)
    python.write_text("", encoding="utf-8")
    python.chmod(0o755)

    def unhealthy(command, **_kwargs):
        command = [str(part) for part in command]
        if command[-2:] == ["-c", "import sys; print('.'.join(map(str, sys.version_info[:2])))"]:
            return SimpleNamespace(returncode=0, stdout="3.12\n", stderr="")
        if command[-3:] == ["-m", "pip", "check"]:
            return SimpleNamespace(returncode=1, stdout="missing dependency\n", stderr="")
        return SimpleNamespace(returncode=0, stdout="ok\n", stderr="")

    result = install(
        root,
        python_command=["python3.12"],
        platform_name="darwin",
        runner=unhealthy,
    )
    assert result["state"] == "repair-needed"
    assert result["actions"] == ["repair-required", "create-launcher"]


def test_failed_apply_returns_nonzero_boundary_and_readable_log(tmp_path):
    root = _project(tmp_path)
    runner = FakeRunner(root)

    def fail_venv(command, **kwargs):
        result = runner(command, **kwargs)
        if [str(part) for part in command][1:3] == ["-m", "venv"]:
            return SimpleNamespace(returncode=9, stdout="", stderr="forced failure\n")
        return result

    with pytest.raises(InstallError) as raised:
        install(
            root,
            apply=True,
            python_command=["python3.12"],
            platform_name="darwin",
            runner=fail_venv,
        )

    assert raised.value.code == "command_failed"
    log_path = Path(raised.value.log_path)
    assert log_path.is_file()
    assert "forced failure" in log_path.read_text(encoding="utf-8")
