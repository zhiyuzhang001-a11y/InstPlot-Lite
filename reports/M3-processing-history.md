# M3.1 processing and legacy history baseline

- Date: `2026-08-23`
- Stage: `M3.1 — behavior characterization and baseline`
- Result: `COMPLETE_PENDING_INDEPENDENT_VERIFICATION`
- Production code changed: `none`

## Scope completed

- Extended `tests/test_processing.py` to characterize center, normalize, local flatten, denoise and the GUI-embedded
  polynomial background path without changing `InstPlot.py`.
- Added `tests/test_history.py` to inventory all eight `copy.deepcopy(self.loaded_files)` owners, prove legacy
  snapshot identity/order/index/dtype/value isolation, and freeze current undo observer behavior.
- Added `scripts/benchmark_history.py`, a deterministic standard-library/NumPy/Pandas baseline for retained full
  snapshots. It reports logical payload, step timing, `tracemalloc` and process max RSS, then clears data and runs GC.

## Behavior matrix

- Center: normal midrange and input non-mutation pass; empty raises NumPy `ValueError`; all-NaN and Inf preserve
  legacy NaN/Inf results with RuntimeWarning.
- Normalize: normal top-N average/clamping, empty/all-NaN and zero scale are fixed; all-negative values currently
  use a signed negative scale (`[-3,-2,-1] -> [-1,-1,1]`, scale `-1.5`); Inf scale produces `[0,nan]` with warning.
- Local flatten: left anchor, no-op short selection, invalid anchor, unsorted X and repeated X are fixed. Repeated X
  currently returns unchanged values with a poorly-conditioned fit warning.
- Denoise: short/invalid windows, valid linear signal, bounded range and input non-mutation are fixed. A NaN in a
  five-point window currently spreads to all NaN; an Inf leaves Inf at its source and NaN elsewhere.
- Background: offscreen linear fit removes an exact linear baseline and saves one pre-change snapshot. Cancelling the
  dialog keeps data unchanged but still consumes one legacy history slot because the snapshot precedes the dialog.

These are legacy characterizations, not endorsements of scientific edge semantics. M3.2 must preserve confirmed
normal results; changing all-negative/near-zero normalization or non-finite handling requires the contract decision
and stable error/no-op rules rather than silently changing formulas.

## Eight legacy history owners

1. `PlotApp.on_click_point`
2. `PlotApp.on_mouse_release`
3. `PlotApp.open_delete_line_dialog.delete_line`
4. `PlotApp.apply_denoise.on_ok`
5. `PlotApp.apply_local_detrend.on_ok`
6. `PlotApp.apply_center`
7. `PlotApp.apply_normalize`
8. `PlotApp.remove_background`

The source inventory is AST-based and fails if an owner is added, removed or renamed. Source audit records the
current publication boundary: delete confirmation paths snapshot after confirmation; denoise/local processing save
before per-file publication; center/normalize save before checking whether any file changes; background saves before
the dialog, so cancel/no-op can occupy history. M3.1 records these defects but does not repair them.

## Legacy memory baseline

Command:

`/Users/zhiyu/miniconda3/bin/python scripts/benchmark_history.py --rows 250000 --files 4 --columns 8 --steps 10`

Final run exit 0:

- fixture payload: `64,000,528 bytes` (`61.036 MiB`)
- each full snapshot: `64,000,528 bytes`
- ten retained snapshots: `640,005,280 bytes` (`610.357 MiB`), exactly ten fixture payloads
- elapsed: `0.054095334 s`; individual copy times `0.003421708–0.004335458 s`
- `tracemalloc` peak: `640,665,799 bytes` (`610.987 MiB`)
- process max RSS on macOS: `799,768,576 bytes` (`762.719 MiB`)
- benchmark clears history and fixture references and runs `gc.collect()` before returning; no fixture is written.

The M3.3/M3.5 comparison must reuse the same dimensions and logical payload calculation. The new ten-step history
payload gate remains at no more than 35% of this legacy retained payload: `224,001,848 bytes`.

## Commands, failures, and results

