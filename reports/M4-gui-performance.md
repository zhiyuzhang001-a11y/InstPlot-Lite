# M4 GUI and performance record

## M4.1 baseline and warning cleanup — 2026-08-23 — COMPLETE

- Failure-first warning escalation reproduced both CSV append tests failing at deprecated
  `QMessageBox.setButtonText`. The three calls now retrieve each standard button and call its supported `setText`.
  The directed warnings-as-error replay passed `2 passed`.
- Added `scripts/benchmark_gui.py` and a small structured-output test. Initial failure-first collection reported the
  missing module. Its first implementation used an unsupported `.tsv` suffix and correctly failed through
  `DataIOError(unsupported_extension)`; the deterministic tab-delimited fixture now uses supported `.txt`.
- Formal local baseline: macOS 26.5 arm64, Python 3.13.5, 100,000 rows, three measured runs (and three cold starts).
  Median elapsed: cold startup `1.100853417s`; text import `1.420332000s`; center+denoise `0.002099042s`;
  polynomial fit `0.003529917s`; rectangle scan `0.000556875s`; full redraw `0.062301833s`; pan event including
  synchronous draw `0.036027334s`.
- Median tracemalloc peak: startup parent measurement `63,672`; import `54,861,214`; processing `5,001,946`;
  fit `8,066,856`; rectangle `703,800`; redraw `9,440,678`; pan `1,681,790` bytes. Cold startup memory is only
  parent-side subprocess overhead and must not be treated as child process RSS.
- Full suite with DeprecationWarning escalated passed `204 passed in 3.55s`; SyntaxWarning compilation and
  `git diff --check` passed with zero Python warnings. The run exposed repeated Matplotlib `SimHei` fallback messages
  on this macOS host; changing the font can affect appearance, so this is recorded for M4.4 rather than silently
  changed in M4.1.

## M4.2 bounded dialog extraction — 2026-08-23 — COMPLETE

- Failure-first tests could not import `instplot_dialogs`; after the first extraction, export/I/O/GUI directed tests
  passed 91 items. The main-window export-column method is now a three-line compatibility adapter, and the existing
  monkeypatch surface remains valid.
- A second failure-first test found no common file-selection builder. Added shared file checkbox-table and dialog
  button factories, then reused them in denoise and local-flatten dialogs. Offscreen tests freeze row order,
  basenames, widths and default checked state.
- `InstPlot.py` is 6,018 lines after the bounded extraction. Unique warning messages were deliberately not hidden
  behind a generic wrapper, which would add indirection without consolidating policy. The 2,241-line publication
  dialog state machine and all processing callbacks remain in place.
- Directed M4.2 suite passed 98 tests; full DeprecationWarning-as-error suite passed `208 passed in 3.73s`.
  Compilation and `git diff --check` passed. M4.3 lifecycle planning is next.

## M4.3.1 task lifecycle and M4.3.2 async import — 2026-08-23 — COMPLETE

- Failure-first task tests could not import `instplot_tasks`. Added a Qt thread-pool controller with cooperative
  cancellation tokens, main-thread success/failure/cancel dispatch, identity-based callbacks, active count and
  bounded wait. A queued-task counterexample found that `QThreadPool.clear()` would strand controller records;
  cancel-all now lets queued runnables start, observe cancellation and emit deterministic completion.
- Failure-first async import tests found no `PlotApp.load_file_async`. Open-file and drag/drop now submit reads to a
  single-worker queue, preserving multi-file order. Only the main-thread callback mutates `loaded_files`, history,
  combos, status and plot. The synchronous `load_file` boundary remains for tests and scripts.
- A blocked-reader test proves a zero-delay Qt heartbeat runs while import is pending. Cancellation after the third-
  party call starts discards its late result and reports cancellation. Clear/new manual data cancels outstanding
  work; close requests cancellation and accepts only after a bounded 3-second worker wait.
- Task/async import/data I/O/smoke directed suite passed 39 tests; full DeprecationWarning-as-error suite passed
  `215 passed in 3.50s`. M4.1 shows ordinary 100k processing and cubic fit at 2.10ms and 3.53ms, so M4.3.3 will first
  benchmark nonlinear/custom fits and add worker overhead only to paths that exceed the interaction threshold.

## M4.3.3 fitting core and M4.3.4 large-fit worker — 2026-08-23 — COMPLETE

- Extracted five fit methods into `instplot_fitting.py` with finite one-dimensional inputs, stable `FitResult`,
  coded `FitError`, restricted custom-expression AST and cooperative cancellation boundaries. The GUI no longer
  embeds SciPy solvers or expression evaluation.
- Formal 100k medians for polynomial/exponential/logarithmic/power/custom were respectively
  `4.616/9.773/4.275/16.281/10.390ms`; all fixture R-squared values were 1.0. At 1,000,000 points the nonlinear
  paths measured `101.57/172.85/109.03ms`, supporting a 250,000-point async threshold.
- A blocked 250k power solver proved the execute click returns in under 0.5s, a zero-delay Qt heartbeat runs while
  work is pending, the result is published afterward, and task state returns to zero. Full warnings-as-error suite
  passed `231 passed`. An unrelated strict-suite resource warning also exposed an unclosed test `ExcelFile`; it now
  uses its context manager.

## M4.4 coalesced interaction rendering — 2026-08-23 — COMPLETE

- Added a reusable 16ms single-shot draw scheduler. Pan, selection rectangle and wheel updates request coalesced
  `draw_idle`; pan release flushes the final view. Full redraw and output paths retain exact rendering.
- The matching 100k/3-run benchmark measured pan submission at `0.000167250s` versus the M4.1 synchronous
  `0.036027334s`, a `99.54%` reduction in per-event main-thread blocking. Full redraw remained `0.062355041s`.
- Rectangle scan (`0.000611708s`) and ordinary processing (`0.001866458s`) remain below the interaction threshold,
  so no invalidation-prone DataFrame cache was added without evidence. Scheduler tests, full warnings-as-error
  suite (`233 passed in 3.75s`), compilation and `git diff --check` passed.

## M4.5 diagnostics and closure — 2026-08-23 — COMPLETE

- Added cross-platform UTF-8 rotating diagnostics (1MB plus three backups), environment override, and a temporary-
  directory fallback if the normal location is unusable. Default locations are `~/Library/Logs/InstPlot` on macOS,
  `%LOCALAPPDATA%\\InstPlot\\Logs` on Windows, and `$XDG_STATE_HOME/instplot` or
  `~/.local/state/instplot` on Linux.
- The top-level exception hook logs a traceback and presents a read-only dialog with an explicit copy-details action.
  Import, fit, export and close lifecycle events are structured; loaded DataFrame contents are no longer printed.
  Window close cancels both pending interaction draws and background work, records a deferred close after the bounded
  three-second wait, and records successful closure.
- Final local warnings-as-error suite passed `238 passed in 3.88s`; compilation and `git diff --check` passed. A final
  CPython 3.12.14 installed-wheel run passed `238 passed in 4.01s`, `pip check`, nine wheel modules, 67 SVG resources,
  offscreen `PlotApp` construction and real log creation. The clean environment, wheels and generated caches were
  moved to `/Users/zhiyu/.Trash/PlotApp-M4-cleanup.sxs93O` and remain recoverable. M4 is complete.
