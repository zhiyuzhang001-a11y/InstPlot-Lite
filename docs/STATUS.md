# InstPlot Lite status

Updated: 2026-09-11

> [!NOTE]
> InstPlot Lite is now the current user-facing product. The public v0.2.0
> unsigned preview contains the core workflow; the current development branch
> additionally includes consistent dark application content, mouse-wheel zoom,
> bundled license files, and white PNG export with black ticks. These changes
> are intended for the next preview package.

## Overall objective

Ship a small native Windows, macOS, and Linux application for importing numeric
text data, drawing basic curves, zooming and panning, deleting points with
undo/redo, and exporting cleaned data. End users must not need Python, Rust, or
a terminal.

## Completed

- L0 scope and interaction contract frozen in `docs/SCOPE.md`.
- Rust project promoted to the repository root after the historical Python
  edition moved to its own source-only archive repository.
- Rust 1.98.0 minimal development toolchain installed without changing shell
  startup files.
- Native eframe/egui window and egui_plot prototype implemented.
- Curves default to a same-color point-and-line presentation with 3.5-point
  markers for easier point identification and deletion.
- Wheel zoom and right-button pan configured in the prototype.
- OFL-licensed Noto Sans SC subset bundled for common Simplified Chinese,
  Latin, Greek, mathematical symbols, and punctuation.
- Release-size optimization enabled (LTO, one codegen unit, stripped symbols).
- Three-platform GitHub Actions build workflow added.
- TXT/CSV/DAT/TSV import implemented with UTF-8/UTF-16/GBK decoding, separator
  and header detection, strict row-width validation, and numeric-column storage.
- Native XLSX/XLS import maps each valid numeric worksheet to an independent
  dataset and skips empty or notes-only worksheets without requiring Excel.
- File-picker and drag-and-drop import are connected to the native interface.
- PNG export captures the complete plot, tick values, X/Y labels, and legend on
  a white background with black labels/ticks, light-gray grids, and unchanged data-series colors;
  it excludes the toolbar, sidebar, and status bar.
- Retained numeric rows can be exported from the active dataset as CSV, XLSX,
  TSV, or tab-delimited TXT; deleted rows are omitted by design. All datasets
  can be exported to one multi-sheet XLSX or separate collision-safe text files.
- Visible-range min/max decimation replaces fixed-stride sampling so narrow
  peaks are retained while plotted point counts stay tied to screen width.
- Short left-click selects the nearest point within an eight-pixel tolerance;
  left-drag selects all complete-data rows inside a rectangle.
- Pending deletion is highlighted in yellow and requires confirmation. Undo
  and redo retain row indices rather than full copies and are bounded to 256
  commands or 64 MiB.
- Clicking anywhere in the plot keeps a compact `(x, y)` value visible in the
  single-line bottom bar. Clicking near a data point snaps to that point's exact
  coordinates and retains the existing deletion workflow.
- The interface now uses rounded, larger controls and a 13-point minimum text
  size. Status text uses the 15-point body size, axis titles use bold 17-point
  text, and the plot reserves a 36-point left gutter for long Y-axis labels.
- Dataset and X/Y controls now occupy a content-aware 185–280-point left sidebar
  on desktop-sized windows. Dropdowns retain a right inset, while reset/clear
  buttons use their natural text widths instead of stretching across the panel.
  Below 820 points wide the controls return to a compact horizontal layout.
- Only the primary **打开文件** and **数据处理** actions use bold labels; export
  and edit actions retain normal weight so the toolbar has a clear hierarchy.
- The native processing window now has 18-point horizontal and 14-point
  vertical content margins. Its execute actions are bold, while undo/redo use
  directional labels (`← 撤销` and `重做 →`) with explanatory tooltips.
- Local flattening now includes a visible one-sentence purpose description and
  parameter tooltips for transition width, anchor, and strength.
- The Python reference processing core was audited and ported to native Rust:
  midpoint centering, the legacy center-then-top-20 normalization sequence,
  polynomial background subtraction (orders 0–5), locally anchored flattening
  with cosine transitions, and Savitzky–Golay denoising over finite segments.
- Processing can overwrite the selected source column (the default) or retain
  a derived numeric column. The choice applies consistently to every operation;
  both forms can be undone and redone as one atomic multi-curve command.
- Formula processing applies expressions to the axis they reference (`x` or
  `y`), supports parameters and common mathematical functions, and accepts
  constant expressions such as fractions and parenthesized values for formula
  coefficients and custom-fit initial values.
- Processing can target the active curve, an explicit multi-selection, or all
  curves. Each batch validates every target before changing any dataset.
- A command-line check mode supports remote and CI validation without opening a
  GUI window.
- Positional file paths are accepted so desktop launchers can open data files
  without exposing command-line options to end users.
