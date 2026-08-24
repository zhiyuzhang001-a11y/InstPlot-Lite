"""Small, reusable Qt dialog boundaries that do not depend on the main window."""

from __future__ import annotations

import os
from typing import Any, Iterable

from PySide6.QtWidgets import (
    QApplication,
    QCheckBox,
    QDialog,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPushButton,
    QScrollArea,
    QTableWidget,
    QTableWidgetItem,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)


def show_error_details(parent: QWidget | None, title: str, message: str, details: str):
    dialog = QDialog(parent)
    dialog.setWindowTitle(title)
    dialog.resize(680, 420)
    layout = QVBoxLayout(dialog)
    message_label = QLabel(message)
    message_label.setWordWrap(True)
    layout.addWidget(message_label)
    details_edit = QTextEdit()
    details_edit.setReadOnly(True)
    details_edit.setPlainText(details)
    layout.addWidget(details_edit)
    buttons = QHBoxLayout()
    copy_button = QPushButton("复制错误详情")
    close_button = QPushButton("关闭")
    buttons.addStretch()
    buttons.addWidget(copy_button)
    buttons.addWidget(close_button)
    layout.addLayout(buttons)
    copy_button.clicked.connect(lambda: QApplication.clipboard().setText(details))
    close_button.clicked.connect(dialog.reject)
    return dialog.exec()


def create_file_selection_table(loaded_files, name_width: int = 450):
    table = QTableWidget()
    table.setRowCount(len(loaded_files))
    table.setColumnCount(2)
    table.setHorizontalHeaderLabels(["选择", "文件名"])
    table.setColumnWidth(0, 50)
    table.setColumnWidth(1, name_width)
    checkboxes = []
    for row, (path, _frame) in enumerate(loaded_files):
        checkbox = QCheckBox()
        checkbox.setChecked(True)
        checkboxes.append(checkbox)
        table.setCellWidget(row, 0, checkbox)
        table.setItem(row, 1, QTableWidgetItem(os.path.basename(path)))
    return table, checkboxes


def create_dialog_buttons():
    layout = QHBoxLayout()
    accept_button = QPushButton("确定")
    cancel_button = QPushButton("取消")
    layout.addWidget(accept_button)
    layout.addWidget(cancel_button)
    return layout, accept_button, cancel_button


def choose_export_columns(parent: QWidget, loaded_files: Iterable[tuple[Any, Any]]):
    columns_by_text = {}
    for _path, frame in loaded_files:
        for column in frame.columns:
            columns_by_text.setdefault(str(column), column)
    if not columns_by_text:
        QMessageBox.warning(parent, "提示", "没有找到任何列")
        return None

    dialog = QDialog(parent)
    dialog.setWindowTitle("选择要导出的列")
    dialog.resize(400, 400)
    layout = QVBoxLayout(dialog)
    layout.setSpacing(12)
    layout.addWidget(QLabel("选择要导出的列（勾选全选）："))

    select_all = QCheckBox("全选")
    layout.addWidget(select_all)
    scroll_area = QScrollArea()
    scroll_area.setWidgetResizable(True)
    container = QWidget()
    container_layout = QVBoxLayout(container)
    container_layout.setContentsMargins(0, 0, 0, 0)
    checks = {}
    for text in sorted(columns_by_text):
        check = QCheckBox(text)
        check.setChecked(True)
        checks[text] = check
        container_layout.addWidget(check)
    container_layout.addStretch()
    scroll_area.setWidget(container)
    layout.addWidget(scroll_area)
    def toggle_all(checked):
        for check in checks.values():
            check.setChecked(bool(checked))

    select_all.stateChanged.connect(toggle_all)
    select_all.setChecked(True)

    buttons = QHBoxLayout()
    accept_button = QPushButton("确定")
    cancel_button = QPushButton("取消")
    buttons.addStretch()
    buttons.addWidget(accept_button)
    buttons.addWidget(cancel_button)
    layout.addLayout(buttons)
    selected = None

    def accept_selection():
        nonlocal selected
        selected = [columns_by_text[text] for text, check in checks.items() if check.isChecked()]
        if not selected:
            QMessageBox.warning(dialog, "提示", "请至少选择一列")
            return
        dialog.accept()

    accept_button.clicked.connect(accept_selection)
    cancel_button.clicked.connect(dialog.reject)
    dialog.exec()
    return selected


def choose_existing_file_mode(parent: QWidget | None, path: os.PathLike[str] | str):
    message_box = QMessageBox(parent)
    message_box.setWindowTitle("文件已存在")
    message_box.setText(f"文件 {os.path.basename(path)} 已存在，请选择操作方式：")
    message_box.setStandardButtons(QMessageBox.Yes | QMessageBox.No | QMessageBox.Cancel)
    message_box.button(QMessageBox.Yes).setText("覆盖")
    message_box.button(QMessageBox.No).setText("追加")
    message_box.button(QMessageBox.Cancel).setText("取消")
    response = message_box.exec()
    if response == QMessageBox.Yes:
        return "overwrite"
    if response == QMessageBox.No:
        return "append"
    return None
