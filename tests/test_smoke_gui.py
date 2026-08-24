from pathlib import Path

import InstPlot
from PySide6.QtGui import QIcon


def test_main_window_initializes_offscreen(qapp):
    window = InstPlot.PlotApp()
    try:
        assert window.windowTitle() == "InstPlot"
        assert window.canvas is not None
        assert len(window.loaded_files) == 7
    finally:
        window.close()


def test_symbol_selector_assets_are_available_at_runtime(qapp):
    icon_dir = Path(InstPlot.__file__).parent / "symbol_icons"
    icons = sorted(icon_dir.rglob("*.svg"))

    assert len(icons) == 67
    assert not QIcon(str(icons[0])).isNull()