- Native packaging definitions now cover macOS DMG, Windows Inno Setup, Linux
  DEB, and a Linux portable archive. The CI workflow builds and smoke-tests
  these artifacts on their native operating systems.
- A consolidated `THIRD_PARTY_NOTICES` file and the complete MIT, Liberation
  Sans OFL, and Noto Sans SC OFL texts are now included in every native package.
  CI verifies the installed Windows and Linux license files; the macOS DMG keeps
  them both in a visible folder and inside the installed app bundle.
- Fifty-eight local tests pass with no compiler warnings, including real XLS and
  Chinese-header fixtures, multi-sheet XLSX round trips, and column-misalignment
  and no-overwrite regression cases.

## L1 measurements: macOS arm64

- Optimized v0.2 release executable: 6,813,392 bytes (6.50 MiB).
- Current empty-window sampled RSS: 108,256 KiB (105.72 MiB).
- Renderer baseline with a minimal font: 99,888 KiB (97.55 MiB).
- Full macOS STHeiti font experiment: 208,496 KiB (203.61 MiB), rejected.
- Bundled font asset: 1,063,072 bytes (1.01 MiB).

The binary-size gate passes by a wide margin. The calibrated macOS empty-window
RSS gate is 110 MiB because the Cocoa/OpenGL renderer alone measures about
97.55 MiB; the bundled Chinese font adds only about 3 MiB.

The five native processing operations added about 113 KiB to the executable
and 0.72 MiB to the sampled empty-window RSS. They add no Python, NumPy, or
SciPy runtime. A derived `f64` column requires approximately 8 bytes per row;
processing history stores only the recipe and column identity.

## Current state

L0, the local L1 gate, and the local L2 import/export/rendering slice are
complete. A real CSV was imported and exported end to end as a valid
866 x 547 plot-only PNG (43,270 bytes). The import/export smoke run peaked at
113.03 MiB because it temporarily holds the window screenshot and PNG buffers;
the calibrated empty-window gate still passes at 100.83 MiB.

A generated one-million-row, two-column UTF-8 CSV (16,343,948 bytes) completed
the full import, render, PNG capture, and automatic shutdown path in 0.89
seconds on the current macOS arm64 machine. Peak RSS was 196,149,248 bytes
(187.06 MiB), including the native renderer, imported arrays, and screenshot
buffers. A fast path for ordinary unquoted delimited rows reduced this smoke
run from 8.43 seconds to 0.89 seconds.

All six user-provided files in `test_files` now pass the no-window import check.
This set covers GBK Chinese headers, CRLF, trailing delimiters, a CSV containing
a text timestamp column, tab-separated headers containing spaces, and a
Quantum Design `[Header]`/`[Data]` file. The fixes are protected by synthetic
regression tests so CI does not depend on personal measurement files.

The L3 deletion and history implementation is locally complete and the updated
window is running for hands-on interaction verification. Data edits preserve
the current plot transform.

Changing either the X or Y selector now clears the previous visible-range
filter, cancels stale point selections, and fits the plot to the new column pair
in the same frame. Previously the new label could appear while the curve was
still filtered by the old X range.

The lightweight processing slice is locally complete. Symmetry, normalization,
background removal, local flattening, denoising, and formula calculation are
available through a separately movable native **数据处理** window. They preserve
NaN/Inf positions and can be undone and redone; users can choose a single
curve, several selected curves, or all curves, with a window-wide overwrite or
retain-derived-columns policy. The denoising implementation reuses fixed
convolution weights in the interior rather than fitting a new polynomial for
every row. Current-dataset data export also offers explicit column selection.

The complete source history is merged into `main`. GitHub CI has built and
validated the Windows x64 installer, macOS arm64 and x86_64 DMGs, Linux amd64
DEB, and Linux x86_64 portable archive. The Windows installer completed silent
install, bundled importer execution, and uninstall; the Linux DEB completed the
same installation lifecycle; both DMGs passed structure, checksum, bundled
import, and disk-image verification.

The verified artifacts are published in the
[v0.2.0 unsigned preview](https://github.com/zhiyuzhang001-a11y/InstPlot-Lite/releases/tag/v0.2.0),
together with a five-entry SHA-256 checksum manifest. The release is
intentionally marked as a prerelease because macOS Developer ID
signing/notarization and Windows Authenticode signing require owner-provided
developer certificates. Mouse interaction still benefits from a final hands-on
user-session check, while the import-to-PNG path is automated.

## Next step

The unsigned v0.2.0 preview release is complete. The formula, multi-curve,
column-selection, and revised overwrite/undo workflow is ready for the next
preview release after cross-platform CI packages it. Stable-release promotion
still requires release-owner credentials or hands-on devices:

1. Configure Apple Developer ID signing and notarization.
2. Configure Windows Authenticode signing.
3. Perform the final mouse-interaction check on clean user machines.
4. Rebuild signed packages and promote a prerelease to a stable release.
