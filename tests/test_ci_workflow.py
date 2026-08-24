from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_native_install_workflow_covers_all_platforms_and_repair():
    workflow = (ROOT / ".github" / "workflows" / "native-install.yml").read_text(
        encoding="utf-8"
    )
    for runner in ["ubuntu-latest", "macos-latest", "windows-latest"]:
        assert runner in workflow
    for entrypoint in ["install_linux.sh", "install_macos.command", "install_windows.bat"]:
        assert entrypoint in workflow
    assert "actions/checkout@v6" in workflow
    assert "actions/setup-python@v6" in workflow
    assert "contents: read" in workflow
    assert "pull_request_target" not in workflow
    assert "--repair" in workflow
    assert "verify_install.py" in workflow
    assert "pytest" in workflow


def test_entrypoints_support_nonlaunching_ci_mode():
    for name in ["install_windows.bat", "install_macos.command", "install_linux.sh"]:
        content = (ROOT / name).read_text(encoding="utf-8")
        assert "INSTPLOT_INSTALL_ONLY" in content


def test_entrypoints_bootstrap_pinned_uv_without_requiring_python():
    unix_bootstrap = (ROOT / "scripts" / "bootstrap_uv.sh").read_text(encoding="utf-8")
    windows_bootstrap = (ROOT / "scripts" / "bootstrap_uv.ps1").read_text(encoding="utf-8")
    entrypoints = [
        (ROOT / name).read_text(encoding="utf-8")
        for name in ["install_windows.bat", "install_macos.command", "install_linux.sh"]
    ]

    assert "https://astral.sh/uv/0.12.5/install.sh" in unix_bootstrap
    assert "504511fbbbd811aeaba6738abc79408956b6c7da0ca35437b3dcc24a41efc111" in unix_bootstrap
    assert "https://astral.sh/uv/0.12.5/install.ps1" in windows_bootstrap
    assert "ca1ad558c65d31e2d3a24464638aff90bfb81d6c72428b4e71d6f55944a68541" in windows_bootstrap
    assert "UV_UNMANAGED_INSTALL" in unix_bootstrap
    assert "UV_UNMANAGED_INSTALL" in windows_bootstrap
    for content in entrypoints:
        assert ">=3.10,<3.15" in content
        assert "Python was not found" not in content
        assert "未找到 Python" not in content


def test_ci_covers_supported_python_range_and_forced_uv_bootstrap():
    workflow = (ROOT / ".github" / "workflows" / "native-install.yml").read_text(
        encoding="utf-8"
    )

    assert "python-version: ['3.10', '3.11', '3.12', '3.13', '3.14']" in workflow
    assert 'INSTPLOT_FORCE_UV_BOOTSTRAP: "1"' in workflow
    assert "Verify generated user launcher" in workflow
    assert "InstPlot.command" in workflow
    assert "InstPlot.desktop" in workflow
    assert "InstPlot.lnk" in workflow
    assert "build_release_archives.py" in workflow
    assert "actions/upload-artifact@v7" in workflow
    assert "InstPlot-installers" in workflow