- First M3.1 run exit 1: `17 passed, 1 failed`. The failure was a fixture-index construction error: Series indexed
  `[0,1]` aligned into DataFrame index `[10,20]`, converting the intended int column to float NaN. Explicit matching
  indexes fixed the test; production code was not changed.
- After the fixture correction: `18 passed in 1.11s`.
- After adding benchmark self-check: `19 passed in 1.14s`.
- After completing non-finite/repeated/unsorted characterization: `23 passed in 1.16s`.
- Final M3.1 suite exit 0: `23 passed in 1.21s`; final full suite exit 0:
  `111 passed, 6 warnings in 2.19s`.
- SyntaxWarning-as-error compilation of `InstPlot.py`, benchmark and both M3.1 tests exit 0; `git diff --check`
  exit 0.
- The six warnings are pre-existing Qt `QMessageBox.setButtonText` deprecations in M2 export tests.

## Deviations, risks, and cleanup

- The draft counted seven full snapshots; AST/source verification found eight and the contract was corrected before
  implementation.
- Background fitting has no pure production function in M3.1, so its behavior is exercised through an offscreen
  dialog with deterministic button/table injection. Extraction remains M3.2.
- No production module, GUI, dependency, lock, installer, I/O behavior, commit, push or external write was changed.
- The budgeted navigation cache was removed at closeout. No test or benchmark process remains.

## Independent M3.1 verification — 2026-08-23 — ACCEPT

- Independently inspected both test files, the benchmark and this report. Expected values come from explicit legacy
  behavior, not from a second call to the implementation under test; the history owner set is independently derived
  from the production AST.
- Pre-M3.1 and verification SHA-256 for `InstPlot.py` are identical:
  `786070ca365846720ca263d27bd38d1c267b15cd3e6c21d6b7412d81c059ded3`.
- Independent AST inventory returned exactly eight owners at lines 735, 860, 1769, 4772, 5005, 5252, 5296 and 5350.
- Independent M3.1 suite exit 0: `23 passed in 1.08s`; full suite exit 0:
  `111 passed, 6 warnings in 1.92s`; compilation and `git diff --check` exit 0.
- Independent full benchmark exit 0: fixture `64,000,528 bytes`, ten snapshots `640,005,280 bytes`,
  `tracemalloc` peak `640,665,799 bytes`, process max RSS `799,277,056 bytes`, elapsed `0.048696333 s`.
- Result: `ACCEPT_M3.1`. Full-negative/zero/near-zero finite normalization remains a compatibility behavior in M3.2;
  it must not be silently replaced by absolute-magnitude normalization. Non-finite and invalid-input contracts remain
  eligible for the explicit M3.2 error/no-op rules.

## M3.2 pure processing core — 2026-08-23 — COMPLETE_PENDING_INDEPENDENT_VERIFICATION

### Implemented boundary

- Added Qt-independent `instplot_processing.py` with `ProcessingError`, immutable `ProcessingResult`, and pure
  center, normalize, local-flatten, denoise and polynomial-background functions.
- Preserved finite legacy results, including signed all-negative, zero and near-zero normalization. NaN/Inf are
  excluded from calculations and retained at their original positions; denoise works on contiguous finite segments.
- Replaced the four legacy functions with signature-compatible thin wrappers and routed both GUI background paths
  through the core. No history/undo/redo or GUI layout code was intentionally changed.
- Added core boundary/error tests and two offscreen partial-success GUI tests. Added the new module to the wheel.

### Failure-first and verification evidence

- Failure-first core run exited 2 during collection with `ModuleNotFoundError: instplot_processing` before the module
  existed. After implementation, targeted processing/GUI/history suite: `35 passed in 1.11s`.
- Local full suite exit 0: `123 passed, 6 warnings in 2.01s`. SyntaxWarning-as-error compilation and
  `git diff --check` exit 0. The six warnings remain the pre-existing Qt button-text deprecations.
- AST/source inventory still finds exactly the same eight `copy.deepcopy(self.loaded_files)` owners; M3.1's
  640,005,280-byte ten-step legacy baseline therefore remains the comparison baseline for M3.3/M3.5.
