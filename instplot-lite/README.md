# InstPlot Lite

InstPlot Lite is a small native Rust edition of InstPlot. It focuses on importing
instrument text data, drawing basic curves, zooming and panning, deleting points,
undo/redo, lightweight numerical processing, and exporting cleaned data.

The Python application remains the stable reference while this edition is built
and validated independently.

## Development

```sh
cargo run
cargo test
cargo build --release
```

## Download

The [v0.1.0 unsigned preview release](https://github.com/zhiyuzhang001-a11y/InstPlot/releases/tag/v0.1.0)
provides native installers for Windows x64, Apple Silicon and Intel Macs, and
Linux x64. End users do not need Rust, Cargo, Python, or a terminal.

- Windows 10/11: download and run the Setup EXE.
- macOS: download the DMG matching the Mac processor, open it, and drag
  **InstPlot Lite** to Applications.
- Debian/Ubuntu: download and open the DEB with the graphical package manager.
- Other compatible x64 Linux systems: use the portable tar archive.

The preview packages are not yet signed with Apple and Microsoft developer
certificates, so macOS Gatekeeper or Windows SmartScreen may display a warning
on first launch. Checksums are published with the release notes.

## Current workflow

- On ordinary desktop widths, dataset and X/Y selectors live in a compact left
  sidebar so the plot remains balanced vertically. Narrow windows automatically
  switch back to horizontal controls.
- Click **打开文件**, or drag TXT, CSV, or DAT files into the window.
- Choose the numeric X and Y columns; curves default to same-color lines with
  clearly visible point markers.
- Use the wheel to zoom and right-button drag to pan.
- A short left-click shows `(x, y)` at the bottom of the window. Near a data
  point it snaps to the point and requests deletion confirmation; left-drag
  selects a rectangle. Use the toolbar or Command/Ctrl+Z to undo.
- Click **数据处理** for the reference application's center,
  center-then-normalize, polynomial background removal, local flattening, and
  Savitzky–Golay denoising operations in a movable native tool window.
  Processing creates a selected derived column and never overwrites the
  imported column. Local flattening removes the fitted linear slope inside a
  chosen X interval while preserving the selected anchor position.
- Click **导出图片** to save the complete plot as a PNG, including tick values,
  X/Y axis labels, and the legend, but excluding application controls.
- Select a dataset and click **导出数据** to save retained numeric rows as CSV
  or tab-delimited TXT.

The importer accepts UTF-8, UTF-16, and common GBK text. Rows whose field count
does not match the detected header are rejected with a line number instead of
silently shifting column names onto the wrong data.

Large curves are reduced to screen-sized min/max buckets only for rendering,
so narrow peaks remain visible. The complete retained arrays are still used for
data export, point hit-testing, and rectangle deletion.

For automated development checks, a build can load a file and export a plot
without manual dialog interaction:

```sh
cargo run -- --open tests/fixtures/smoke.csv --export-plot /tmp/instplot-smoke.png
cargo run -- --check ../test_files/CoGd.txt
```

## Fonts

The bundled `InstPlotSansSC-Level1.otf` is a size-optimized subset of Noto Sans
SC. Noto fonts are distributed under the SIL Open Font License 1.1; the license
text is included at `assets/OFL.txt`. The subset contains common Simplified
Chinese characters, Latin, Greek, mathematical symbols, and common punctuation.
