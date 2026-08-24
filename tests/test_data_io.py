from pathlib import Path

import pandas as pd
import pytest

import InstPlot


@pytest.fixture
def window(qapp):
    app_window = InstPlot.PlotApp()
    app_window.loaded_files.clear()
    app_window.combo_x.clear()
    app_window.combo_y.clear()
    yield app_window
    app_window.close()


def load_only_frame(window, path: Path):
    window.load_file(str(path))
    assert len(window.loaded_files) == 1
    return window.loaded_files[0][1]


@pytest.mark.parametrize(
    "content",
    [b"", b"\n \t\n", b"\xef\xbb\xbf", b"# metadata\n# run 2\n"],
    ids=["empty", "whitespace-only", "bom-only", "comments-only"],
)
def test_gui_empty_text_variants_show_structured_error(tmp_path, window, content):
    path = tmp_path / "empty.txt"
    path.write_bytes(content)

    window.load_file(str(path))

    assert window.loaded_files == []
    message = window.statusBar().currentMessage()
    assert str(path) in message
    assert "empty_file" in message
    assert "编码:" in message
    assert "分隔符: 未知" in message


def test_txt_header_after_blank_lines_keeps_header_and_values(tmp_path, window):
    path = tmp_path / "blank-lines.txt"
    path.write_text("\n\nField\tSignal\n1\t2\n3\t4\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["Field", "Signal"]
    assert frame.to_dict("list") == {"Field": [1, 3], "Signal": [2, 4]}


def test_comment_lines_before_header_are_ignored(tmp_path, window):
    path = tmp_path / "comments.txt"
    path.write_text("# instrument metadata\n# run 42\nx,y\n1,2\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["x", "y"]
    assert frame.to_dict("list") == {"x": [1], "y": [2]}


def test_headerless_numeric_txt_uses_generated_columns(tmp_path, window):
    path = tmp_path / "headerless.csv"
    path.write_text("1,2\n3,4\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["Column 1", "Column 2"]
    assert frame.to_dict("list") == {"Column 1": [1, 3], "Column 2": [2, 4]}


def test_single_numeric_row_without_header_is_preserved(tmp_path, window):
    path = tmp_path / "single-row.csv"
    path.write_text("1,2\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["Column 1", "Column 2"]
    assert frame.to_dict("list") == {"Column 1": [1], "Column 2": [2]}


def test_txt_trailing_delimiter_does_not_shift_columns(tmp_path, window):
    path = tmp_path / "trailing.txt"
    path.write_text("x,y\n1,2,\n3,4,\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["x", "y"]
    assert frame.to_dict("list") == {"x": [1, 3], "y": [2, 4]}


def test_gbk_header_is_read_without_garbled_column_names(tmp_path, window):
    path = tmp_path / "gbk.txt"
    path.write_bytes("磁场\t磁化强度\n1\t2\n".encode("gbk"))

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["磁场", "磁化强度"]
    assert frame.to_dict("list") == {"磁场": [1], "磁化强度": [2]}


def test_column_count_mismatch_is_not_imported(tmp_path, window):
    path = tmp_path / "mismatch.csv"
    path.write_text("x,y\n1,2,3\n", encoding="utf-8")

    window.load_file(str(path))

    assert window.loaded_files == []
    assert "第 2 行有 3 列" in window.statusBar().currentMessage()


def test_ambiguous_separator_is_reported_instead_of_silently_guessing(tmp_path, window):
    path = tmp_path / "ambiguous.txt"
    path.write_text("a,b;c\nd,e;f\ng,h;i\n", encoding="utf-8")

    window.load_file(str(path))

    assert window.loaded_files == []
    message = window.statusBar().currentMessage()
    assert "ambiguous_separator" in message
    assert "分隔符: 未知" in message


def test_whitespace_column_count_mismatch_is_not_imported(tmp_path, window):
    path = tmp_path / "mismatch.txt"
    path.write_text("x y\n1 2 3\n", encoding="utf-8")

    window.load_file(str(path))

    assert window.loaded_files == []
    assert "第 2 行有 3 列" in window.statusBar().currentMessage()


@pytest.mark.parametrize(
    "content",
    [
        "x y\n1 2\n3 4\n",
        "x   y\n1   2\n3   4\n",
        "x\ty\n1\t2\n3\t4\n",
        "x y\n1 \t 2\n3\t  4\n",
        "x y\n1\t2\n3\t4\n",
        "x\ty\n1 2\n3 4\n",
    ],
    ids=[
        "single-space",
        "multiple-spaces",
        "tabs",
        "mixed-within-data-row",
        "space-header-tab-data",
        "tab-header-space-data",
    ],
)
def test_whitespace_delimited_rows_preserve_field_boundaries(tmp_path, window, content):
    path = tmp_path / "valid-whitespace.txt"
    path.write_text(content, encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["x", "y"]
    assert frame.to_dict("list") == {"x": [1, 3], "y": [2, 4]}


def test_csv_export_round_trip_is_a_single_standard_table(tmp_path, window, monkeypatch):
    source = tmp_path / "source, run 1.txt"
    window.loaded_files = [
        (str(source), pd.DataFrame({"x": [1, 2], "y": [3, 4]}))
    ]
    window.combo_x.addItems(["x", "y"])
    window.combo_y.addItems(["x", "y"])
    window.combo_x.setCurrentText("x")
    window.combo_y.setCurrentText("y")
    output = tmp_path / "round-trip.csv"

    monkeypatch.setattr(window, "_show_column_selection_dialog", lambda: ["x", "y"])
    monkeypatch.setattr(
        InstPlot.QFileDialog,
        "getSaveFileName",
        lambda *args, **kwargs: (str(output), "CSV Files (*.csv)"),
    )

    window.export_data()

    exported = pd.read_csv(output)
    assert exported.to_dict("list") == {
        "source_file": ["source, run 1.txt", "source, run 1.txt"],
        "data_type": ["raw_data", "raw_data"],
        "x": [1, 2],
        "y": [3, 4],
    }

    reloaded_window = InstPlot.PlotApp()
    reloaded_window.loaded_files.clear()
    reloaded_window.combo_x.clear()
    reloaded_window.combo_y.clear()
    try:
        reloaded = load_only_frame(reloaded_window, output)
        assert reloaded.columns.tolist() == ["source_file", "data_type", "x", "y"]
        assert reloaded["source_file"].tolist() == ["source, run 1.txt", "source, run 1.txt"]
        assert reloaded["x"].tolist() == [1, 2]
        assert reloaded["y"].tolist() == [3, 4]
    finally:
        reloaded_window.close()


def test_plain_dat_uses_the_text_import_path(tmp_path, window):
    path = tmp_path / "ordinary.dat"
    path.write_text("field,signal\n1,2\n3,4\n", encoding="utf-8")

    frame = load_only_frame(window, path)

    assert frame.to_dict("list") == {"field": [1, 3], "signal": [2, 4]}


def test_vsm_dat_reads_the_expected_columns_after_metadata(tmp_path, window):
    path = tmp_path / "measurement.dat"
    metadata = ["VSM export"] + [f"metadata {index}" for index in range(30)]
    path.write_text(
        "\n".join(metadata + ["0,1,2,100,-0.5", "0,1,2,200,0.75"]) + "\n",
        encoding="ascii",
    )

    frame = load_only_frame(window, path)

    assert frame.columns.tolist() == ["B (Oe)", "M (emu)"]
    assert frame.to_dict("list") == {"B (Oe)": [100, 200], "M (emu)": [-0.5, 0.75]}


def test_xlsx_import_keeps_unicode_columns_and_values(tmp_path, window):
    path = tmp_path / "measurement.xlsx"
    pd.DataFrame({"磁场": [1, 2], "信号": [3.5, 4.5]}).to_excel(path, index=False)

    frame = load_only_frame(window, path)

    assert frame.to_dict("list") == {"磁场": [1, 2], "信号": [3.5, 4.5]}


def test_offscreen_gui_column_selectors_match_loaded_frame_order(tmp_path, window):
    path = tmp_path / "GUI 列映射.txt"
    path.write_text(
        '磁场 Oe;信号;备注\n-10;1.25;"左,支"\n0;2.5;中心\n10;3.75;"右;支"\n',
        encoding="utf-8-sig",
    )

    frame = load_only_frame(window, path)
    expected_columns = ["磁场 Oe", "信号", "备注"]

    assert frame.columns.tolist() == expected_columns
    assert frame.to_dict("list") == {
        "磁场 Oe": [-10, 0, 10],
        "信号": [1.25, 2.5, 3.75],
        "备注": ["左,支", "中心", "右;支"],
    }
    assert [window.combo_x.itemText(index) for index in range(window.combo_x.count())] == expected_columns
    assert [window.combo_y.itemText(index) for index in range(window.combo_y.count())] == expected_columns


def test_xls_fixture_uses_the_real_xlrd_import_path(window):
    path = Path(__file__).parent / "fixtures" / "data_io" / "legacy-sample.xls"

    frame = load_only_frame(window, path)

    assert frame.to_dict("list") == {"磁场": [1, 2], "信号": [3.5, 4.5]}


def test_column_mismatch_reports_the_original_physical_line_number(tmp_path, window):
    path = tmp_path / "physical-line-number.txt"
    path.write_text("\n# metadata\nx,y\n1,2,3\n", encoding="utf-8")

    window.load_file(str(path))

    assert window.loaded_files == []
    assert "第 4 行有 3 列" in window.statusBar().currentMessage()


def test_import_failure_status_exposes_required_context(tmp_path, window):
    path = tmp_path / "context-required.txt"
    path.write_text("\n# metadata\nx,y\n1,2,3\n", encoding="utf-8")

    window.load_file(str(path))

    message = window.statusBar().currentMessage()
    assert path.name in message
    assert "编码:" in message
    assert "分隔符:" in message
    assert "第 4 行" in message


def _set_export_window_data(window, source: Path, frame: pd.DataFrame, x_unit_mode="degree"):
    window.loaded_files = [(str(source), frame)]
    window.combo_x.clear()
    window.combo_y.clear()
    window.combo_x.addItems(frame.columns)
    window.combo_y.addItems(frame.columns)
    window.combo_x.setCurrentText(frame.columns[0])
    window.combo_y.setCurrentText(frame.columns[-1])
    window.x_unit_mode = x_unit_mode


def _choose_append(monkeypatch):
    monkeypatch.setattr(InstPlot.QMessageBox, "exec", lambda *_args, **_kwargs: InstPlot.QMessageBox.No)


def test_csv_append_preserves_existing_column_order(tmp_path, window, monkeypatch):
    output = tmp_path / "append.csv"
    output.write_text("source_file,data_type,x,y\nold.txt,raw_data,0,0\n", encoding="utf-8-sig")
    source = tmp_path / "new.txt"
    _set_export_window_data(window, source, pd.DataFrame({"x": [1], "y": [2]}))
    monkeypatch.setattr(window, "_show_column_selection_dialog", lambda: ["x", "y"])
    monkeypatch.setattr(
        InstPlot.QFileDialog,
        "getSaveFileName",
        lambda *args, **kwargs: (str(output), "CSV Files (*.csv)"),
    )
    _choose_append(monkeypatch)

    window.export_data()

    assert pd.read_csv(output).to_dict("list") == {
        "source_file": ["old.txt", "new.txt"],
        "data_type": ["raw_data", "raw_data"],
        "x": [0, 1],
        "y": [0, 2],
    }


def test_csv_append_schema_mismatch_leaves_destination_unchanged(tmp_path, window, monkeypatch):
    output = tmp_path / "append-mismatch.csv"
    original = "source_file,data_type,x\nold.txt,raw_data,0\n"
    output.write_text(original, encoding="utf-8-sig")
    source = tmp_path / "new.txt"
    _set_export_window_data(window, source, pd.DataFrame({"x": [1], "y": [2]}))
    monkeypatch.setattr(window, "_show_column_selection_dialog", lambda: ["x", "y"])
    monkeypatch.setattr(
        InstPlot.QFileDialog,
        "getSaveFileName",
        lambda *args, **kwargs: (str(output), "CSV Files (*.csv)"),
    )
    _choose_append(monkeypatch)

    window.export_data()

    assert output.read_text(encoding="utf-8-sig") == original
    assert "列结构不同" in window.statusBar().currentMessage()


def test_export_radian_conversion_does_not_mutate_source_frame(tmp_path, window, monkeypatch):
    source = tmp_path / "angles.txt"
    frame = pd.DataFrame({"angle": [0.0, 180.0], "signal": [1.0, 2.0]})
    original = frame.copy(deep=True)
    output = tmp_path / "angles.csv"
    _set_export_window_data(window, source, frame, x_unit_mode="radian")
    monkeypatch.setattr(window, "_show_column_selection_dialog", lambda: ["angle", "signal"])
    monkeypatch.setattr(
        InstPlot.QFileDialog,
        "getSaveFileName",
        lambda *args, **kwargs: (str(output), "CSV Files (*.csv)"),
    )

    window.export_data()

    pd.testing.assert_frame_equal(frame, original)
    assert pd.read_csv(output)["angle"].tolist() == pytest.approx([0.0, pytest.approx(3.141592653589793)])