- A Python 3.13 wheel attempt correctly refused the declared `<3.13` requirement. A metadata-only retry with
  `--ignore-requires-python` confirmed the module list; this was followed by the required target-version run.
- First clean 3.12 installed-wheel test collection failed only because `scripts/benchmark_history.py` was not copied
  beside the tests. That temporary environment was removed; the corrected run copied the test helper as well.
- Clean Conda CPython 3.12.13 installed locked dependencies, built and installed the wheel, and confirmed wheel
  contents `InstPlot.py`, `instplot_io.py`, `instplot_processing.py`. `pip check` passed; project-external full suite
  passed `123 passed, 6 warnings in 50.17s`; offscreen `PlotApp` startup passed.

### Integrity and cleanup

- Current SHA-256: `InstPlot.py` `528808d6ff836d9a9de27d51db286fde9b218505110e27863da8f44a86fc8e45`;
  `instplot_processing.py` `634f28dc6fc6f2ef054678f2b953d7f38d465bcdddc425f5dfca1d0cab2dda74`.
- Both clean-environment attempts and their wheels were removed by validated temporary-root cleanup. No test,
  installer or background process remains. M3.3 was not started.

## Independent M3.2 verification — 2026-08-23 — REJECT

- Production hashes matched the implementer handoff before review. Independent targeted tests passed `35 passed in
  1.17s`; the full suite passed `123 passed, 6 warnings in 2.09s`; compilation, diff check and the unchanged eight
  history owners also passed. These green gates do not cover the following contract counterexamples.
- `center_values(np.ones((2, 2)))` raises bare Pandas `TypeError` before `_values` reaches its intended
  `invalid_dimensions` check. The shared conversion order affects every public core accepting `values`.
- `denoise_values(np.arange(7.0), 4, 2)` silently converts the forbidden even window to five, and a `3.9` window is
  silently truncated to three. Section 4.1 requires stable errors for even and invalid window parameters.
- Under `warnings.simplefilter("error")`, `center_values([1e308, 1e308])` raises a scalar-add overflow warning and
  `normalize_values([-1e308, -5e-324], top_n=1)` raises divide overflow. Both inputs are finite, so the core violates
  the no-warning finite-value invariant. Extreme-X linear/background fits likewise leak overflow warnings.
- NaN interval parameters are not rejected: local flatten and ranged denoise return no-op, while background returns
  `insufficient_fit_points`; all should return a stable invalid-parameter/interval error rather than misclassify the
  request.
- Result: `REJECT_M3.2`. Required correction is one bounded batch: add failure-first tests for the reported cases and
  adjacent infinities/non-numeric/fractional parameters, validate shape before Pandas conversion, validate all scalar
  parameters as finite values of the required type, and make finite extreme arithmetic/fits warning-free or return a
  stable `ProcessingError`. Preserve legacy finite outputs, thin-wrapper behavior and all eight history owners.

## M3.2 correction batch 1 — 2026-08-23 — COMPLETE_PENDING_INDEPENDENT_VERIFICATION

- Failure-first expansion produced `28 failed, 32 passed`: every public core now has multidimensional coverage;
  denoise covers even, fractional, string, bool, NaN and Inf window/order values; local/background/ranged denoise
  cover non-finite, string and complex scalar bounds; extreme finite arithmetic and fits run under warning escalation.
- `_values` now validates dimensionality before Pandas conversion. Shared strict helpers reject non-integer control
  parameters and non-real/non-finite scalar parameters with operation-specific stable codes.
- Center uses half-sum midpoint arithmetic. Normalize computes the signed top-N mean through magnitude scaling and
  divides only unclamped elements, preserving legacy signed-scale results without evaluating overflowing branches.
- SciPy/NumPy filter and fit operations run behind one numerical guard: warnings, floating-point failures and linear
  algebra failures become `ProcessingError(code="numeric_failure")`. Extreme fits may fail explicitly rather than
  leaking warnings or publishing questionable data.
