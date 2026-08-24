import csv
from io import StringIO
from pathlib import Path
from random import Random

import pandas as pd
import pytest

import instplot_io
from instplot_io import (
    DataIOError,
    ExportSource,
    FittedCurve,
    prepare_export,
    read_data_file,
    write_export,
)


@pytest.mark.parametrize(
    "content",
    [b"", b"\n \t\r\n", b"\xef\xbb\xbf", b"# metadata\n# run 2\n"],
    ids=["empty", "whitespace-only", "bom-only", "comments-only"],
)
def test_core_normalizes_empty_text_variants(content, tmp_path):
    path = tmp_path / "empty.txt"
    path.write_bytes(content)

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    error = raised.value
    assert error.path == path
    assert error.operation == "import"
    assert error.code == "empty_file"
    assert error.encoding == "utf-8-sig"
    assert error.separator is None
    assert error.line_number is None


def test_core_reads_semicolon_bom_from_unicode_space_path(tmp_path):
    path = tmp_path / "含 空格" / "测量 数据.csv"
    path.parent.mkdir()
    path.write_text("\ufeff磁场;信号\n1;2\n", encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == ";"
    assert result.frame.to_dict("list") == {"磁场": [1], "信号": [2]}


@pytest.mark.parametrize(
    ("separator", "content"),
    [
        (",", "# instrument\n# operator\nname,label\nalice,left\n"),
        (";", "# instrument\n# operator\nname;label\nalice;left\n"),
        (",", "name,label\nalice,left\n"),
        (";", "name;label\nalice;left\n"),
    ],
    ids=["commented-comma", "commented-semicolon", "plain-comma", "plain-semicolon"],
)
def test_core_reads_all_string_delimited_rows(separator, content, tmp_path):
    path = tmp_path / "all strings.csv"
    path.write_text(content, encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == separator
    assert result.frame.to_dict("list") == {"name": ["alice"], "label": ["left"]}


@pytest.mark.parametrize(
    ("separator", "content", "expected_note"),
    [
        (",", 'name,note\nalice,"left,right"\n', "left,right"),
        (";", 'name;note\nalice;"left;right"\n', "left;right"),
        (",", 'name,note\nalice,"said ""hello, world"""\n', 'said "hello, world"'),
    ],
    ids=["quoted-comma", "quoted-semicolon", "escaped-double-quote"],
)
def test_core_uses_csv_field_boundaries_for_quoted_strings(
    separator, content, expected_note, tmp_path
):
    path = tmp_path / "quoted fields.csv"
    path.write_text(content, encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == separator
    assert result.frame.to_dict("list") == {"name": ["alice"], "note": [expected_note]}


@pytest.mark.parametrize(
    ("separator", "content", "expected_column", "expected_value"),
    [
        (";", 'name;"note, label"\nalice;"left,right"\n', "note, label", "left,right"),
        ("\t", 'name\t"note, label"\nalice\t"left,right"\n', "note, label", "left,right"),
        (",", 'name,"note; label"\nalice,"left;right"\n', "note; label", "left;right"),
        (",", 'name,"note\tlabel"\nalice,"left\tright"\n', "note label", "left\tright"),
        (
            ";",
            '# metadata\nname;"note, label"\nalice;"said ""hello, world"""\n',
            "note, label",
            'said "hello, world"',
        ),
    ],
    ids=[
        "semicolon-with-quoted-comma",
        "tab-with-quoted-comma",
        "comma-with-quoted-semicolon",
        "comma-with-quoted-tab",
        "commented-semicolon-with-escaped-quoted-comma",
    ],
)
def test_core_ignores_other_delimiter_candidates_inside_quotes(
    separator, content, expected_column, expected_value, tmp_path
):
    path = tmp_path / "cross delimiter collision.csv"
    path.write_text(content, encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == separator
    assert result.frame.columns.tolist() == ["name", expected_column]
    assert result.frame.iloc[0].tolist() == ["alice", expected_value]


def test_core_literal_quote_in_unquoted_header_does_not_hide_separator(tmp_path):
    path = tmp_path / "literal quote.csv"
    path.write_text('height 5";label\nshort;left\n', encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == ";"
    assert result.frame.to_dict("list") == {'height 5"': ["short"], "label": ["left"]}


@pytest.mark.parametrize(
    ("actual_separator", "competing_separator"),
    [
        (",", ";"),
        (",", "\t"),
        (";", ","),
        (";", "\t"),
        ("\t", ","),
        ("\t", ";"),
    ],
    ids=[
        "comma-vs-semicolon",
        "comma-vs-tab",
        "semicolon-vs-comma",
        "semicolon-vs-tab",
        "tab-vs-comma",
        "tab-vs-semicolon",
    ],
)
def test_core_scores_complete_dialect_when_unquoted_fields_contain_competitors(
    actual_separator, competing_separator, tmp_path
):
    header = [f"key{competing_separator} literal", f'note {competing_separator} "quoted"']
    rows = [
        [f"alice{competing_separator} literal", f'value {competing_separator} "one"'],
        [f"bob{competing_separator} literal", f'value {competing_separator} "two"'],
        [f"carol{competing_separator} literal", f'value {competing_separator} "three"'],
    ]
    output = StringIO()
    csv.writer(output, delimiter=actual_separator, lineterminator="\n").writerows([header, *rows])
    path = tmp_path / "competing dialects.txt"
    path.write_text("# metadata\n" + output.getvalue(), encoding="utf-8")

    result = read_data_file(path)

    assert result.separator == actual_separator
    assert result.frame.columns.tolist() == [
        " ".join(f"key{competing_separator} literal".split()),
        " ".join(f'note {competing_separator} "quoted"'.split()),
    ]
    assert result.frame.astype(str).values.tolist() == rows


@pytest.mark.parametrize(
    ("encoding", "separator", "header", "rows"),
    [
        (
            "utf-8-sig",
            ",",
            ["磁场 Oe", "信号,原始值", "备注"],
            [[-10, 1.25, "左,支"], [0, 2.5, '含 "引号"'], [10, 3.75, "右支"]],
        ),
        (
            "gb18030",
            ";",
            ["温度 K", "电阻;原始值", "备注"],
            [[2, 10.1, "低温;段"], [20, 11.2, '含 "引号"'], [300, 12.3, "室温"]],
        ),
        (
            "big5",
            "\t",
            ["磁場 Oe", "訊號\t原始值", "備註"],
            [[-5, 0.1, "左\t支"], [0, 0.2, '含 "引號"'], [5, 0.3, "右支"]],
        ),
    ],
    ids=["utf8-bom-comma", "gb18030-semicolon", "big5-tab"],
)
def test_core_golden_encoded_dialects_preserve_columns_and_cells(
    encoding, separator, header, rows, tmp_path
):
    output = StringIO()
    csv.writer(output, delimiter=separator, lineterminator="\n").writerows([header, *rows])
    path = tmp_path / f"黄金 样本 {encoding}.txt"
    path.write_bytes(("# instrument metadata\n" + output.getvalue()).encode(encoding))

    result = read_data_file(path)

    assert result.separator == separator
    assert result.frame.columns.tolist() == [" ".join(value.split()) for value in header]
    assert result.frame.values.tolist() == rows


def test_core_seeded_thousand_dialects_match_independent_csv_matrices(tmp_path):
    rng = Random(20260823)
    separators = [",", ";", "\t"]
    root = tmp_path / "seeded-dialects"
    root.mkdir()

    for case_index in range(1000):
        actual = separators[case_index % len(separators)]
        competitors = [candidate for candidate in separators if candidate != actual]
        competing = competitors[rng.randrange(len(competitors))]
        token = rng.randrange(1_000_000)
        header = [
            f"key{competing} literal {case_index}",
            f'note {competing} "quoted"',
            "measurement",
        ]
        rows = [
            [
                f"sample-{row_index}{competing}literal-{token}",
                f'value {competing} "{rng.randrange(1_000_000)}"',
                f"reading-{rng.randrange(1_000_000)}",
            ]
            for row_index in range(3 + rng.randrange(4))
        ]
        output = StringIO()
        csv.writer(output, delimiter=actual, lineterminator="\n").writerows([header, *rows])
        path = root / f"case-{case_index:04d}.txt"
        prefix = "# instrument\n# run metadata\n" if case_index % 2 else ""
        path.write_text(prefix + output.getvalue(), encoding="utf-8")

        result = read_data_file(path)

        expected_columns = [" ".join(value.split()) for value in header]
        assert result.separator == actual, f"case {case_index}: wrong separator"
        assert result.frame.columns.tolist() == expected_columns, f"case {case_index}: wrong columns"
        assert result.frame.astype(str).values.tolist() == rows, f"case {case_index}: wrong cells"


def test_core_rejects_genuinely_ambiguous_delimiter_instead_of_guessing(tmp_path):
    path = tmp_path / "ambiguous.txt"
    path.write_text("a,b;c\nd,e;f\ng,h;i\n", encoding="utf-8")

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    error = raised.value
    assert error.code == "ambiguous_separator"
    assert error.encoding == "utf-8-sig"
    assert error.separator is None
    assert error.line_number is None


def test_core_reports_unclosed_quote_at_original_physical_line(tmp_path):
    path = tmp_path / "unclosed quote.csv"
    path.write_text('# metadata\nname,note\nalice,"left\n', encoding="utf-8")

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    error = raised.value
    assert error.code == "text_parse_failed"
    assert error.encoding == "utf-8-sig"
    assert error.separator == ","
    assert error.line_number == 3


def test_core_counts_quoted_delimiter_as_one_field_in_width_error(tmp_path):
    path = tmp_path / "quoted mismatch.csv"
    path.write_text('name,note\nalice,"left,right",extra\n', encoding="utf-8")

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    error = raised.value
    assert error.code == "column_count_mismatch"
    assert error.separator == ","
    assert error.line_number == 2
    assert "有 3 列" in error.reason


def test_core_reads_plain_dat_without_gui(tmp_path):
    path = tmp_path / "ordinary.dat"
    path.write_text("field,signal\n1,2\n3,4\n", encoding="utf-8")

    result = read_data_file(path)

    assert result.format == "text"
    assert result.encoding == "utf-8-sig"
    assert result.separator == ","
    assert result.frame.to_dict("list") == {"field": [1, 3], "signal": [2, 4]}


def test_core_reads_vsm_dat_with_fixed_metadata_offset(tmp_path):
    path = tmp_path / "measurement.dat"
    metadata = ["vSm export"] + [f"metadata {index}" for index in range(30)]
    path.write_text(
        "\n".join(metadata + ["0,1,2,100,-0.5", "0,1,2,200,0.75"]) + "\n",
        encoding="ascii",
    )

    result = read_data_file(path)

    assert result.format == "vsm"
    assert result.encoding == "VSM"
    assert result.separator == ","
    assert result.frame.to_dict("list") == {"B (Oe)": [100, 200], "M (emu)": [-0.5, 0.75]}


@pytest.mark.parametrize(
    "lines",
    [
        ["VSM export", "1,2"],
        ["VSM export", *[f"metadata {index}" for index in range(30)], "0,1,2,100"],
    ],
    ids=["fewer-than-31-lines", "fewer-than-5-columns"],
)
def test_core_normalizes_malformed_vsm_files(lines, tmp_path):
    path = tmp_path / "malformed.dat"
    path.write_text("\n".join(lines) + "\n", encoding="ascii")

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    assert raised.value.path == path
    assert raised.value.code == "vsm_parse_failed"
    assert raised.value.encoding == "VSM"
    assert raised.value.separator == ","


@pytest.mark.parametrize("fixture_name", ["legacy-sample.xls"])
def test_core_reads_excel_fixtures(fixture_name):
    path = Path(__file__).parent / "fixtures" / "data_io" / fixture_name

    result = read_data_file(path)

    assert result.format == "xls"
    assert result.encoding == "Excel"
    assert result.separator is None
    assert result.frame.to_dict("list") == {"磁场": [1, 2], "信号": [3.5, 4.5]}


def test_core_error_keeps_file_encoding_separator_and_physical_line(tmp_path):
    path = tmp_path / "physical-line-number.txt"
    path.write_text("\n# metadata\nx,y\n1,2,3\n", encoding="utf-8")

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    error = raised.value
    assert error.path == path
    assert error.operation == "import"
    assert error.code == "column_count_mismatch"
    assert error.encoding == "utf-8-sig"
    assert error.separator == ","
    assert error.line_number == 4
    assert path.name in str(error)
    assert "编码:" in str(error)
    assert "分隔符:" in str(error)


def test_core_rejects_unsupported_extension_with_structured_error(tmp_path):
    path = tmp_path / "measurement.bin"
    path.write_bytes(b"not a supported data file")

    with pytest.raises(DataIOError, match="unsupported_extension"):
        read_data_file(path)


def test_core_normalizes_missing_file(tmp_path):
    path = tmp_path / "missing.csv"

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    assert raised.value.path == path
    assert raised.value.code == "file_not_found"


def test_core_normalizes_decode_failure(tmp_path, monkeypatch):
    path = tmp_path / "undecodable.txt"

    class UndecodableBytes(bytes):
        def decode(self, encoding="utf-8", errors="strict"):
            raise UnicodeDecodeError(encoding, bytes(self), 0, 1, "forced decode failure")

    monkeypatch.setattr(Path, "read_bytes", lambda _path: UndecodableBytes(b"x"))

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    assert raised.value.path == path
    assert raised.value.code == "decode_failed"


def test_core_normalizes_excel_engine_failure(tmp_path, monkeypatch):
    path = tmp_path / "broken.xlsx"
    path.write_bytes(b"not an excel workbook")
    monkeypatch.setattr(
        instplot_io.pd,
        "read_excel",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(ImportError("missing engine")),
    )

    with pytest.raises(DataIOError) as raised:
        read_data_file(path)

    assert raised.value.path == path
    assert raised.value.code == "excel_parse_failed"
    assert raised.value.encoding == "Excel"


def test_core_reads_text_bytes_once(tmp_path, monkeypatch):
    path = tmp_path / "single-read.csv"
    path.write_text("x,y\n1,2\n", encoding="utf-8")
    original_read_bytes = Path.read_bytes
    read_paths: list[Path] = []

    def tracked_read_bytes(self):
        read_paths.append(self)
        return original_read_bytes(self)

    monkeypatch.setattr(Path, "read_bytes", tracked_read_bytes)

    result = read_data_file(path)

    assert result.frame.to_dict("list") == {"x": [1], "y": [2]}
    assert read_paths == [path]


def test_core_does_not_mutate_imported_frame_during_column_cleanup(tmp_path):
    path = tmp_path / "columns.csv"
    expected = pd.DataFrame({"x value": [1], "y value": [2]})
    path.write_text(" x   value , y value \n1,2\n", encoding="utf-8")

    result = read_data_file(path)

    pd.testing.assert_frame_equal(result.frame, expected)


def test_prepare_export_converts_radians_without_mutating_sources():
    frame = pd.DataFrame({"angle": [0.0, 180.0], "signal": [1.0, 2.0]})
    original = frame.copy(deep=True)

    bundle = prepare_export(
        [ExportSource("angles.txt", frame)],
        [],
        ["angle", "signal"],
        "angle",
        "radian",
        "csv",
    )

    pd.testing.assert_frame_equal(frame, original)
    assert bundle.table is not None
    assert bundle.table["angle"].tolist() == pytest.approx([0.0, 3.141592653589793])
    assert bundle.table["source_file"].tolist() == ["angles.txt", "angles.txt"]


def test_prepare_export_handles_multiple_sources_selection_and_fitted_curve():
    first = pd.DataFrame({"x": [1], "y": [2], "ignored": [9]})
    second = pd.DataFrame({"x": [3], "y": [4], "ignored": [8]})
    originals = [first.copy(deep=True), second.copy(deep=True)]
    fit_x = [5]
    fit_y = [6]

    bundle = prepare_export(
        [ExportSource("same.txt", first), ExportSource("folder/same.txt", second)],
        [FittedCurve("fit", fit_x, fit_y)],
        ["x", "y"],
        "x",
        "degree",
        "csv",
    )

    pd.testing.assert_frame_equal(first, originals[0])
    pd.testing.assert_frame_equal(second, originals[1])
    assert fit_x == [5]
    assert fit_y == [6]
    assert bundle.table is not None
    assert bundle.table.columns.tolist() == [
        "source_file", "data_type", "x", "y", "x_fit", "y_fit"
    ]
    assert bundle.table["source_file"].tolist() == ["same.txt", "same.txt", "fit"]
    assert bundle.table["data_type"].tolist() == ["raw_data", "raw_data", "fitted_data"]


def test_write_export_appends_in_existing_column_order(tmp_path):
    path = tmp_path / "append.csv"
    path.write_text("source_file,data_type,x,y\nold.txt,raw_data,0,0\n", encoding="utf-8-sig")
    bundle = prepare_export(
        [ExportSource("new.txt", pd.DataFrame({"x": [1], "y": [2]}))],
        [],
        ["x", "y"],
        "x",
        "degree",
        "csv",
    )

    result = write_export(path, bundle, "append")

    assert result.row_count == 1
    assert pd.read_csv(path).to_dict("list") == {
        "source_file": ["old.txt", "new.txt"],
        "data_type": ["raw_data", "raw_data"],
        "x": [0, 1],
        "y": [0, 2],
    }


def test_write_export_schema_mismatch_preserves_existing_bytes(tmp_path):
    path = tmp_path / "append-mismatch.csv"
    original = "source_file,data_type,x\nold.txt,raw_data,0\n"
    path.write_text(original, encoding="utf-8-sig")
    bundle = prepare_export(
        [ExportSource("new.txt", pd.DataFrame({"x": [1], "y": [2]}))],
        [],
        ["x", "y"],
        "x",
        "degree",
        "csv",
    )

    with pytest.raises(DataIOError, match="append_schema_mismatch"):
        write_export(path, bundle, "append")

    assert path.read_text(encoding="utf-8-sig") == original


def test_write_txt_round_trip(tmp_path):
    path = tmp_path / "round trip.txt"
    bundle = prepare_export(
        [ExportSource("源 数据.csv", pd.DataFrame({"x": [1, 2], "y": [3, 4]}))],
        [],
        ["x", "y"],
        "x",
        "degree",
        "txt",
    )

    write_export(path, bundle, "overwrite")
    result = read_data_file(path)

    assert result.separator == "\t"
    assert result.frame.to_dict("list") == {
        "source_file": ["源 数据.csv", "源 数据.csv"],
        "data_type": ["raw_data", "raw_data"],
        "x": [1, 2],
        "y": [3, 4],
    }


def test_write_txt_appends_same_schema_in_existing_order(tmp_path):
    path = tmp_path / "append.txt"
    path.write_text("y\tdata_type\tsource_file\tx\n0\traw_data\told.txt\t0\n", encoding="utf-8-sig")
    bundle = prepare_export(
        [ExportSource("new.txt", pd.DataFrame({"x": [1], "y": [2]}))],
        [],
        ["x", "y"],
        "x",
        "degree",
        "txt",
    )

    write_export(path, bundle, "append")

    assert pd.read_csv(path, sep="\t").to_dict("list") == {
        "y": [0, 2],
        "data_type": ["raw_data", "raw_data"],
        "source_file": ["old.txt", "new.txt"],
        "x": [0, 1],
    }


def test_write_export_normalizes_unwritable_target(tmp_path):
    path = tmp_path / "target.csv"
    path.mkdir()
    bundle = prepare_export(
        [ExportSource("source.txt", pd.DataFrame({"x": [1], "y": [2]}))],
        [],
        ["x", "y"],
        "x",
        "degree",
        "csv",
    )

    with pytest.raises(DataIOError) as raised:
        write_export(path, bundle, "overwrite")

    assert raised.value.path == path
    assert raised.value.code == "delimited_write_failed"


def test_write_export_normalizes_dataframe_write_failure(tmp_path, monkeypatch):
    path = tmp_path / "failure.csv"
    bundle = prepare_export(
        [ExportSource("source.txt", pd.DataFrame({"x": [1]}))],
        [],
        ["x"],
        "x",
        "degree",
        "csv",
    )
    monkeypatch.setattr(
        pd.DataFrame,
        "to_csv",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(OSError("forced write failure")),
    )

    with pytest.raises(DataIOError) as raised:
        write_export(path, bundle, "overwrite")

    assert raised.value.path == path
    assert raised.value.code == "delimited_write_failed"


def test_write_xlsx_sanitizes_and_uniquifies_sheet_names_on_append(tmp_path):
    path = tmp_path / "sheets.xlsx"
    bundle = prepare_export(
        [ExportSource("bad:name?.txt", pd.DataFrame({"x": [1], "y": [2]}))],
        [FittedCurve("fit", [1, 2], [3, 4])],
        ["x", "y"],
        "x",
        "degree",
        "xlsx",
    )

    first = write_export(path, bundle, "overwrite")
    second = write_export(path, bundle, "append")

    assert len(first.sheet_names) == 2
    assert len(second.sheet_names) == 2
    assert all(len(name) <= 31 and not any(char in name for char in "[]:*?/\\") for name in second.sheet_names)
    with pd.ExcelFile(path) as workbook:
        assert len(workbook.sheet_names) == 4


def test_write_xlsx_round_trip_preserves_duplicate_basenames_on_append(tmp_path):
    path = tmp_path / "duplicate.xlsx"
    bundle = prepare_export(
        [
            ExportSource("first/same.txt", pd.DataFrame({"x": [1], "y": [2]})),
            ExportSource("second/same.txt", pd.DataFrame({"x": [3], "y": [4]})),
        ],
        [],
        ["x", "y"],
        "x",
        "degree",
        "xlsx",
    )

    first = write_export(path, bundle, "overwrite")
    second = write_export(path, bundle, "append")

    assert first.sheet_names == ["0_same", "1_same"]
    assert second.sheet_names == ["0_same_1", "1_same_1"]
    assert pd.read_excel(path, sheet_name="0_same").to_dict("list") == {"x": [1], "y": [2]}
    assert pd.read_excel(path, sheet_name="1_same_1").to_dict("list") == {"x": [3], "y": [4]}


def test_write_xlsx_normalizes_writer_failure(tmp_path, monkeypatch):
    path = tmp_path / "failure.xlsx"
    bundle = prepare_export(
        [ExportSource("source.txt", pd.DataFrame({"x": [1]}))],
        [],
        ["x"],
        "x",
        "degree",
        "xlsx",
    )
    monkeypatch.setattr(
        instplot_io.pd,
        "ExcelWriter",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(OSError("forced xlsx failure")),
    )

    with pytest.raises(DataIOError) as raised:
        write_export(path, bundle, "overwrite")

    assert raised.value.path == path
    assert raised.value.code == "xlsx_write_failed"


def test_prepare_export_keeps_empty_column_selection_as_all_columns():
    frame = pd.DataFrame({"x": [1], "y": [2]})

    bundle = prepare_export(
        [ExportSource("source.txt", frame)], [], [], "x", "degree", "txt"
    )

    assert bundle.table is not None
    assert bundle.table.columns.tolist() == ["source_file", "data_type", "x", "y"]


def test_prepare_export_normalizes_invalid_fitted_curve_data():
    with pytest.raises(DataIOError, match="export_prepare_failed"):
        prepare_export(
            [ExportSource("source.txt", pd.DataFrame({"x": [1]}))],
            [FittedCurve("bad", [1, 2], [3])],
            ["x"],
            "x",
            "degree",
            "csv",
        )
