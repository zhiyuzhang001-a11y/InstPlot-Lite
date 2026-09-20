# InstPlot Lite developer guide

> [!NOTE]
> 普通用户请阅读仓库首页的[中文下载与使用说明](../README.md)，并从
> [Releases](https://github.com/zhiyuzhang001-a11y/InstPlot-Lite/releases) 下载安装包。
> 本页用于源码构建、自动验证和实现细节。

InstPlot Lite is a small native Rust edition of InstPlot. It focuses on importing
instrument text data, drawing basic curves, zooming and panning, deleting points,
undo/redo, lightweight numerical processing, and exporting cleaned data.

InstPlot Lite is the current end-user product. The historical Python source,
algorithms, plans, and tests now live in the separate
[InstPlot Python 原版](https://github.com/zhiyuzhang001-a11y/InstPlot)
repository.

## Development

```sh
cargo run
cargo test
cargo build --release
```

## One-command release

After committing and pushing the version bump and
`docs/RELEASE_NOTES_vX.Y.Z.md`, dispatch the complete release pipeline with:

```sh
scripts/release.sh
```

The command requires a clean working tree, validates that local `main` matches
`origin/main`, derives the tag from `Cargo.toml`, and hands the release to
GitHub Actions. It then exits; the computer does not need to remain awake.
GitHub independently builds and tests all supported packages, creates the
version tag and prerelease, publishes the signed Windows update to OSS, and
verifies the public download. Ordinary pushes run quality checks only, so
release packages are no longer built twice.

## Download

The [v0.3.4 unsigned preview release](https://github.com/zhiyuzhang001-a11y/InstPlot-Lite/releases/tag/v0.3.4)
provides native installers for Windows x64, Apple Silicon Macs, and Linux x64.
macOS Intel builds are no longer maintained as of v0.3.1. End users do not need
Rust, Cargo, Python, or a terminal.

- Windows 10/11: download and run the Setup EXE.
- macOS on Apple Silicon: download the arm64 DMG, open it, and drag
  **InstPlot Lite** to Applications.
- Debian/Ubuntu: download and open the DEB with the graphical package manager.
- Other compatible x64 Linux systems: use the portable tar archive.

Uninstallation is clean because InstPlot Lite does not create a user database,
saved settings, logs, or application caches. On Windows, uninstall it from
**Settings > Apps**; on macOS, move **InstPlot Lite.app** to the Trash; on
Debian/Ubuntu, use the graphical package manager's **Remove** action; for the
portable Linux archive, delete its extracted folder.

Every native package includes the project MIT license, a consolidated
third-party notice, and the complete SIL Open Font License texts for both
bundled font subsets. The macOS DMG exposes these in its `Licenses` folder and
keeps a copy inside the installed app; Windows and Linux install the same files
alongside the application documentation.

The preview packages are not yet signed with Apple and Microsoft developer
certificates, so macOS Gatekeeper or Windows SmartScreen may display a warning
on first launch. Checksums are published with the release notes.

## Current workflow

- On ordinary desktop widths, dataset and X/Y selectors live in a compact left
  sidebar so the plot remains balanced vertically. Narrow windows automatically
  switch back to horizontal controls.
- Click **打开文件**, or drag TXT, CSV, DAT, TSV, XLSX, or legacy XLS files
  into the window. Each numeric worksheet in a multi-sheet Excel workbook is
  loaded as an independent dataset; empty and notes-only worksheets are skipped.
- Choose the numeric X and Y columns; curves default to same-color lines with
  clearly visible point markers.
- Use the wheel to zoom and right-button drag to pan.
- A short left-click shows `(x, y)` at the bottom of the window. Near a data
  point it snaps to the point and requests deletion confirmation; left-drag
  selects a rectangle. Use the toolbar or Command/Ctrl+Z to undo.
- Click **数据处理** for the reference application's center,
  center-then-normalize, polynomial background removal, local flattening, and
  Savitzky–Golay denoising operations in a movable native tool window.
  Processing can overwrite the selected source column (the default) or retain
  a derived column, and can target the current curve or an explicit
  multi-selection. Local flattening removes the fitted linear slope inside a
  chosen X interval while preserving the selected anchor position.
- Click **曲线拟合** for polynomial (degree 1–10), exponential, logarithmic,
  power-law, or custom-expression fitting. The movable fit window supports the
  active curve or an explicit multi-selection whose curves are fitted
  independently, plus optional X/Y ranges and degree/radian conversion. It
  reports the equation, R², and retained point count, then draws each fitted
  curve directly on the matching main-plot coordinates. The fitting
  implementation is native Rust and does not require Python or SciPy.
- Click **导出图片** to save the complete plot as a white-background PNG with
  black tick values and labels, X/Y axis labels, and the legend, but excluding
  application controls.
- Open **导出数据…** to export the current dataset as CSV, XLSX, TSV, or
  tab-delimited TXT. All datasets can be exported either to one XLSX workbook
  with one worksheet per dataset, or to separate CSV/TSV/TXT files in a chosen
  folder. Batch text export generates unique names and never overwrites an
  existing file.

The text importer accepts UTF-8, UTF-16, and common GBK encodings. Rows whose
field count does not match the detected header are rejected with a line number
instead of silently shifting column names onto the wrong data. Excel import and
export are implemented in native Rust and do not require Python or Microsoft
Excel.

Large curves are reduced to screen-sized min/max buckets only for rendering,
so narrow peaks remain visible. The complete retained arrays are still used for
data export, point hit-testing, and rectangle deletion.

For automated development checks, a build can load a file and export a plot
without manual dialog interaction:

```sh
cargo run -- --open tests/fixtures/smoke.csv --export-plot /tmp/instplot-smoke.png
cargo run -- --check test_files/CoGd.txt
```

## Fonts

The interface uses two bundled font subsets, so users do not need to install
Arial or any other system font:

- `InstPlotSans-Latin.ttf` is a 117 KiB subset derived from Liberation Sans
  2.1.5. It is the primary proportional interface font and is metrically
  compatible with Arial. Because “Liberation” is a Reserved Font Name, the
  modified subset is named “InstPlot Sans”. Its SIL Open Font License 1.1 text
  is included at `assets/OFL-Liberation.txt`.
- `InstPlotSansSC-Level1.otf` is a size-optimized subset of Noto Sans SC and is
  used as the Chinese fallback. Its SIL Open Font License 1.1 text is included
  at `assets/OFL.txt`.

The Liberation Sans source version is 2.1.5 from the
[official Liberation Fonts project](https://github.com/liberationfonts/liberation-fonts/tree/2.1.5).
The bundled binary was obtained from Debian's
[`fonts-liberation2` package](https://packages.debian.org/bookworm/fonts-liberation2),
subset to the interface's Latin, punctuation, symbol, and mathematical ranges,
and renamed as required by the license. The original package SHA-256 is
`35ffaa54f117e633e89a5da89af235a073b34dbe582726fbf6989568f7fd9bda`;
the bundled subset SHA-256 is
`25e940e1e5275125303bf31c427feb94b98eb563ebe0451a3b2a65b31fe8da18`.
