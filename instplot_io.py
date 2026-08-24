"""Format-aware data import primitives with no GUI or plotting dependencies."""

from __future__ import annotations

import csv
from dataclasses import dataclass
from io import StringIO
from os import PathLike
from pathlib import Path
import re
from typing import Literal

import chardet
import pandas as pd


TEXT_EXTENSIONS = {".txt", ".csv", ".dat"}
EXCEL_EXTENSIONS = {".xls", ".xlsx"}
WHITESPACE_SEPARATOR = r"\s+"
PHYSICAL_LINE_PATTERN = re.compile(r"[^\r\n]*(?:\r\n|[\r\n]|$)")
SIGNIFICANT_LINE_PATTERN = re.compile(r"(?m)^[^\S\r\n]*(?!#)\S")


@dataclass
class ImportResult:
    """A parsed frame together with the decisions made while reading it."""

    path: Path
    frame: pd.DataFrame
    format: str
    encoding: str | None
    separator: str | None


@dataclass
class ExportSource:
    """One source table and the path label shown in exports."""

    path: str | PathLike[str]
    frame: pd.DataFrame


@dataclass
class FittedCurve:
    """Plot-independent fitted coordinates prepared by the GUI adapter."""

    name: str
    x: object
    y: object


@dataclass
class ExportBundle:
    """Purely prepared output content; writing it is a separate operation."""

    format: Literal["csv", "txt", "xlsx"]
    table: pd.DataFrame | None = None
    sheets: list[tuple[str, pd.DataFrame]] | None = None


@dataclass
class ExportResult:
    path: Path
    mode: Literal["overwrite", "append"]
    row_count: int
    sheet_names: list[str]


class DataIOError(RuntimeError):
    """Stable, user-displayable diagnostic for an import or export boundary."""

    def __init__(
        self,
        *,
        path: str | PathLike[str],
        operation: Literal["import", "export"],
        code: str,
        reason: str,
        encoding: str | None = None,
        separator: str | None = None,
        line_number: int | None = None,
    ) -> None:
        super().__init__(reason)
        self.path = Path(path)
        self.operation = operation
        self.code = code
        self.reason = reason
        self.encoding = encoding
        self.separator = separator
        self.line_number = line_number

    def __str__(self) -> str:
        parts = [
            f"文件: {self.path}",
            f"阶段: {self.operation}",
            f"错误: {self.code}",
            f"编码: {self.encoding or '未知'}",
            f"分隔符: {repr(self.separator) if self.separator is not None else '未知'}",
        ]
        if self.line_number is not None:
            parts.append(f"第 {self.line_number} 行")
        parts.append(f"原因: {self.reason}")
        return "；".join(parts)


def _split_fields(content: str, separator: str) -> list[str]:
    if separator == WHITESPACE_SEPARATOR:
        return re.split(WHITESPACE_SEPARATOR, content.strip())
    if '"' not in content:
        return content.split(separator)
    return next(csv.reader([content], delimiter=separator, strict=True))


def _quote_boundary_evidence(content: str, separator: str) -> tuple[int, int]:
    """Count structural quoted fields and literal quotes for one candidate dialect."""
    if separator == WHITESPACE_SEPARATOR:
        return 0, content.count('"')

    in_quotes = False
    at_field_start = True
    structural_quotes = 0
    literal_quotes = 0
    index = 0
    while index < len(content):
        character = content[index]
        if in_quotes:
            if character != '"':
                index += 1
                continue
            if in_quotes and index + 1 < len(content) and content[index + 1] == '"':
                index += 2
                continue
            in_quotes = False
        elif character == separator:
            at_field_start = True
        elif character == '"' and at_field_start:
            structural_quotes += 1
            in_quotes = True
            at_field_start = False
        else:
            if character == '"':
                literal_quotes += 1
            at_field_start = False
        index += 1
    return structural_quotes, literal_quotes


