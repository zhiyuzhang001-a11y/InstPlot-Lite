#!/usr/bin/env python3
"""Installed-runtime smoke test for InstPlot dependencies and core workflows."""

from __future__ import annotations

import argparse
import json
import logging
import os
from pathlib import Path
import tempfile

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
logging.getLogger("matplotlib.font_manager").setLevel(logging.ERROR)


def run_smoke():
    import InstPlot
    from PySide6 import QtCore, QtGui, QtSvg, QtWidgets  # noqa: F401
    from PySide6.QtWidgets import QApplication
    from instplot_io import ExportSource, prepare_export, read_data_file, write_export

    icon_directory = Path(InstPlot.__file__).resolve().parent / "symbol_icons"
    svg_count = len(list(icon_directory.rglob("*.svg")))
    if svg_count != 67:
        raise RuntimeError(f"expected 67 SVG resources, found {svg_count}")

    app = QApplication.instance() or QApplication([])
    with tempfile.TemporaryDirectory(prefix="instplot-install-smoke-") as temporary:
        root = Path(temporary)
        text_path = root / "input.txt"
        text_path.write_text("x\ty\n1\t2\n3\t4\n", encoding="utf-8")
        imported = read_data_file(text_path)
        if imported.frame.columns.tolist() != ["x", "y"] or len(imported.frame) != 2:
            raise RuntimeError("TXT round trip returned unexpected data")

        source = [ExportSource(text_path, imported.frame)]
        for suffix in (".csv", ".xlsx"):
            destination = root / f"export{suffix}"
            bundle = prepare_export(source, [], ["x", "y"], "x", "", suffix)
            write_export(destination, bundle, "overwrite")
            reread = read_data_file(destination)
            if len(reread.frame) != 2:
                raise RuntimeError(f"{suffix} round trip returned unexpected rows")

        window = InstPlot.PlotApp()
        try:
            window.loaded_files = [(str(text_path), imported.frame)]
            window.history.reset(window.loaded_files)
            window.combo_x.clear()
            window.combo_y.clear()
            window.combo_x.addItems(["x", "y"])
            window.combo_y.addItems(["x", "y"])
            window.combo_x.setCurrentText("x")
            window.combo_y.setCurrentText("y")
            window._draw_all_files("x", "y")
            png_path = root / "plot.png"
            window.figure.savefig(png_path)
            if not png_path.is_file() or png_path.stat().st_size == 0:
                raise RuntimeError("PNG export did not produce output")
        finally:
            window.close()
            app.processEvents()

    return {
        "status": "healthy",
        "module": str(Path(InstPlot.__file__).resolve()),
        "svg_count": svg_count,
        "txt_rows": 2,
        "exports": ["csv", "png", "xlsx"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        result = run_smoke()
    except Exception as error:
        if args.json:
            print(json.dumps({"status": "error", "reason": str(error)}, ensure_ascii=False))
        else:
            print(f"InstPlot install verification failed: {error}")
        return 1
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print("InstPlot install verification: healthy")
        print(f"module: {result['module']}")
        print(f"SVG resources: {result['svg_count']}")
        print("workflows: TXT/XLSX import, CSV/XLSX export, plot, PNG export")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