- The pure denoise core now rejects even windows as contracted. The legacy `denoise_data` thin wrapper explicitly
  raises an even window to the next odd value, preserving existing GUI behavior; a compatibility test freezes it.
- An adjacent NumPy-complex parameter test initially exposed `ComplexWarning`; restricting scalar inputs to real
  numbers closed that shared gap. Final processing suite: `63 passed in 1.03s`; processing/GUI/history:
  `70 passed in 1.20s`.
- Reviewer reproducer replay under warnings-as-error returned only expected values or named errors. Fixed-seed finite
  center/normalize comparison against legacy formulas passed. Local full suite: `158 passed, 6 warnings in 2.02s`;
  compilation, diff check and exactly eight history owners passed.
- Clean Conda CPython 3.12.13 installed locked dependencies and the built wheel; wheel contained `InstPlot.py`,
  `instplot_io.py`, `instplot_processing.py`; `pip check` passed; project-external suite passed
  `158 passed, 6 warnings in 49.51s`; offscreen startup passed.
- Final SHA-256: `InstPlot.py` `b7cc7840c49cb4b94967cac395e86d5a04df75b8c669449ff14152d1d7965eef`;
  core `b4dbe1aa5aa97fcc4f7aec1dbd90a1b2e644e0fd1792228c1d20d30eb9e1a81d`. Clean environment,
  wheel/build metadata, bytecode and navigation cache were removed; M3.3 was not started.

## Independent M3.2 correction verification 2 — 2026-08-23 — REJECT

- Production hashes matched the correction handoff. Existing directed tests passed `70 passed in 1.38s`; full suite
  passed `158 passed, 6 warnings in 2.37s`; compilation, diff check and all eight history owners passed.
- Adjacent shared-conversion probe found `center_values([[1.0], [2.0, 3.0]])` and the equivalent normalize call
  leak bare NumPy `ValueError` at `raw = np.asarray(values)`. The conversion `try` begins only afterward, so ragged
  shape construction never reaches `invalid_dimensions`/`invalid_values`.
- `center_values(np.array([1+2j, 3+4j]))` leaks `ComplexWarning` while casting the one-dimensional complex array to
  float. It can also silently discard imaginary components when warnings are not escalated. This violates both the
  stable-error and no-silent-coercion requirements.
- Nullable Pandas float input and scalar dimensional rejection behaved correctly. The defects are not environment
  variation: both reproduce under warnings-as-error before any numerical algorithm runs.
- Result: `REJECT_M3.2_CORRECTION_1`. This is the second failure of the same illegal shape/type conversion invariant;
  per the high-risk contract, stop case-by-case patches. Rebuild and execute one shared conversion matrix covering
  rectangular/ragged/scalar/complex/object/Pandas inputs and every public x/y position, with all array construction,
  coercion and float casting inside one stable-error boundary.

## M3.2 shared-conversion closure — 2026-08-23 — COMPLETE

- Added a full conversion matrix for 0D/1D/2D/ragged inputs, nested object arrays, real/object/nullable/complex
  dtypes, supported coercible strings and every public x/y value position. The corrected failure-first run was
  `16 failed, 70 passed`; one unrelated list-copy assertion in the new test was corrected before production work.
- `_values` now converts array-construction exceptions/warnings to `invalid_dimensions`, rejects nested object-array
  elements, rejects complex data before and after Pandas coercion, and wraps coercion/cast failures as
  `invalid_values`. Supported real inputs still create an independent float copy and preserve the caller object.
- Final local evidence: processing `86 passed in 0.98s`; processing/GUI/history `93 passed in 1.23s`; complete
  project `181 passed, 6 warnings in 2.19s`. An independent probe covering DataFrame 2D, ragged object arrays,
  complex object/dtype arrays, a failing `__array__`, nullable Pandas and mixed strings passed.
- SyntaxWarning compilation, `git diff --check` and the same eight history owners passed. Clean Conda CPython
  3.12.13 installed locked dependencies and wheel; `pip check`, wheel contents, project-external
  `181 passed, 6 warnings in 50.09s` and offscreen startup passed.
