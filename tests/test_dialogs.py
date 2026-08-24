import inspect

import instplot_dialogs
import InstPlot
import pandas as pd
from PySide6.QtWidgets import QDialog, QPushButton, QTextEdit


def test_dialog_module_does_not_import_main_application():
    assert "import InstPlot" not in inspect.getsource(instplot_dialogs)


def test_plotapp_export_dialog_adapter_stays_thin():
    source = inspect.getsource(InstPlot.PlotApp._show_column_selection_dialog)
    assert "choose_export_columns" in source
    assert len(source.splitlines()) <= 6


def test_existing_file_mode_maps_standard_responses(qapp, monkeypatch, tmp_path):
    path = tmp_path / "exists.csv"
    path.write_text("x\n1\n", encoding="utf-8")

    monkeypatch.setattr(instplot_dialogs.QMessageBox, "exec", lambda *_args: instplot_dialogs.QMessageBox.Yes)
    assert instplot_dialogs.choose_existing_file_mode(None, path) == "overwrite"
    monkeypatch.setattr(instplot_dialogs.QMessageBox, "exec", lambda *_args: instplot_dialogs.QMessageBox.No)
    assert instplot_dialogs.choose_existing_file_mode(None, path) == "append"
    monkeypatch.setattr(instplot_dialogs.QMessageBox, "exec", lambda *_args: instplot_dialogs.QMessageBox.Cancel)
    assert instplot_dialogs.choose_existing_file_mode(None, path) is None


def test_file_selection_table_preserves_order_names_and_default_selection(qapp):
    loaded = [
        ("/first/same.txt", pd.DataFrame({"x": [1]})),
        ("/second/other.txt", pd.DataFrame({"x": [2]})),
    ]
    table, checkboxes = instplot_dialogs.create_file_selection_table(loaded, name_width=420)

    assert table.rowCount() == 2
    assert table.item(0, 1).text() == "same.txt"
    assert table.item(1, 1).text() == "other.txt"
    assert all(checkbox.isChecked() for checkbox in checkboxes)
    assert table.columnWidth(1) == 420


def test_error_details_dialog_has_explicit_copy_action(qapp, monkeypatch):
    observed = {}

    def execute(dialog):
        details = dialog.findChild(QTextEdit)
        copy_button = next(
            button for button in dialog.findChildren(QPushButton) if button.text() == "复制错误详情"
        )
        observed["read_only"] = details.isReadOnly()
        copy_button.click()
        observed["clipboard"] = qapp.clipboard().text()
        return QDialog.Rejected

    monkeypatch.setattr(instplot_dialogs.QDialog, "exec", execute)
    instplot_dialogs.show_error_details(None, "失败", "操作失败", "Traceback details")

    assert observed == {"read_only": True, "clipboard": "Traceback details"}
