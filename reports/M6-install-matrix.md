# M6 install matrix

## M6.1 core and macOS workflow — 2026-08-23 — IMPLEMENTED

- Added a standard-library-only, dry-run-first installer with fixed states `missing`, `healthy`, `repair-needed`, and
  `conflict`. It validates project inputs, rejects a symlink/non-directory `.venv` and modified launcher, accepts only
  CPython 3.12, executes argument arrays without a shell, installs the hash lock and project into project-local
  `.venv`, runs `pip check`, and invokes the installed-runtime smoke test in Python isolated mode.
- Apply writes timestamped UTF-8 logs under `.install-logs`. Healthy reruns do not reinstall. An unhealthy environment
  exits nonzero unless the caller explicitly supplies `--repair`; failures retain the command output and log path.
  Launchers are exclusively created and never overwrite existing differing content.
- Added `install_windows.bat`, executable `install_macos.command`, and executable `install_linux.sh`. They use their
  own location rather than the current directory, forward `--repair`, never invoke sudo/PowerShell/curl, and launch
  the generated system-specific runner after successful installation. Missing Python/uv produces instructions and a
  nonzero exit rather than a silent system mutation.
- Failure-first installer tests initially failed because `scripts.install` and `scripts.verify_install` did not exist.
  Directed matrix now passes 13 tests, including Unicode/space paths, zero-write dry-run, Python version rejection,
  command-array construction, environment/launcher conflicts, dependency-health detection, readable failure logs,
  entrypoint safety and installed TXT/XLSX/CSV/PNG/SVG workflows.

## Real macOS installation matrix

- Source was copied to `/tmp/plotapp-m6-install.sEacgM/中文 安装 project`. The current working project remained free
  of `.venv`, generated launchers and `.install-logs` after dry-run.
- First apply selected uv-managed CPython 3.12, created `.venv`, installed `requirements.lock` with hashes, installed
  the project, passed `pip check`, created `run_instplot.command`, and passed isolated installed-runtime verification:
  TXT/XLSX import, CSV/XLSX export, plotting, PNG output and 67 SVG resources.
- A healthy second apply returned no actions. The first repeated check exposed two counterexamples which were fixed:
  standard macOS venv Python is legitimately a symlink, and a probe run from the project directory can import source
  instead of the installed wheel. Health/verification now uses `-I`; the confirmed module path is inside the venv
  `site-packages`.
- Removing chardet from the temporary environment changed dry-run to `repair-needed`. Apply without `--repair`
  returned exit 1 and a timestamped log. Explicit repair reinstalled the lock/project, reran verification and returned
  healthy; final `pip check` passed.
- Full local warnings-as-error suite passed `251 passed in 4.14s`; shell syntax, compilation and `git diff --check`
  passed. Windows and Linux wrappers have static/pure-core coverage but no native runtime execution yet, so M6 and the
  cross-platform part of M5 remain pending rather than being reported complete.

## Native CI matrix — 2026-08-23 — COMPLETE

- Added `.github/workflows/native-install.yml` for `ubuntu-latest`, `macos-latest`, and `windows-latest`, with read-only
  repository permission and explicit CPython 3.12. It uses the current official major examples
  `actions/checkout@v6` and `actions/setup-python@v6`.
- Each job runs the actual platform entrypoint with `INSTPLOT_INSTALL_ONLY=1`, repeats the healthy installation,
  removes chardet, proves an unapproved repair fails, repairs through the same entrypoint, then runs `pip check`, the
  isolated installed-runtime smoke and the full pytest suite. The environment flag only suppresses launching the GUI
  after installation; interactive user behavior is unchanged.
- Two failure-first tests froze the three runners, all entrypoints, repair, smoke, pytest, permissions and nonlaunching
  wrapper mode. Directed installer/CI suite passed 15 items; final local warnings-as-error suite passed
  `253 passed in 4.11s`, shell syntax, YAML parsing, compilation and `git diff --check` passed.
- After authorization, the accumulated project work was committed to `codex/m6-native-ci` and the workflow was run
  on GitHub. The first run exposed deprecated table-form setuptools license metadata on all platforms; it was replaced
  with SPDX `MIT` plus `license-files`. A second run confirmed macOS and exposed two platform-specific CI assumptions:
  Ubuntu lacked the host `libEGL.so.1` runtime and Windows cannot validate POSIX execute bits. The final workflow
  provisions Ubuntu's `libegl1`, scopes execute-bit assertions to POSIX, preserves the expected nonzero repair gate in
  PowerShell, and prints installer logs after failures.
- Final run `32647577404` succeeded on every native runner. Ubuntu passed 253 tests in 11.11s, macOS passed 253 in
  10.77s, and Windows passed 252 with one intentional POSIX-only skip in 11.97s. Every runner completed first install,
  healthy repeat, dependency removal, repair-required rejection, explicit repair, `pip check`, isolated installed
  TXT/XLSX/CSV/PNG/67-SVG smoke, and the full test suite.
- M6 is complete. The installer still does not elevate privileges or silently install host software; Linux users need
  a distribution package providing `libEGL.so.1` before the PySide6 GUI can run.
- A later Windows rerun exposed a real low-resolution clock collision when two failure logs received the same
  microsecond timestamp. Log creation now retains the timestamp prefix while using the operating system's atomic
  unique-file allocation. A dedicated collision regression increased the suite to 254 tests; the current native
  matrix passes 254 tests on Ubuntu/macOS and 253 plus one POSIX-only skip on Windows.

## Zero-Python prerequisite amendment — 2026-08-24 — COMPLETE

- User-facing entrypoints no longer invoke a preinstalled Python. They prefer an existing uv, otherwise download the
  pinned uv 0.12.5 installer, verify its pinned SHA-256 and install uv under project-local `.installer/uv` without PATH
  or shell-profile changes. uv may then download a managed compatible CPython when the host has none.
- Project and installer metadata now accept CPython 3.10–3.14, matching PySide6 Essentials 6.11.2's published
  `>=3.10,<3.15` boundary. The universal hash lock is regenerated from Python 3.10 and contains version markers for
  dependencies whose supported releases differ by Python minor version.
- Local macOS bootstrap downloaded and verified the official installer, created uv 0.12.5 at the expected project-local
  path and located a compatible interpreter. GitHub Actions run `32679629432` then forced the same bootstrap on
  Windows, macOS and Ubuntu, downloaded managed CPython 3.14.7 on each, and passed first/repeat/repair workflows.
- The compatibility jobs passed the complete 271-test suite independently on CPython 3.10, 3.11, 3.12, 3.13 and 3.14.
