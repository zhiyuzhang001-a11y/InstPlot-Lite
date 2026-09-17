# InstPlot Lite v0.1 contract

## Product objective

Provide the smallest practical cross-platform desktop workflow for importing
numeric instrument data, inspecting a curve, removing unwanted points, undoing
mistakes, and exporting the cleaned data. The historical Python application is
maintained separately as a source-only behavioral reference.

## Required v0.1 workflow

1. Open or drag one or more TXT, CSV, or DAT files.
2. Detect text encoding, delimiter, header row, and numeric columns without
   shifting a header relative to its data.
3. Select X and Y columns and draw line or scatter series.
4. Zoom around the pointer with the wheel and pan with right-button drag.
5. Delete the nearest point with a short left click after confirmation.
6. Delete all points in a left-drag rectangle after confirmation.
7. Preserve the visible plot bounds after deletion, undo, and redo.
8. Export retained rows to TXT or CSV and save a basic PNG screenshot.
9. Reproduce the reference application's lightweight numerical operations:
   center, center-then-normalize, polynomial background removal, local
   flattening, and Savitzky–Golay denoising.
10. Store processing results as derived columns so the imported values remain
    intact and processing can participate in undo/redo.

## Explicitly out of v0.1

- publication plotting, LaTeX, PDF, and SVG
- general nonlinear fitting and SciPy as a runtime dependency
- secondary axes and per-series publication styling
- Excel import/export (candidate for a later optional module)

## Interaction contract

- Wheel: zoom around the pointer.
- Right-button drag: pan the current viewport.
- Short left click: highlight the nearest point within a fixed pixel tolerance,
  show its `(x, y)` coordinates, then request deletion confirmation.
- Left-button drag: show a rectangular selection and request bulk-deletion
  confirmation on release.
- Data edits never reset the current viewport.
- The processing controls use an independently movable native tool window.
- Desktop-width layouts place dataset and axis selectors in a content-aware
  180–220-point left sidebar; widths below 820 points use compact horizontal
  controls.
- Plot layout reserves a 20-point left gutter for the vertical Y-axis label and keeps
  the status/coordinate bar inside the main viewport.
- Undo and redo operate on deletion commands and deterministic processing
  recipes without retaining full-table history copies.

## Architecture constraints

- Native Rust executable; no bundled Python runtime.
- Numeric columns are contiguous `f64` arrays.
- Curves reference columns rather than copying full data.
- Deleted rows use a compact alive/deleted bitmap.
- History is bounded by command count and retained bytes (64 MiB default).
- Rendering uses viewport-aware decimation while hit testing and export use the
  complete retained dataset.

## L1 decision gate

The technical prototype must build on Windows, macOS, and Linux, render Chinese
text correctly, and demonstrate plot zoom/pan. Target release artifact size is
at most 60 MiB and target empty-window peak RSS is at most 110 MiB on macOS.
The memory target was calibrated from the measured 97.55 MiB Cocoa/OpenGL
renderer baseline rather than guessed from library size. Missing a
target pauses feature migration until the renderer or font strategy is revised.
