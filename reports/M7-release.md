# M7 release validation

## 2026-08-23 — AUTOMATION COMPLETE / PENDING_USER_VALIDATION

- Added failure-first release checks. Initial collection failed because `scripts.verify_release` did not exist;
  the release boundary is now represented by an executable standard-library verifier and regression tests.
- README release metadata now matches version `1.0.0`, lists all three systems, removes placeholder contact links,
  documents CPython 3.12, 1 GB free space, Linux EGL prerequisites, repair/conflict handling, diagnostic locations,
  supported format boundaries and `PENDING_USER_VALIDATION` for unknown instrument variants.
- `pyproject.toml` now publishes the real repository and issue tracker URLs.
- Local `uv 0.12.5` regenerated `requirements.lock` byte-for-byte from `pyproject.toml` with Python 3.12,
  universal resolution and hashes. The same operation is now a dedicated CI release job.
- The footprint probe initially installed and removed full PySide6 in the test environment. Although its measurement
  passed, uninstalling shared Qt files damaged later imports on all three systems. The final probe copies the environment
  to a temporary directory, measures and mutates only that copy, then proves the original environment remains healthy.
- GitHub Actions run `32648822899` passed on commit `61759a0`: Ubuntu and macOS each ran `258 passed`; Windows ran
  `257 passed, 1 skipped` for its intentionally POSIX-only executable-bit check. Each system also passed first install,
  healthy repeat, repair gating, explicit repair, `pip check`, installed-runtime smoke checks, release documentation and
  byte-reproducible lock validation.
- Same-run logical footprint results with PySide6 `6.11.2`:

  - Linux: Essentials `635,202,444 bytes` (`605.78 MiB`), full `1,068,298,618 bytes` (`1,018.81 MiB`), saving
    `433,096,174 bytes` (`40.54%`).
  - macOS arm64: Essentials `638,141,562 bytes` (`608.58 MiB`), full `1,518,455,076 bytes` (`1,448.11 MiB`), saving
    `880,313,514 bytes` (`57.97%`).
  - Windows x64: Essentials `569,322,737 bytes` (`542.95 MiB`), full `1,021,148,587 bytes` (`973.84 MiB`), saving
    `451,825,850 bytes` (`44.25%`).
- CI evidence: `https://github.com/zhiyuzhang001-a11y/InstPlot/actions/runs/32648822899`.
- No real instrument file was available. Automated fixtures remain accepted, while real TXT/DAT/VSM/XLS/XLSX
  validation remains pending rather than being inferred from CI.

## 2026-08-24 — native desktop follow-up

- Ran `INSTPLOT_INSTALL_ONLY=1 ./install_macos.command` against the existing healthy environment. It atomically created
  the missing `run_instplot.command`; the next installer dry-run reported `healthy`, an identical launcher and no action.
- Launched through `run_instplot.command` with the native Cocoa backend. The project `.venv` process remained alive and
  `~/Library/Logs/InstPlot/instplot.log` recorded `diagnostics_started` and `application_start`, closing the macOS desktop
  startup item without relying on the offscreen CI backend.
- The first native run exposed deprecated Qt 6 high-DPI attributes and repeated missing-`SimHei` font messages. Added two
  failure-first release tests, removed the Qt 5-era attributes (high DPI is always enabled in Qt 6), and filtered named
  font fallbacks against Matplotlib's installed font list while retaining a generic fallback. The repeated native launch
  produced no terminal warning output; local regression is `260 passed`.
- Repository history contains no instrument sample beyond the fixed parser `.xls` fixture, and the GitHub repository has
  no issue carrying an external sample. Real instrument validation therefore remains the only M7 user-supplied gate.
- Final GitHub Actions run `32678530521` passed the updated release gate and full native matrix: Ubuntu/macOS
  `260 passed`, Windows `259 passed, 1 skipped`. Evidence:
  `https://github.com/zhiyuzhang001-a11y/InstPlot/actions/runs/32678530521`.

## 2026-08-24 — zero-Python prerequisite and compatibility extension — COMPLETE

- Public Windows, macOS and Linux entrypoints now execute through uv instead of requiring a preinstalled Python.
  When uv is absent, a pinned uv 0.12.5 installer is downloaded, matched against a repository-pinned SHA-256 and
  installed under `.installer/uv` without changing PATH or a shell profile. uv may supply a managed interpreter.
- Project metadata, installer selection and environment health accept CPython 3.10–3.14. The universal hash lock is
  regenerated from Python 3.10 with per-version dependency markers; byte-for-byte regeneration passes.
- The macOS entrypoint completed a forced project-local uv bootstrap and healthy install. A separate clean uv-managed
  CPython 3.10.21 environment passed hashed dependency installation, project wheel installation, `pip check`, isolated
  TXT/XLSX/CSV/PNG/67-SVG smoke and the complete `271 passed` suite.
- GitHub Actions run `32679629432` passed all nine jobs. Windows, macOS and Ubuntu forced the verified project-local uv
  bootstrap, each downloaded managed CPython 3.14.7, then passed first install, healthy repeat, repair, footprint, smoke
  and full regression. Ubuntu/macOS each passed 271 tests; Windows passed 270 with one POSIX-only skip.
- Dedicated Ubuntu jobs on CPython 3.10, 3.11, 3.12, 3.13 and 3.14 each passed hashed installation, `pip check`, smoke
  and all 271 tests. Real instrument samples remain the only user-supplied validation item.

## 2026-08-24 — Linux EGL preflight — COMPLETE

- The Linux entrypoint now checks the dynamic linker cache and standard multi-architecture library directories for
  `libEGL.so.1` before downloading uv, Python or project dependencies.
- If missing, it reads distribution identifiers without sourcing `/etc/os-release`, prints a package-manager-specific
  command for Debian/Ubuntu, Fedora/RHEL derivatives, Arch derivatives or openSUSE, and exits without invoking sudo.
- Local shell syntax, the missing-library path and the complete 272-test suite pass. GitHub Actions run `32680280641`
  passed all nine jobs: every Python 3.10–3.14 compatibility job and native Ubuntu/macOS passed 272 tests; Windows
  passed 271 with one POSIX-only skip. Ubuntu passed the real EGL-present branch before its installer workflow.

## 2026-08-24 — click install and user-visible launchers — AUTOMATION COMPLETE

- Windows creates a Desktop `InstPlot.lnk` targeting project-local `pythonw.exe -m InstPlot`, so later GUI launches do
  not require a command prompt. macOS creates an executable Desktop `InstPlot.command` that detaches the GUI and exits;
  Linux installs `InstPlot.desktop` in the per-user application menu.
- Shortcut creation is repeatable. An existing different file, symlink or Windows shortcut is retained instead of being
  overwritten. Project-local `run_instplot.*` remains the recovery path.
- Failure-first launcher and archive tests pass. Local archives are Windows `214 KiB`, macOS `215 KiB` and Linux
  `156 KiB`; they contain only required source/runtime files, and the macOS/Linux executable modes survive packaging.
- GitHub Actions run `32681539816` passed all nine jobs. The native jobs proved the actual Windows `.lnk`, macOS
  `.command` and Ubuntu `.desktop` paths exist. Python 3.10–3.14 and native Ubuntu/macOS each passed 279 tests;
  Windows passed 278 with one POSIX-only skip. The `InstPlot-installers` artifact contains all three archives.
- Publishing these files as permanent, public GitHub Release v1.0.0 assets remains pending explicit user authorization;
  CI artifacts are validation/download intermediates rather than the final student-facing distribution page.