- Final SHA-256: `InstPlot.py` `b7cc7840c49cb4b94967cac395e86d5a04df75b8c669449ff14152d1d7965eef`;
  `instplot_processing.py` `12941b7106e5f8fe2a4293e45644d3c47f585a60108acc7261879f90c62e31e0`.
  Clean environment, wheel/build metadata, bytecode and navigation cache were removed. M3.3 production work was
  not started.

## M3.3 differential history core — 2026-08-23 — COMPLETE

- Failure-first collection exited with `ModuleNotFoundError: instplot_history`. The new fixture initially aligned
  Series against a custom index and produced missing values; this test-only construction was corrected before the
  production implementation.
- Added a GUI-free `HistoryManager` with per-entry sidecar identities, bounded undo/redo stacks, drift detection and
  delta payload accounting. Added column patch, row deletion, file deletion and atomic composite commands. Duplicate
  paths, duplicate indexes and a DataFrame shared by multiple list entries are targeted by exact entry identity.
- Core tests: `15 passed`; directed core/legacy/processing suite: `108 passed`; local complete suite:
  `196 passed, 6 warnings in 2.07s`. Compilation, `git diff --check`, and the exact eight legacy
  `copy.deepcopy(self.loaded_files)` owners passed.
- Built wheel contains `InstPlot.py`, `instplot_history.py`, `instplot_io.py` and `instplot_processing.py`. A clean
  CPython 3.12.14 environment installed hashed dependencies plus the wheel; `pip check` passed, external full suite
  passed `196 passed, 6 warnings in 58.74s`, and offscreen startup passed.
- Final SHA-256: `InstPlot.py` `b7cc7840c49cb4b94967cac395e86d5a04df75b8c669449ff14152d1d7965eef`;
  `instplot_history.py` `fcdb2a05d65f43238c32b86b5518ff096c516a8a327980f95ef70f70899b8261`.
  The clean environment and wheel were moved to Trash; local build metadata and caches were removed. Setuptools
  emitted a non-blocking future deprecation for the table-form license metadata; defer that packaging-only cleanup
  to M5/M6. M3.4 GUI integration remains unimplemented.

## M3.4 GUI integration and M3.5 acceptance — 2026-08-23 — COMPLETE

- Five failure-first GUI integration tests all failed against the legacy list history. Migrated all eight snapshot
  owners to column/row/file/composite commands, added a redo toolbar action, and reset both stacks when import,
  manual input, clear, or placeholder replacement begins a new data session.
- Processing now computes against temporary frames and publishes the successful subset once. Cancel, all-failed and
  numerical no-op paths do not consume history. Point and rectangle deletion use row positions, closing duplicate
  index over-deletion; a filtered duplicate-index undo/redo counterexample passes.
- The first direct benchmark invocation failed before fixture allocation because `scripts/` rather than the project
  root was on `sys.path`; the standalone entry was corrected and rerun. Formal 4×250,000×8×10 result:
  legacy `640,005,280 bytes`, differential `160,010,560 bytes`, ratio `25.0014%`, elapsed `28.587972 s`,
  tracemalloc peak `306,337,085 bytes`, and exact ten-step undo/redo roundtrip.
- Local full suite passed `203 passed, 6 warnings in 2.38s`; SyntaxWarning compilation, zero legacy owner gate and
  `git diff --check` passed. Clean CPython 3.12.14 installed the wheel and hashed dependencies: `pip check` passed,
  external suite passed `203 passed, 6 warnings in 51.14s`, wheel contained four modules and 67 SVGs, and offscreen
  startup confirmed manager/list binding.
- Final SHA-256: `InstPlot.py` `303640295e2f460788d33947977d81d8e540b6a520430a654fb83e15b4c82adc`;
  `instplot_history.py` `f5f5abe28da351349e0436128bf573cb2546512785d3fab39e433ce2e94e374a`.
  Temporary clean environment/wheel were moved to Trash. Local build/cache cleanup follows this record. M3 is
  complete; M4.1 measurement planning is next.
