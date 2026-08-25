# InstPlot Lite v0.2.0 — unsigned preview

InstPlot Lite is a lightweight native Rust application. End users do not need
Python, Rust, Microsoft Excel, or a terminal.

## Highlights

- Added native polynomial, exponential, logarithmic, power-law, and safe custom
  expression fitting without SciPy.
- Added TXT, CSV, DAT, TSV, XLSX, and legacy XLS import. Numeric worksheets in
  multi-sheet workbooks become independent datasets; empty and notes-only
  worksheets are skipped.
- Added current-dataset export as CSV, XLSX, TSV, or TXT.
- Added all-dataset export to one multi-sheet XLSX or separate collision-safe
  CSV/TSV/TXT files. Existing files are never silently overwritten.
- Added an embedded Arial-compatible English interface font while preserving
  the embedded Simplified Chinese fallback. No system font installation is
  required.
- Improved fitting-field visibility, toolbar hierarchy, status guidance, plot
  bottom spacing, coordinate display, and movable processing/fitting windows.
- Preserved point deletion, rectangle deletion, zoom, pan, undo/redo, data
  processing, PNG export, and strict column-alignment validation.

## Validation

- 44 native tests pass locally.
- A 1.9 MiB workbook containing 100,000 rows, four columns, and two worksheets
  parsed in 0.07 seconds with a 38,567,936-byte peak RSS in check mode on the
  development Apple Silicon Mac.
- The optimized macOS arm64 executable is 6,813,392 bytes before packaging.

## Signing notice

This is an unsigned preview. Windows SmartScreen and macOS Gatekeeper may show
a warning on first launch. The release artifacts are built and installed or
mounted in native GitHub Actions jobs, and SHA-256 checksums are provided.