def _sample_significant_lines(text: str, limit: int = 10) -> list[tuple[int, str]]:
    """Collect a bounded sample while preserving zero-based physical line numbers."""
    lines: list[tuple[int, str]] = []
    for line_number, match in enumerate(PHYSICAL_LINE_PATTERN.finditer(text)):
        if match.start() == len(text):
            break
        line = match.group(0).rstrip("\r\n")
        if line.strip() and not line.lstrip().startswith("#"):
            lines.append((line_number, line))
            if len(lines) == limit:
                break
    return lines


def _numeric_row(line: str, columns_count: int | None = None) -> tuple[bool, str | None, int]:
    """Return whether a line is numeric and the separator that proves it."""
    for separator in (",", ";", WHITESPACE_SEPARATOR, "\t"):
        try:
            parts = [part.strip() for part in _split_fields(line, separator) if part.strip()]
        except csv.Error:
            continue
        if not parts:
            continue
        for part in parts:
            if part.lower() in {"nan", "na", "none", "inf"}:
                continue
            try:
                float(part)
            except ValueError:
                break
        else:
            if columns_count is None or len(parts) == columns_count:
                return True, separator, len(parts)
    return False, None, 0


def _find_header_row_index(text: str) -> tuple[int | None, int, str | None]:
    """Find the physical header/data positions without discarding blank line offsets."""
    lines = [
        (line_number, line.strip())
        for line_number, line in enumerate(text.splitlines())
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if not lines:
        return 0, 0, None
    if len(lines) == 1:
        line_number, line = lines[0]
        is_numeric, separator, _columns = _numeric_row(line)
        return (None, line_number, separator) if is_numeric else (line_number, line_number + 1, None)

    data_row_index = -1
    data_separator: str | None = None
    data_columns = 0
    for index, (_line_number, line) in enumerate(lines):
        is_numeric, separator, columns = _numeric_row(line)
        if is_numeric:
            data_row_index = index
            data_separator = separator
            data_columns = columns
            break
    if data_row_index == -1:
        return 0, 0, None

    assert data_separator is not None
    for index in range(data_row_index - 1, -1, -1):
        fields = [part.strip() for part in _split_fields(lines[index][1], data_separator) if part.strip()]
        if len(fields) != data_columns:
            continue
        for field in fields:
            if field.lower() in {"nan", "na", "none", "inf"}:
                continue
            try:
                float(field)
            except ValueError:
                return lines[index][0], lines[index][0] + 1, data_separator

    if data_row_index > 0:
        previous_line_number, previous_line = lines[data_row_index - 1]
        for field in _split_fields(previous_line, data_separator):
            field = field.strip()
            if not field or field.lower() in {"nan", "na", "none", "inf"}:
                continue
            try:
                float(field)
            except ValueError:
                return previous_line_number, previous_line_number + 1, data_separator

    return None, lines[data_row_index][0], data_separator


def _decode_text(path: Path) -> tuple[str, str]:
    """Read text bytes once, then try deterministic encoding candidates in memory."""
    raw = path.read_bytes()
    detected_encoding = chardet.detect(raw[:5000]).get("encoding")
    generic_single_byte_encodings = {
        "ascii",
        "iso-8859-1",
        "iso8859-1",
        "latin-1",
        "windows-1252",
        "cp1252",
    }
    candidates = ["utf-8-sig", "utf-8"]
    if detected_encoding and detected_encoding.lower() not in generic_single_byte_encodings:
        candidates.append(detected_encoding)
    candidates.extend(["gb18030", "gbk", "big5", detected_encoding, "cp1252", "latin-1", "mac_roman"])
    for encoding in dict.fromkeys(candidate for candidate in candidates if candidate):
        try:
            return raw.decode(encoding), encoding
        except (LookupError, UnicodeDecodeError):
            continue
    raise DataIOError(
        path=path,
        operation="import",
        code="decode_failed",
        reason="无法用常见编码读取文件",
    )


def _detect_vsm(path: Path) -> bool:
    with path.open("r", encoding="ascii", errors="ignore") as handle:
        preview_lines = [handle.readline().strip() for _ in range(10)]
    return any("vsm" in line.lower() for line in preview_lines if line)


def _detect_fallback_separator(
    text: str, *, path: Path, encoding: str
) -> tuple[int, int, str | None]:
    lines = _sample_significant_lines(text)
    if not lines:
        raise DataIOError(
            path=path,
            operation="import",
            code="empty_file",
            reason="文件为空或只包含空行或注释",
            encoding=encoding,
        )
    header_index, header_line = lines[0]
    data_index = lines[1][0] if len(lines) > 1 else header_index + 1
    candidates: list[dict[str, object]] = []
    for separator in (",", ";", "\t", WHITESPACE_SEPARATOR):
        if separator == WHITESPACE_SEPARATOR:
            if not any(character.isspace() for character in header_line):
                continue
        elif separator not in header_line:
            continue

        parsed_rows: list[tuple[str, ...] | None] = []
        structural_quotes = 0
        literal_quotes = 0
        parse_errors = 0
        for _line_number, line in lines:
            structural, literal = _quote_boundary_evidence(line, separator)
            structural_quotes += structural
            literal_quotes += literal
            try:
                parsed_rows.append(tuple(_split_fields(line, separator)))
            except csv.Error:
                parsed_rows.append(None)
                parse_errors += 1

        header_fields = parsed_rows[0]
        if header_fields is not None and len(header_fields) <= 1:
            continue
        header_width = len(header_fields) if header_fields is not None else None
        matching_rows = sum(
            row is not None and header_width is not None and len(row) == header_width
            for row in parsed_rows[1:]
        )
        mismatching_rows = sum(
            row is not None and header_width is not None and len(row) != header_width
            for row in parsed_rows[1:]
        )
        score = (
            int(structural_quotes > 0),
            structural_quotes,
            -literal_quotes,
            matching_rows,
            -mismatching_rows,
            -parse_errors,
        )
        candidates.append(
            {
                "separator": separator,
                "score": score,
                "parsed_rows": parsed_rows,
            }
        )

    if not candidates:
        return header_index, data_index, None

    best_score = max(candidate["score"] for candidate in candidates)
    winners = [candidate for candidate in candidates if candidate["score"] == best_score]
    if len(winners) > 1:
        winner_separators = {candidate["separator"] for candidate in winners}
        if winner_separators == {"\t", WHITESPACE_SEPARATOR} and all(
            candidate["parsed_rows"] == winners[0]["parsed_rows"] for candidate in winners[1:]
        ):
            return header_index, data_index, "\t"
        choices = ", ".join(repr(candidate["separator"]) for candidate in winners)
        raise DataIOError(
            path=path,
            operation="import",
            code="ambiguous_separator",
            reason=f"多个分隔符候选具有相同结构证据: {choices}",
            encoding=encoding,
        )

    return header_index, data_index, winners[0]["separator"]


def _normalize_and_validate_rows(
    text: str,
    separator: str,
    start_line: int,
    reference_line: int,
    *,
    path: Path,
    encoding: str,
) -> str:
    """Preserve valid fields while rejecting real width mismatches with source lines."""
    raw_lines = text.splitlines(keepends=True)
    try:
        expected_columns = len(
            _split_fields(raw_lines[reference_line].rstrip("\r\n"), separator)
        )
    except csv.Error as error:
        raise DataIOError(
            path=path,
            operation="import",
            code="text_parse_failed",
            reason=str(error),
            encoding=encoding,
            separator=separator,
            line_number=reference_line + 1,
        ) from error
    normalized_lines: list[str] = []
    for line_number, line in enumerate(raw_lines):
        if line_number < start_line or not line.strip() or line.lstrip().startswith("#"):
            normalized_lines.append(line)
            continue
        content = line.rstrip("\r\n")
        newline = line[len(content) :]
        try:
            fields = _split_fields(content, separator)
        except csv.Error as error:
            raise DataIOError(
                path=path,
                operation="import",
                code="text_parse_failed",
                reason=str(error),
                encoding=encoding,
                separator=separator,
                line_number=line_number + 1,
            ) from error
        removed_trailing_fields = False
        while (
            separator != WHITESPACE_SEPARATOR
            and len(fields) > expected_columns
            and not fields[-1].strip()
        ):
            fields.pop()
            removed_trailing_fields = True
        if len(fields) != expected_columns:
            raise DataIOError(
                path=path,
                operation="import",
                code="column_count_mismatch",
                reason=f"第 {line_number + 1} 行有 {len(fields)} 列，但表头只有 {expected_columns} 列",
                encoding=encoding,
                separator=separator,
                line_number=line_number + 1,
            )
        if separator == WHITESPACE_SEPARATOR or not removed_trailing_fields:
            normalized_lines.append(line)
        else:
            output = StringIO()
            csv.writer(output, delimiter=separator, lineterminator=newline).writerow(fields)
            normalized_lines.append(output.getvalue())
    return "".join(normalized_lines)


def _clean_columns(frame: pd.DataFrame) -> pd.DataFrame:
    cleaned = frame.copy()
    cleaned.columns = [re.sub(r"\s+", " ", str(column).strip()) for column in cleaned.columns]
    cleaned.columns = [
        column.replace("¦È", "θ").replace("¡ã", "°").replace("¦¸", "Ω").replace("Â", "").strip()
        for column in cleaned.columns
    ]
    return cleaned


def _read_text(path: Path) -> ImportResult:
    text, encoding = _decode_text(path)
    if not text or SIGNIFICANT_LINE_PATTERN.search(text) is None:
        raise DataIOError(
            path=path,
            operation="import",
            code="empty_file",
            reason="文件为空或只包含空行",
            encoding=encoding,
        )
    header_index, data_start_index, separator = _find_header_row_index(text)
    if separator is None:
        header_index, data_start_index, separator = _detect_fallback_separator(
            text, path=path, encoding=encoding
        )
    chosen_separator = separator or "\t"
    try:
        normalized_text = _normalize_and_validate_rows(
            text,
            chosen_separator,
            data_start_index,
            header_index if header_index is not None else data_start_index,
            path=path,
            encoding=encoding,
        )
        frame = pd.read_csv(
            StringIO(normalized_text),
            sep=chosen_separator,
            engine="python",
            skiprows=header_index if header_index is not None else data_start_index,
            header=0 if header_index is not None else None,
            index_col=False,
            comment="#",
        )
    except DataIOError:
        raise
    except Exception as error:
        raise DataIOError(
            path=path,
            operation="import",
            code="text_parse_failed",
            reason=str(error),
            encoding=encoding,
            separator=chosen_separator,
        ) from error
    if header_index is None:
        frame.columns = [f"Column {index + 1}" for index in range(frame.shape[1])]
    return ImportResult(path, _clean_columns(frame), "text", encoding, chosen_separator)


def _read_vsm(path: Path) -> ImportResult:
    try:
        frame = pd.read_csv(path, skiprows=31, header=None, usecols=[3, 4])
    except Exception as error:
        raise DataIOError(
            path=path,
            operation="import",
            code="vsm_parse_failed",
            reason=str(error),
            encoding="VSM",
            separator=",",
        ) from error
    frame.columns = ["B (Oe)", "M (emu)"]
    return ImportResult(path, _clean_columns(frame), "vsm", "VSM", ",")


def _read_excel(path: Path) -> ImportResult:
    try:
        frame = pd.read_excel(path, header=0)
    except Exception as error:
        raise DataIOError(
            path=path,
            operation="import",
            code="excel_parse_failed",
            reason=str(error),
            encoding="Excel",
        ) from error
    return ImportResult(path, _clean_columns(frame), path.suffix.lower().removeprefix("."), "Excel", None)


def read_data_file(path: str | PathLike[str]) -> ImportResult:
    """Read a supported data file and return its frame plus import diagnostics."""
    source_path = Path(path)
    suffix = source_path.suffix.lower()
    try:
        if suffix in EXCEL_EXTENSIONS:
            return _read_excel(source_path)
        if suffix not in TEXT_EXTENSIONS:
            raise DataIOError(
                path=source_path,
                operation="import",
                code="unsupported_extension",
                reason=f"不支持的文件扩展名: {suffix or '无'}",
            )
        if suffix == ".dat" and _detect_vsm(source_path):
            return _read_vsm(source_path)
        return _read_text(source_path)
    except DataIOError:
        raise
    except FileNotFoundError as error:
        raise DataIOError(
            path=source_path,
            operation="import",
            code="file_not_found",
            reason=str(error),
        ) from error
    except OSError as error:
        raise DataIOError(
            path=source_path,
            operation="import",
            code="file_read_failed",
            reason=str(error),
        ) from error


def _normalise_export_format(output_format: str) -> Literal["csv", "txt", "xlsx"]:
    normalised = output_format.lower().lstrip(".")
    if normalised not in {"csv", "txt", "xlsx"}:
        raise ValueError(f"不支持的导出格式: {output_format}")
    return normalised  # type: ignore[return-value]


def _prepare_source_frame(
    source: ExportSource,
    selected_columns: list[str] | None,
    x_column: str,
    x_unit_mode: str,
) -> pd.DataFrame:
    frame = source.frame.copy(deep=True)
    if x_unit_mode == "radian" and x_column in frame.columns:
        frame[x_column] = pd.to_numeric(frame[x_column], errors="coerce") * (3.141592653589793 / 180.0)
    if selected_columns:
        frame = frame[[column for column in selected_columns if column in frame.columns]]
    return frame


def _prepare_export(
    sources: list[ExportSource],
    fitted_curves: list[FittedCurve],
    selected_columns: list[str] | None,
    x_column: str,
    x_unit_mode: str,
    output_format: str,
) -> ExportBundle:
    """Construct export tables without displaying UI, opening files, or mutating inputs."""
    try:
        format_name = _normalise_export_format(output_format)
    except ValueError as error:
        raise DataIOError(
            path=".", operation="export", code="unsupported_export_format", reason=str(error)
        ) from error
    if not sources:
        raise DataIOError(path=".", operation="export", code="no_export_data", reason="没有数据可以导出")

    if format_name == "xlsx":
        sheets: list[tuple[str, pd.DataFrame]] = []
        for index, source in enumerate(sources):
            sheets.append((f"{index}_{Path(source.path).stem}", _prepare_source_frame(
                source, selected_columns, x_column, x_unit_mode
            )))
        if fitted_curves:
            fitted_frames = [
                pd.DataFrame({"x_fit": curve.x, "y_fit": curve.y}) for curve in fitted_curves
            ]
            sheets.append(("Fitted_Data", pd.concat(fitted_frames, axis=1)))
        return ExportBundle(format="xlsx", sheets=sheets)

    frames: list[pd.DataFrame] = []
    for source in sources:
        frame = _prepare_source_frame(source, selected_columns, x_column, x_unit_mode)
        frame.insert(0, "source_file", Path(source.path).name)
        frame.insert(1, "data_type", "raw_data")
        frames.append(frame)
    for index, curve in enumerate(fitted_curves):
        frames.append(
            pd.DataFrame(
                {
                    "source_file": curve.name or f"fitted_curve_{index + 1}",
                    "data_type": "fitted_data",
                    "x_fit": curve.x,
                    "y_fit": curve.y,
                }
            )
        )
    return ExportBundle(format=format_name, table=pd.concat(frames, ignore_index=True, sort=False))


def prepare_export(
    sources: list[ExportSource],
    fitted_curves: list[FittedCurve],
    selected_columns: list[str] | None,
    x_column: str,
    x_unit_mode: str,
    output_format: str,
) -> ExportBundle:
    """Prepare an export bundle and normalize all preparation failures."""
    try:
        return _prepare_export(
            sources, fitted_curves, selected_columns, x_column, x_unit_mode, output_format
        )
    except DataIOError:
        raise
    except Exception as error:
        raise DataIOError(
            path=".", operation="export", code="export_prepare_failed", reason=str(error)
        ) from error


def _unique_sheet_name(name: str, existing_names: set[str]) -> str:
    base = re.sub(r"[\[\]:*?/\\]", "_", name).strip() or "Sheet"
    base = base[:31]
    candidate = base
    suffix = 1
    while candidate in existing_names:
        suffix_text = f"_{suffix}"
        candidate = f"{base[:31 - len(suffix_text)]}{suffix_text}"
        suffix += 1
    existing_names.add(candidate)
    return candidate


def _write_delimited(path: Path, bundle: ExportBundle, mode: Literal["overwrite", "append"]) -> ExportResult:
    assert bundle.table is not None
    separator = "," if bundle.format == "csv" else "\t"
    table = bundle.table
    write_header = True
    if mode == "append" and path.exists():
        try:
            existing_columns = pd.read_csv(path, sep=separator, encoding="utf-8-sig", nrows=0, comment="#").columns.tolist()
        except Exception as error:
            raise DataIOError(
                path=path, operation="export", code="append_schema_read_failed", reason=str(error), separator=separator
            ) from error
        if set(existing_columns) != set(table.columns):
            raise DataIOError(
                path=path,
                operation="export",
                code="append_schema_mismatch",
                reason="追加失败：现有文件的列结构不同。请使用“覆盖”或另存为新文件。",
                encoding="utf-8-sig",
                separator=separator,
            )
        table = table[existing_columns]
        write_header = False
    try:
        table.to_csv(
            path,
            sep=separator,
            index=False,
            mode="a" if mode == "append" and path.exists() else "w",
            header=write_header,
            encoding="utf-8-sig",
        )
    except Exception as error:
        raise DataIOError(
            path=path, operation="export", code="delimited_write_failed", reason=str(error), encoding="utf-8-sig", separator=separator
        ) from error
    return ExportResult(path, mode, len(table), [])


def _write_xlsx(path: Path, bundle: ExportBundle, mode: Literal["overwrite", "append"]) -> ExportResult:
    assert bundle.sheets is not None
    try:
        import openpyxl  # noqa: F401
        writer_mode = "a" if mode == "append" and path.exists() else "w"
        with pd.ExcelWriter(path, engine="openpyxl", mode=writer_mode) as writer:
            existing_names = set(writer.book.sheetnames)
            sheet_names: list[str] = []
            row_count = 0
            for proposed_name, frame in bundle.sheets:
                sheet_name = _unique_sheet_name(proposed_name, existing_names)
                frame.to_excel(writer, sheet_name=sheet_name, index=False)
                sheet_names.append(sheet_name)
                row_count += len(frame)
    except DataIOError:
        raise
    except Exception as error:
        raise DataIOError(path=path, operation="export", code="xlsx_write_failed", reason=str(error)) from error
    return ExportResult(path, mode, row_count, sheet_names)


def write_export(
    path: str | PathLike[str], bundle: ExportBundle, mode: Literal["overwrite", "append"]
) -> ExportResult:
    """Write a prepared bundle with explicit overwrite or append semantics."""
    destination = Path(path)
    if mode not in {"overwrite", "append"}:
        raise DataIOError(path=destination, operation="export", code="invalid_export_mode", reason=f"无效导出模式: {mode}")
    if destination.suffix.lower().lstrip(".") != bundle.format:
        raise DataIOError(
            path=destination,
            operation="export",
            code="export_extension_mismatch",
            reason=f"目标扩展名与导出格式不一致: {destination.suffix or '无'}",
        )
    if bundle.format == "xlsx":
        return _write_xlsx(destination, bundle, mode)
    return _write_delimited(destination, bundle, mode)
