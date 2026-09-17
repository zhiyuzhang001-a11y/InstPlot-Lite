use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use calamine::{Data, DataType, Reader, open_workbook_auto};
use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{GBK, UTF_16BE, UTF_16LE};

const TEXT_EXTENSIONS: &[&str] = &["txt", "csv", "dat", "tsv"];
const SPREADSHEET_EXTENSIONS: &[&str] = &["xlsx", "xls"];
const SECTION_BEGIN: &str = "# -----BEGIN INSTPLOT DATA-----";
const SECTION_END: &str = "# -----END INSTPLOT DATA-----";

#[derive(Clone, Debug)]
pub struct NumericColumn {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitLink {
    pub parent_dataset_id: Option<String>,
    pub source_x_column: String,
    pub source_y_column: String,
    pub equation: Option<String>,
    pub display_equation: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataSetKind {
    #[default]
    Source,
    Fit,
}

impl DataSetKind {
    pub fn metadata_value(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Fit => "fit",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataSet {
    pub source: PathBuf,
    pub label: Option<String>,
    pub kind: DataSetKind,
    pub plot_id: String,
    pub fit_link: Option<FitLink>,
    pub encoding: String,
    pub separator: String,
    pub columns: Vec<NumericColumn>,
    pub row_count: usize,
    pub alive: Vec<bool>,
}

fn generated_plot_id(path: &Path, columns: &[NumericColumn]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;
    let mut hash = OFFSET_BASIS;
    let mut update = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    update(path.to_string_lossy().as_bytes());
    for column in columns {
        update(column.name.as_bytes());
        for value in &column.values {
            update(&value.to_bits().to_le_bytes());
        }
    }
    format!("instplot-{hash:016x}")
}

impl DataSet {
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        self.source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未命名数据")
            .to_owned()
    }

    pub fn plot_points(
        &self,
        x_column: usize,
        y_column: usize,
        limit: usize,
        x_range: Option<[f64; 2]>,
    ) -> Vec<[f64; 2]> {
        let Some(x_values) = self.columns.get(x_column).map(|column| &column.values) else {
            return Vec::new();
        };
        let Some(y_values) = self.columns.get(y_column).map(|column| &column.values) else {
            return Vec::new();
        };
        let in_range = |x: f64| {
            x_range.is_none_or(|[left, right]| x >= left.min(right) && x <= left.max(right))
        };
        let eligible_count = x_values
            .iter()
            .zip(y_values)
            .zip(&self.alive)
            .filter(|(values, alive)| {
                let (x, y) = values;
                **alive && x.is_finite() && y.is_finite() && in_range(**x)
            })
            .count();
        let limit = limit.max(2);
        if eligible_count <= limit {
            return x_values
                .iter()
                .zip(y_values)
                .zip(&self.alive)
                .filter_map(|((x, y), alive)| {
                    (*alive && x.is_finite() && y.is_finite() && in_range(*x)).then_some([*x, *y])
                })
                .collect();
        }

        // Keep both y extrema from each source-order bucket. This preserves
        // narrow peaks that ordinary stride sampling can silently skip.
        let bucket_size = eligible_count.div_ceil(limit / 2).max(1);
        let mut output = Vec::with_capacity(limit + 2);
        let mut bucket: Vec<(usize, [f64; 2])> = Vec::with_capacity(bucket_size);
        for (row_index, ((x, y), alive)) in
            x_values.iter().zip(y_values).zip(&self.alive).enumerate()
        {
            if !*alive || !x.is_finite() || !y.is_finite() || !in_range(*x) {
                continue;
            }
            bucket.push((row_index, [*x, *y]));
            if bucket.len() == bucket_size {
                append_bucket_extrema(&mut output, &bucket);
                bucket.clear();
            }
        }
        append_bucket_extrema(&mut output, &bucket);
        output
    }

    pub fn row_points(
        &self,
        x_column: usize,
        y_column: usize,
    ) -> impl Iterator<Item = (usize, [f64; 2])> + '_ {
        let x_values = self
            .columns
            .get(x_column)
            .map_or(&[][..], |column| column.values.as_slice());
        let y_values = self
            .columns
            .get(y_column)
            .map_or(&[][..], |column| column.values.as_slice());
        x_values
            .iter()
            .zip(y_values)
            .zip(&self.alive)
            .enumerate()
            .filter_map(|(row_index, ((x, y), alive))| {
                (*alive && x.is_finite() && y.is_finite()).then_some((row_index, [*x, *y]))
            })
    }

    pub fn rows_in_bounds(
        &self,
        x_column: usize,
        y_column: usize,
        x_bounds: [f64; 2],
        y_bounds: [f64; 2],
    ) -> Vec<usize> {
        let x_min = x_bounds[0].min(x_bounds[1]);
        let x_max = x_bounds[0].max(x_bounds[1]);
        let y_min = y_bounds[0].min(y_bounds[1]);
        let y_max = y_bounds[0].max(y_bounds[1]);
        self.row_points(x_column, y_column)
            .filter_map(|(row_index, [x, y])| {
                (x >= x_min && x <= x_max && y >= y_min && y <= y_max).then_some(row_index)
            })
            .collect()
    }

    pub fn delete_rows(&mut self, rows: &[usize]) -> Vec<usize> {
        let mut changed = Vec::with_capacity(rows.len());
        for &row_index in rows {
            if let Some(alive) = self.alive.get_mut(row_index)
                && *alive
            {
                *alive = false;
                changed.push(row_index);
            }
        }
        changed
    }

    pub fn restore_rows(&mut self, rows: &[usize]) {
        for &row_index in rows {
            if let Some(alive) = self.alive.get_mut(row_index) {
                *alive = true;
            }
        }
    }
}

fn append_bucket_extrema(output: &mut Vec<[f64; 2]>, bucket: &[(usize, [f64; 2])]) {
    let Some(minimum) = bucket
        .iter()
        .min_by(|left, right| left.1[1].total_cmp(&right.1[1]))
    else {
        return;
    };
    let Some(maximum) = bucket
        .iter()
        .max_by(|left, right| left.1[1].total_cmp(&right.1[1]))
    else {
        return;
    };
    if minimum.0 <= maximum.0 {
        output.push(minimum.1);
        if minimum.0 != maximum.0 {
            output.push(maximum.1);
        }
    } else {
        output.push(maximum.1);
        output.push(minimum.1);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportError {
    pub code: &'static str,
    pub line_number: Option<usize>,
    pub reason: String,
}

impl ImportError {
    fn new(code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            code,
            line_number: None,
            reason: reason.into(),
        }
    }

    fn at_line(code: &'static str, line_number: usize, reason: impl Into<String>) -> Self {
        Self {
            code,
            line_number: Some(line_number),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line_number) = self.line_number {
            write!(
                formatter,
                "{}（第 {} 行）：{}",
                self.code, line_number, self.reason
            )
        } else {
            write!(formatter, "{}：{}", self.code, self.reason)
        }
    }
}

impl std::error::Error for ImportError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Separator {
    Character(u8),
    Whitespace,
}

impl Separator {
    fn label(self) -> String {
        match self {
            Self::Character(b'\t') => "制表符".to_owned(),
            Self::Character(character) => (character as char).to_string(),
            Self::Whitespace => "空白字符".to_owned(),
        }
    }
}

pub fn read_data_file(path: &Path) -> Result<Vec<DataSet>, ImportError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if SPREADSHEET_EXTENSIONS.contains(&extension.as_str()) {
        return read_spreadsheet(path);
    }
    if !TEXT_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ImportError::new(
            "unsupported_extension",
            format!("仅支持 TXT、CSV、DAT、TSV、XLSX 和 XLS，收到 .{extension}"),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| ImportError::new("file_read_failed", error.to_string()))?;
    let (text, encoding) = decode_text(&bytes)?;
    parse_text_datasets(path, &text, encoding)
}

fn read_spreadsheet(path: &Path) -> Result<Vec<DataSet>, ImportError> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| ImportError::new("spreadsheet_open_failed", error.to_string()))?;
    let sheet_names = workbook.sheet_names();
    let has_multiple_sheets = sheet_names.len() > 1;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名工作簿");
    let mut datasets = Vec::new();

    for sheet_name in sheet_names {
        let range = workbook.worksheet_range(&sheet_name).map_err(|error| {
            ImportError::new(
                "spreadsheet_sheet_failed",
                format!("工作表“{sheet_name}”读取失败：{error}"),
            )
        })?;
        match spreadsheet_range_to_dataset(path, &sheet_name, &range) {
            Ok(mut dataset) => {
                if has_multiple_sheets {
                    dataset.label = Some(format!("{file_name} — {sheet_name}"));
                }
                datasets.push(dataset);
            }
            Err(error)
                if matches!(
                    error.code,
                    "empty_file" | "no_numeric_data" | "insufficient_numeric_columns"
                ) => {}
            Err(error) => {
                return Err(ImportError {
                    reason: format!("工作表“{sheet_name}”：{}", error.reason),
                    ..error
                });
            }
        }
    }

    if datasets.is_empty() {
        return Err(ImportError::new(
            "spreadsheet_no_numeric_data",
            "工作簿中没有包含至少两个数值列的工作表",
        ));
    }
    validate_unique_source_plot_ids(&datasets)?;
    Ok(datasets)
}

fn spreadsheet_range_to_dataset(
    path: &Path,
    sheet_name: &str,
    range: &calamine::Range<Data>,
) -> Result<DataSet, ImportError> {
    let rows: Vec<&[Data]> = range.rows().collect();
    if rows.is_empty() {
        return Err(ImportError::new("empty_file", "工作表为空"));
    }
    let (data_position, column_count) = rows
        .iter()
        .enumerate()
        .find_map(|(position, row)| {
            // A worksheet is already a rectangular grid. Keep its full used
            // width so an empty value in the first numeric row cannot shift or
            // discard a later column.
            let width = row.len();
            let numeric_count = row[..width]
                .iter()
                .filter(|cell| spreadsheet_finite_number(cell).is_some())
                .count();
            (width >= 2 && numeric_count >= 2).then_some((position, width))
        })
        .ok_or_else(|| ImportError::new("no_numeric_data", "未找到至少两列数值数据"))?;

    let mut data_start_position = data_position;
    while data_start_position > 0
        && spreadsheet_partial_numeric_row(rows[data_start_position - 1], column_count)
    {
        data_start_position -= 1;
    }

    let matching_header_position = (0..data_start_position).rev().find(|position| {
        let row = rows[*position];
        spreadsheet_row_width(row) == column_count
            && row[..column_count]
                .iter()
                .any(|cell| !cell.is_empty() && spreadsheet_number(cell).is_none())
    });
    let adjacent_header_position = data_start_position.checked_sub(1).filter(|position| {
        let row = rows[*position];
        spreadsheet_row_width(row) >= 2
            && row
                .iter()
                .take(column_count)
                .any(|cell| !cell.is_empty() && spreadsheet_number(cell).is_none())
    });
    let header_position = matching_header_position.or(adjacent_header_position);
    let headers = unique_column_names(header_position.map_or_else(
        || {
            (1..=column_count)
                .map(|index| format!("Column {index}"))
                .collect::<Vec<_>>()
        },
        |position| {
            rows[position][..column_count]
                .iter()
                .enumerate()
                .map(|(index, cell)| {
                    let name = clean_header(&spreadsheet_cell_text(cell));
                    if name.is_empty() {
                        format!("Column {}", index + 1)
                    } else {
                        name
                    }
                })
                .collect()
        },
    ));

    let mut values = vec![Vec::new(); column_count];
    let mut numeric_counts = vec![0_usize; column_count];
    for (row_index, row) in rows.iter().enumerate().skip(data_start_position) {
        let width = spreadsheet_row_width(row);
        if width > column_count {
            return Err(ImportError::at_line(
                "column_count_mismatch",
                row_index + 1,
                format!(
                    "该行有 {width} 列，但首行数据定义了 {column_count} 列；为防止列名错位，工作表未导入"
                ),
            ));
        }
        for column_index in 0..column_count {
            let number = row
                .get(column_index)
                .and_then(spreadsheet_number)
                .unwrap_or(f64::NAN);
            if number.is_finite() {
                numeric_counts[column_index] += 1;
            }
            values[column_index].push(number);
        }
    }

    let row_count = values.first().map(Vec::len).unwrap_or(0);
    let columns: Vec<NumericColumn> = headers
        .into_iter()
        .zip(values)
        .zip(numeric_counts)
        .filter_map(|((name, values), numeric_count)| {
            (numeric_count > 0).then_some(NumericColumn { name, values })
        })
        .collect();
    if columns.len() < 2 {
        return Err(ImportError::new(
            "insufficient_numeric_columns",
            format!("只识别到 {} 个数值列，绘图至少需要两个", columns.len()),
        ));
    }

    let kind = spreadsheet_dataset_kind(&rows[..data_start_position])?;
    let (stored_plot_id, fit_link) = spreadsheet_plot_metadata(&rows[..data_start_position], kind)?;
    let plot_id = stored_plot_id.unwrap_or_else(|| {
        let mut sheet_identity = path.as_os_str().to_os_string();
        sheet_identity.push(format!("#{sheet_name}"));
        generated_plot_id(Path::new(&sheet_identity), &columns)
    });
    Ok(DataSet {
        source: path.to_path_buf(),
        label: None,
        kind,
        plot_id,
        fit_link,
        encoding: "Excel 工作簿".to_owned(),
        separator: format!("工作表 {sheet_name}"),
        columns,
        row_count,
        alive: vec![true; row_count],
    })
}

fn spreadsheet_row_width(row: &[Data]) -> usize {
    row.iter()
        .rposition(|cell| !cell.is_empty())
        .map_or(0, |index| index + 1)
}

fn spreadsheet_finite_number(cell: &Data) -> Option<f64> {
    spreadsheet_number(cell).filter(|number| number.is_finite())
}

fn spreadsheet_partial_numeric_row(row: &[Data], column_count: usize) -> bool {
    let width = spreadsheet_row_width(row);
    width > 0
        && width <= column_count
        && row[..width]
            .iter()
            .all(|cell| cell.is_empty() || spreadsheet_number(cell).is_some())
        && row[..width]
            .iter()
            .any(|cell| spreadsheet_finite_number(cell).is_some())
}

fn spreadsheet_number(cell: &Data) -> Option<f64> {
    cell.as_f64().or_else(|| match cell {
        Data::String(value) => parse_number(value),
        Data::Empty => Some(f64::NAN),
        _ => None,
    })
}

fn spreadsheet_cell_text(cell: &Data) -> String {
    cell.to_string().replace(['\r', '\n'], " ")
}

fn parse_dataset_kind(value: &str) -> Result<DataSetKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "source" => Ok(DataSetKind::Source),
        "fit" => Ok(DataSetKind::Fit),
        value => Err(format!(
            "不支持的 InstPlot 数据区类型“{value}”，应为 source 或 fit"
        )),
    }
}

fn spreadsheet_dataset_kind(metadata_rows: &[&[Data]]) -> Result<DataSetKind, ImportError> {
    let mut kind = None;
    for (row_index, row) in metadata_rows.iter().enumerate() {
        let Some(value) = row
            .first()
            .map(spreadsheet_cell_text)
            .and_then(|text| text.trim().strip_prefix("# Type:").map(str::to_owned))
        else {
            continue;
        };
        if kind.is_some() {
            return Err(ImportError::at_line(
                "duplicate_section_type",
                row_index + 1,
                "同一工作表只能包含一个 InstPlot Type 元数据",
            ));
        }
        kind = Some(parse_dataset_kind(&value).map_err(|reason| {
            ImportError::at_line("invalid_section_type", row_index + 1, reason)
        })?);
    }
    Ok(kind.unwrap_or_default())
}

fn spreadsheet_plot_metadata(
    metadata_rows: &[&[Data]],
    kind: DataSetKind,
) -> Result<(Option<String>, Option<FitLink>), ImportError> {
    let mut dataset_id = None;
    let mut parent_id = None;
    let mut source_x = None;
    let mut source_y = None;
    let mut equation = None;
    let mut display_equation = None;
    for row in metadata_rows {
        let Some(text) = row.first().map(spreadsheet_cell_text) else {
            continue;
        };
        let trimmed = text.trim();
        let value = |prefix: &str| {
            trimmed
                .strip_prefix(prefix)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        dataset_id = value("# Dataset-ID:").or(dataset_id);
        parent_id = value("# Parent-ID:").or(parent_id);
        source_x = value("# Source-X:").or(source_x);
        source_y = value("# Source-Y:").or(source_y);
        equation = value("# Equation:").or(equation);
        display_equation = value("# Display-Equation:").or(display_equation);
    }
    let fit_link = match (parent_id, source_x, source_y) {
        (None, None, None) => None,
        (Some(parent), Some(source_x_column), Some(source_y_column))
            if kind == DataSetKind::Fit =>
        {
            Some(FitLink {
                parent_dataset_id: (parent != "*").then_some(parent),
                source_x_column,
                source_y_column,
                equation,
                display_equation,
            })
        }
        _ => {
            return Err(ImportError::new(
                "incomplete_fit_link",
                "拟合关联元数据必须同时包含 Parent-ID、Source-X 和 Source-Y",
            ));
        }
    };
    Ok((dataset_id, fit_link))
}

#[cfg(test)]
fn read_data_bytes(path: &Path, bytes: &[u8]) -> Result<DataSet, ImportError> {
    let (text, encoding) = decode_text(bytes)?;
    parse_text(path, &text, encoding)
}

fn parse_text_datasets(
    path: &Path,
    text: &str,
    encoding: String,
) -> Result<Vec<DataSet>, ImportError> {
    let has_boundary = text.lines().any(|line| {
        let line = line.trim();
        line == SECTION_BEGIN || line == SECTION_END
    });
    if !has_boundary {
        return parse_text(path, text, encoding).map(|dataset| vec![dataset]);
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名文件");
    let mut datasets = Vec::new();
    let mut section_name = None::<String>;
    let mut section_kind = None::<DataSetKind>;
    let mut section_dataset_id = None::<String>;
    let mut section_parent_id = None::<String>;
    let mut section_source_x = None::<String>;
    let mut section_source_y = None::<String>;
    let mut section_equation = None::<String>;
    let mut section_display_equation = None::<String>;
    let mut section_lines = Vec::<&str>::new();
    let mut section_start = 0_usize;

    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed == SECTION_BEGIN {
            if section_name.is_some() {
                return Err(ImportError::at_line(
                    "nested_section",
                    line_index + 1,
                    "上一个 InstPlot 数据区尚未结束",
                ));
            }
            section_name = Some(format!("数据区 {}", datasets.len() + 1));
            section_kind = None;
            section_dataset_id = None;
            section_parent_id = None;
            section_source_x = None;
            section_source_y = None;
            section_equation = None;
            section_display_equation = None;
            section_lines.clear();
            section_start = line_index + 1;
            continue;
        }
        if trimmed == SECTION_END {
            let Some(name) = section_name.take() else {
                return Err(ImportError::at_line(
                    "unexpected_section_end",
                    line_index + 1,
                    "发现了没有对应 BEGIN 的 END 标记",
                ));
            };
            let body = section_lines.join("\n");
            let mut dataset =
                parse_text(path, &body, encoding.clone()).map_err(|error| ImportError {
                    reason: format!("数据区“{name}”：{}", error.reason),
                    line_number: error.line_number.map(|line| line + section_start),
                    ..error
                })?;
            dataset.label = Some(format!("{file_name} — {name}"));
            dataset.kind = section_kind.take().unwrap_or_default();
            if let Some(dataset_id) = section_dataset_id.take() {
                dataset.plot_id = dataset_id;
            }
            dataset.fit_link = match (
                section_parent_id.take(),
                section_source_x.take(),
                section_source_y.take(),
            ) {
                (None, None, None) => None,
                (Some(parent), Some(source_x_column), Some(source_y_column))
                    if dataset.kind == DataSetKind::Fit =>
                {
                    Some(FitLink {
                        parent_dataset_id: (parent != "*").then_some(parent),
                        source_x_column,
                        source_y_column,
                        equation: section_equation.take(),
                        display_equation: section_display_equation.take(),
                    })
                }
                _ => {
                    return Err(ImportError::at_line(
                        "incomplete_fit_link",
                        line_index + 1,
                        "拟合关联元数据必须同时包含 Parent-ID、Source-X 和 Source-Y",
                    ));
                }
            };
            datasets.push(dataset);
            section_lines.clear();
            continue;
        }
        if let Some(current_name) = section_name.as_mut() {
            if let Some(name) = trimmed.strip_prefix("# Name:") {
                let name = name.trim();
                if !name.is_empty() {
                    *current_name = name.to_owned();
                }
            } else if let Some(value) = trimmed.strip_prefix("# Type:") {
                if section_kind.is_some() {
                    return Err(ImportError::at_line(
                        "duplicate_section_type",
                        line_index + 1,
                        "同一 InstPlot 数据区只能包含一个 Type 元数据",
                    ));
                }
                section_kind = Some(parse_dataset_kind(value).map_err(|reason| {
                    ImportError::at_line("invalid_section_type", line_index + 1, reason)
                })?);
            } else if let Some(value) = trimmed.strip_prefix("# Dataset-ID:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_dataset_id = Some(value.to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("# Parent-ID:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_parent_id = Some(value.to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("# Source-X:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_source_x = Some(value.to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("# Source-Y:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_source_y = Some(value.to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("# Equation:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_equation = Some(value.to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("# Display-Equation:") {
                let value = value.trim();
                if !value.is_empty() {
                    section_display_equation = Some(value.to_owned());
                }
            } else {
                section_lines.push(line);
            }
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            return Err(ImportError::at_line(
                "data_outside_section",
                line_index + 1,
                "InstPlot 分区文件中的数据必须位于 BEGIN 和 END 标记之间",
            ));
        }
    }

    if let Some(name) = section_name {
        return Err(ImportError::at_line(
            "unterminated_section",
            section_start,
            format!("数据区“{name}”缺少 END 标记"),
        ));
    }
    if datasets.is_empty() {
        return Err(ImportError::new(
            "empty_sections",
            "文件包含 InstPlot 分区标记，但没有可读取的数据区",
        ));
    }
    validate_unique_source_plot_ids(&datasets)?;
    Ok(datasets)
}

fn validate_unique_source_plot_ids(datasets: &[DataSet]) -> Result<(), ImportError> {
    let mut source_ids = HashSet::new();
    for dataset in datasets
        .iter()
        .filter(|dataset| dataset.kind == DataSetKind::Source)
    {
        if !source_ids.insert(dataset.plot_id.as_str()) {
            return Err(ImportError::new(
                "duplicate_dataset_id",
                format!(
                    "同一文件中有多个原始数据区使用 Dataset-ID“{}”；为防止拟合关联错误，未导入",
                    dataset.plot_id
                ),
            ));
        }
    }
    Ok(())
}

fn decode_text(bytes: &[u8]) -> Result<(String, String), ImportError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return std::str::from_utf8(&bytes[3..])
            .map(|text| (text.to_owned(), "UTF-8 BOM".to_owned()))
            .map_err(|error| ImportError::new("decode_failed", error.to_string()));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, had_errors) = UTF_16LE.decode(&bytes[2..]);
        return (!had_errors)
            .then(|| (text.into_owned(), "UTF-16LE".to_owned()))
            .ok_or_else(|| ImportError::new("decode_failed", "UTF-16LE 文件包含无效字节"));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, had_errors) = UTF_16BE.decode(&bytes[2..]);
        return (!had_errors)
            .then(|| (text.into_owned(), "UTF-16BE".to_owned()))
            .ok_or_else(|| ImportError::new("decode_failed", "UTF-16BE 文件包含无效字节"));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_owned(), "UTF-8".to_owned()));
    }
    if let Some(text) = GBK.decode_without_bom_handling_and_without_replacement(bytes) {
        return Ok((text.into_owned(), "GBK".to_owned()));
    }

    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let encoding = detector.guess(None, Utf8Detection::Deny);
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(ImportError::new(
            "decode_failed",
            "无法使用常见中文或西文编码读取文件",
        ));
    }
    Ok((text.into_owned(), encoding.name().to_owned()))
}

fn parse_text(path: &Path, text: &str, encoding: String) -> Result<DataSet, ImportError> {
    let all_significant: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .collect();
    if all_significant.is_empty() {
        return Err(ImportError::new(
            "empty_file",
            "文件为空，或只包含空行和注释",
        ));
    }
    let data_section_start = all_significant
        .iter()
        .rposition(|(_, line)| line.trim().eq_ignore_ascii_case("[data]"))
        .map_or(0, |position| position + 1);
    let significant = &all_significant[data_section_start..];
    if significant.is_empty() {
        return Err(ImportError::new(
            "empty_file",
            "[Data] 段之后没有可读取的数据",
        ));
    }

    let (data_position, separator, detected_column_count) = significant
        .iter()
        .enumerate()
        .find_map(|(position, (_, line))| {
            numeric_row(line).map(|(separator, width)| (position, separator, width))
        })
        .ok_or_else(|| ImportError::new("no_numeric_data", "未找到至少两列数值数据"))?;

    let mut data_start_position = data_position;
    while data_start_position > 0
        && text_partial_numeric_row(
            significant[data_start_position - 1].1,
            separator,
            detected_column_count,
        )
    {
        data_start_position -= 1;
    }

    let matching_header_position = (0..data_start_position).rev().find(|position| {
        split_fields(significant[*position].1, separator)
            .map(|mut fields| {
                trim_excess_trailing_empty_fields(&mut fields, detected_column_count);
                fields.len() == detected_column_count
                    && fields.iter().any(|field| !is_number(field))
            })
            .unwrap_or(false)
    });
    let adjacent_header_position = data_start_position.checked_sub(1).filter(|position| {
        split_fields(significant[*position].1, separator)
            .map(|fields| fields.len() >= 2 && fields.iter().any(|field| !is_number(field)))
            .unwrap_or(false)
    });
    let header_position = matching_header_position.or(adjacent_header_position);
    let (headers, column_count): (Vec<String>, usize) = if let Some(position) = header_position {
        let mut fields = split_fields(significant[position].1, separator)?;
        trim_excess_trailing_empty_fields(&mut fields, detected_column_count);
        let width = fields.len();
        (
            fields
                .into_iter()
                .map(|field| clean_header(&field))
                .collect(),
            width,
        )
    } else {
        (
            (1..=detected_column_count)
                .map(|index| format!("Column {index}"))
                .collect(),
            detected_column_count,
        )
    };

    let headers = unique_column_names(headers);
    let mut values = vec![Vec::new(); column_count];
    let mut numeric_counts = vec![0_usize; column_count];
    for (physical_line, line) in significant.iter().skip(data_start_position) {
        let mut fields = split_fields(line, separator).map_err(|error| ImportError {
            line_number: Some(physical_line + 1),
            ..error
        })?;
        trim_excess_trailing_empty_fields(&mut fields, column_count);
        if fields.len() != column_count {
            return Err(ImportError::at_line(
                "column_count_mismatch",
                physical_line + 1,
                format!(
                    "该行有 {} 列，但表头或首行数据定义了 {} 列；为防止列名错位，文件未导入",
                    fields.len(),
                    column_count
                ),
            ));
        }
        for (column_index, field) in fields.iter().enumerate() {
            let number = parse_number(field).unwrap_or(f64::NAN);
            if number.is_finite() {
                numeric_counts[column_index] += 1;
            }
            values[column_index].push(number);
        }
    }

    let row_count = values.first().map(Vec::len).unwrap_or(0);
    let columns: Vec<NumericColumn> = headers
        .into_iter()
        .zip(values)
        .zip(numeric_counts)
        .filter_map(|((name, values), numeric_count)| {
            (numeric_count > 0).then_some(NumericColumn { name, values })
        })
        .collect();
    if columns.len() < 2 {
        return Err(ImportError::new(
            "insufficient_numeric_columns",
            format!("只识别到 {} 个数值列，绘图至少需要两个", columns.len()),
        ));
    }

    let plot_id = generated_plot_id(path, &columns);
    Ok(DataSet {
        source: path.to_path_buf(),
        label: None,
        kind: DataSetKind::Source,
        plot_id,
        fit_link: None,
        encoding,
        separator: separator.label(),
        columns,
        row_count,
        alive: vec![true; row_count],
    })
}

fn numeric_row(line: &str) -> Option<(Separator, usize)> {
    let candidates = if line.contains('\t') {
        [
            Separator::Character(b'\t'),
            Separator::Character(b','),
            Separator::Character(b';'),
            Separator::Whitespace,
        ]
    } else {
        [
            Separator::Character(b','),
            Separator::Character(b';'),
            Separator::Whitespace,
            Separator::Character(b'\t'),
        ]
    };
    candidates.into_iter().find_map(|separator| {
        let fields = split_fields(line, separator).ok()?;
        let numeric_count = fields.iter().filter(|field| is_number(field)).count();
        (fields.len() >= 2 && numeric_count >= 2).then_some((separator, fields.len()))
    })
}

fn text_partial_numeric_row(line: &str, separator: Separator, column_count: usize) -> bool {
    let Ok(mut fields) = split_fields(line, separator) else {
        return false;
    };
    trim_excess_trailing_empty_fields(&mut fields, column_count);
    fields.len() == column_count
        && fields
            .iter()
            .all(|field| field.is_empty() || is_number(field))
        && fields
            .iter()
            .any(|field| parse_number(field).is_some_and(f64::is_finite))
}

fn split_fields(line: &str, separator: Separator) -> Result<Vec<String>, ImportError> {
    if separator == Separator::Whitespace {
        return Ok(line.split_whitespace().map(str::to_owned).collect());
    }
    let Separator::Character(delimiter) = separator else {
        unreachable!();
    };
    if delimiter == b'\t' && !line.contains('\t') {
        return Ok(line.split_whitespace().map(str::to_owned).collect());
    }
    if !line.as_bytes().contains(&b'"') {
        return Ok(line
            .split(delimiter as char)
            .map(|field| field.trim().to_owned())
            .collect());
    }
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(line.as_bytes());
    let record = reader
        .records()
        .next()
        .transpose()
        .map_err(|error| ImportError::new("text_parse_failed", error.to_string()))?
        .ok_or_else(|| ImportError::new("text_parse_failed", "无法拆分字段"))?;
    Ok(record.iter().map(|field| field.trim().to_owned()).collect())
}

fn trim_excess_trailing_empty_fields(fields: &mut Vec<String>, target_width: usize) {
    while fields.len() > target_width && fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
}

fn parse_number(field: &str) -> Option<f64> {
    let trimmed = field.trim();
    if trimmed.is_empty() || matches!(trimmed.to_ascii_lowercase().as_str(), "na" | "none") {
        return Some(f64::NAN);
    }
    trimmed.parse().ok()
}

fn is_number(field: &str) -> bool {
    parse_number(field).is_some()
}

fn clean_header(header: &str) -> String {
    header.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unique_column_names(headers: Vec<String>) -> Vec<String> {
    let mut used = HashSet::new();
    headers
        .into_iter()
        .map(|name| {
            if used.insert(name.clone()) {
                return name;
            }
            let mut duplicate_number = 2;
            loop {
                let candidate = format!("{name} [{duplicate_number}]");
                if used.insert(candidate.clone()) {
                    return candidate;
                }
                duplicate_number += 1;
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{DataSetKind, ImportError, read_data_bytes, read_data_file};
    use rust_xlsxwriter::Workbook;
    use std::path::{Path, PathBuf};

    fn parse(content: &[u8]) -> Result<super::DataSet, ImportError> {
        read_data_bytes(Path::new("sample.txt"), content)
    }

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("instplot-lite-{}-{name}", std::process::id()))
    }

    #[test]
    fn malformed_text_bytes_return_errors_instead_of_panicking() {
        let mut state = 0x4d59_5df4_d0f3_3173_u64;
        for length in 0..256 {
            let mut bytes = Vec::with_capacity(length);
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                bytes.push((state >> 32) as u8);
            }
            let _ = read_data_bytes(Path::new("malformed.csv"), &bytes);
        }
        for bytes in [
            &[0xff, 0xfe, 0x00][..],
            &[0xfe, 0xff, 0x00][..],
            b"# -----BEGIN INSTPLOT DATA-----\n1,2",
            b"x,y\n1,2,3\n",
        ] {
            let _ = read_data_bytes(Path::new("malformed.csv"), bytes);
        }
    }

    #[test]
    fn tsv_extension_uses_the_existing_strict_text_parser() {
        let path = temporary_path("extension.tsv");
        std::fs::write(&path, b"x\ty\n1\t2\n3\t4\n").unwrap();
        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].columns[1].values, [2.0, 4.0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn instplot_begin_end_sections_import_as_independent_datasets() {
        let path = temporary_path("sections.csv");
        std::fs::write(
            &path,
            b"# -----BEGIN INSTPLOT DATA-----\n# Name: original\n# Type: source\nx,y\n1,2\n3,4\n# -----END INSTPLOT DATA-----\n\n# -----BEGIN INSTPLOT DATA-----\n# Name: fitted\n# Type: fit\nX,Y,R2\n1,2.1,0.99\n3,3.9,0.99\n# -----END INSTPLOT DATA-----\n",
        )
        .unwrap();

        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets.len(), 2);
        assert!(datasets[0].display_name().contains("original"));
        assert!(datasets[1].display_name().contains("fitted"));
        assert_eq!(datasets[0].kind, DataSetKind::Source);
        assert_eq!(datasets[1].kind, DataSetKind::Fit);
        assert_eq!(datasets[0].columns[1].values, [2.0, 4.0]);
        assert_eq!(datasets[1].columns[1].values, [2.1, 3.9]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn sectioned_file_rejects_duplicate_source_dataset_ids() {
        let path = temporary_path("duplicate-dataset-id.csv");
        std::fs::write(
            &path,
            b"# -----BEGIN INSTPLOT DATA-----\n# Type: source\n# Dataset-ID: repeated\nx,y\n1,2\n# -----END INSTPLOT DATA-----\n# -----BEGIN INSTPLOT DATA-----\n# Type: source\n# Dataset-ID: repeated\nx,y\n3,4\n# -----END INSTPLOT DATA-----\n",
        )
        .unwrap();
        let error = read_data_file(&path).unwrap_err();
        assert_eq!(error.code, "duplicate_dataset_id");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_or_duplicate_section_type_is_rejected() {
        for (name, metadata, expected_code) in [
            ("invalid", "# Type: other", "invalid_section_type"),
            (
                "duplicate",
                "# Type: source\n# Type: fit",
                "duplicate_section_type",
            ),
        ] {
            let path = temporary_path(&format!("{name}-type.csv"));
            std::fs::write(
                &path,
                format!(
                    "# -----BEGIN INSTPLOT DATA-----\n{metadata}\nx,y\n1,2\n# -----END INSTPLOT DATA-----\n"
                ),
            )
            .unwrap();
            let error = read_data_file(&path).unwrap_err();
            assert_eq!(error.code, expected_code);
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn malformed_instplot_section_reports_the_boundary_problem() {
        let path = temporary_path("unterminated-sections.txt");
        std::fs::write(
            &path,
            b"# -----BEGIN INSTPLOT DATA-----\n# Name: original\nx y\n1 2\n",
        )
        .unwrap();
        let error = read_data_file(&path).unwrap_err();
        assert_eq!(error.code, "unterminated_section");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn sectioned_file_rejects_data_outside_boundaries() {
        let path = temporary_path("outside-sections.csv");
        std::fs::write(
            &path,
            b"outside_x,outside_y\n100,200\n# -----BEGIN INSTPLOT DATA-----\nx,y\n1,2\n# -----END INSTPLOT DATA-----\n",
        )
        .unwrap();
        let error = read_data_file(&path).unwrap_err();
        assert_eq!(error.code, "data_outside_section");
        assert_eq!(error.line_number, Some(1));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn orphan_section_end_is_rejected() {
        let path = temporary_path("orphan-end.csv");
        std::fs::write(&path, b"# comment\n# -----END INSTPLOT DATA-----\n").unwrap();
        let error = read_data_file(&path).unwrap_err();
        assert_eq!(error.code, "unexpected_section_end");
        assert_eq!(error.line_number, Some(2));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn comments_and_blank_lines_are_allowed_outside_sections() {
        let path = temporary_path("section-comments.tsv");
        std::fs::write(
            &path,
            b"# generated file\n\n# -----BEGIN INSTPLOT DATA-----\nx\ty\n1\t2\n# -----END INSTPLOT DATA-----\n\n# trailing comment\n",
        )
        .unwrap();
        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].columns[1].values, [2.0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn spreadsheet_import_skips_notes_and_keeps_each_numeric_sheet() {
        let path = temporary_path("multiple-sheets.xlsx");
        let mut workbook = Workbook::new();
        let notes = workbook.add_worksheet();
        notes.set_name("Notes").unwrap();
        notes.write_string(0, 0, "operator notes").unwrap();
        for sheet_name in ["Forward", "Reverse"] {
            let sheet = workbook.add_worksheet();
            sheet.set_name(sheet_name).unwrap();
            sheet.write_string(0, 0, "Field").unwrap();
            sheet.write_string(0, 1, "Signal").unwrap();
            sheet.write_number(1, 0, 1.0).unwrap();
            sheet.write_number(1, 1, 2.0).unwrap();
        }
        workbook.save(&path).unwrap();

        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets.len(), 2);
        assert!(datasets[0].display_name().contains("Forward"));
        assert!(datasets[1].display_name().contains("Reverse"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn spreadsheet_import_keeps_leading_row_with_a_missing_value() {
        let path = temporary_path("leading-empty.xlsx");
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet.write_string(0, 0, "x").unwrap();
        sheet.write_string(0, 1, "y").unwrap();
        sheet.write_number(1, 0, 1.0).unwrap();
        sheet.write_number(2, 0, 2.0).unwrap();
        sheet.write_number(2, 1, 3.0).unwrap();
        workbook.save(&path).unwrap();

        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets[0].row_count, 2);
        assert_eq!(datasets[0].columns[0].values, [1.0, 2.0]);
        assert!(datasets[0].columns[1].values[0].is_nan());
        assert_eq!(datasets[0].columns[1].values[1], 3.0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn imports_the_legacy_xls_fixture() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data_io/legacy-sample.xls");
        let datasets = read_data_file(&path).unwrap();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].columns[0].name, "磁场");
        assert_eq!(datasets[0].columns[1].name, "信号");
    }

    #[test]
    fn blank_lines_do_not_shift_header_from_data() {
        let data = parse(b"\n\nField\tSignal\n1\t2\n3\t4\n").unwrap();
        assert_eq!(data.columns[0].name, "Field");
        assert_eq!(data.columns[1].name, "Signal");
        assert_eq!(data.columns[0].values, [1.0, 3.0]);
        assert_eq!(data.columns[1].values, [2.0, 4.0]);
    }

    #[test]
    fn headerless_numeric_text_gets_generated_columns() {
        let data = parse(b"1,2\n3,4\n").unwrap();
        assert_eq!(data.columns[0].name, "Column 1");
        assert_eq!(data.columns[1].name, "Column 2");
    }

    #[test]
    fn strips_trailing_delimiters_without_shifting_columns() {
        let data = parse(b"x,y\n1,2,\n3,4,\n").unwrap();
        assert_eq!(data.columns[0].values, [1.0, 3.0]);
        assert_eq!(data.columns[1].values, [2.0, 4.0]);
    }

    #[test]
    fn strips_a_trailing_delimiter_from_the_header_too() {
        let data = parse(b"Field(mT)\tGrayLevel\t\r\n-1\t20\r\n1\t30\r\n").unwrap();
        assert_eq!(data.columns.len(), 2);
        assert_eq!(data.columns[0].name, "Field(mT)");
        assert_eq!(data.columns[1].values, [20.0, 30.0]);
    }

    #[test]
    fn rejects_real_column_width_mismatch() {
        let error = parse(b"x,y\n1,2,3\n").unwrap_err();
        assert_eq!(error.code, "column_count_mismatch");
        assert_eq!(error.line_number, Some(2));
    }

    #[test]
    fn reads_gbk_header_without_garbled_column_names() {
        let (encoded, _, _) = encoding_rs::GBK.encode("磁场\t磁化强度\n1\t2\n");
        let data = parse(&encoded).unwrap();
        assert_eq!(data.columns[0].name, "磁场");
        assert_eq!(data.columns[1].name, "磁化强度");
    }

    #[test]
    fn twelve_column_gbk_lockin_file_keeps_every_header_value_pair_aligned() {
        let headers = [
            "时间", "温度", "磁场", "1-X", "1-Y", "1-R", "1-Theta", "2-X", "2-Y", "2-R", "2-Theta",
            "Theta",
        ];
        let text = format!(
            "{}\t\r\n{}\t\r\n{}\t\r\n",
            headers.join("\t"),
            (1..=12)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join("\t"),
            (101..=112)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join("\t")
        );
        let (encoded, _, _) = encoding_rs::GBK.encode(&text);
        let data = parse(&encoded).unwrap();

        assert_eq!(data.columns.len(), 12);
        for (index, (column, expected_name)) in data.columns.iter().zip(headers).enumerate() {
            assert_eq!(column.name, expected_name);
            assert_eq!(column.values, [index as f64 + 1.0, index as f64 + 101.0]);
        }
    }

    #[test]
    fn real_utf8_fixture_keeps_chinese_headers_aligned() {
        let data = read_data_bytes(
            Path::new("smoke.csv"),
            include_bytes!("../tests/fixtures/smoke.csv"),
        )
        .unwrap();
        assert_eq!(data.columns[0].name, "磁场 Oe");
        assert_eq!(data.columns[1].name, "信号");
        assert_eq!(data.columns[0].values[0], -3.0);
        assert_eq!(data.columns[1].values[0], -0.14112);
        assert_eq!(data.columns[0].values.len(), data.columns[1].values.len());
    }

    #[test]
    fn quoted_delimiter_stays_inside_one_header_field() {
        let data = parse(b"x,\"signal, raw\"\n1,2\n").unwrap();
        assert_eq!(data.columns[1].name, "signal, raw");
    }

    #[test]
    fn supports_mixed_whitespace_rows() {
        let data = parse(b"x y\n1\t2\n3   4\n").unwrap();
        assert_eq!(data.separator, "制表符");
        assert_eq!(data.row_count, 2);
    }

    #[test]
    fn tab_headers_may_contain_spaces_without_becoming_extra_columns() {
        let data = parse(b"Time (sec)\tField (Oe)\tSignal (V)\n1\t2\t3\n").unwrap();
        assert_eq!(data.columns.len(), 3);
        assert_eq!(data.columns[0].name, "Time (sec)");
        assert_eq!(data.columns[2].name, "Signal (V)");
    }

    #[test]
    fn csv_data_rows_may_include_a_non_numeric_text_column() {
        let data =
            parse(b"index,time_str,timestamp,signal\n1,2026-04-02 07:49:04,1775087344.7,3.5\n")
                .unwrap();
        assert_eq!(data.columns.len(), 3);
        assert_eq!(data.columns[0].name, "index");
        assert_eq!(data.columns[1].name, "timestamp");
        assert_eq!(data.columns[2].name, "signal");
    }

    #[test]
    fn text_import_keeps_leading_row_with_a_missing_value() {
        let data = parse(b"x,y\n1,\n2,3\n").unwrap();
        assert_eq!(data.row_count, 2);
        assert_eq!(data.columns[0].values, [1.0, 2.0]);
        assert!(data.columns[1].values[0].is_nan());
        assert_eq!(data.columns[1].values[1], 3.0);
    }

    #[test]
    fn duplicate_headers_are_made_unique() {
        let data = parse(b"x,x,y\n1,10,2\n2,20,4\n").unwrap();
        assert_eq!(
            data.columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["x", "x [2]", "y"]
        );
        assert_eq!(data.columns[1].values, [10.0, 20.0]);
    }

    #[test]
    fn bracketed_data_section_ignores_numeric_metadata() {
        let data = parse(
            b"[Header]\nBYAPP,VSM,2.0,1.0\n[Data]\nComment,Field (Oe),Moment (emu)\n,1,2\n,3,4\n",
        )
        .unwrap();
        assert_eq!(data.row_count, 2);
        assert_eq!(data.columns[0].name, "Field (Oe)");
        assert_eq!(data.columns[0].values, [1.0, 3.0]);
    }

    #[test]
    fn reports_comment_only_file_as_empty() {
        let error = parse(b"# instrument\n# run 2\n").unwrap_err();
        assert_eq!(error.code, "empty_file");
    }

    #[test]
    fn plot_points_are_bounded_and_finite() {
        let data = parse(b"x,y\n1,2\n3,nan\n5,6\n7,8\n").unwrap();
        let points = data.plot_points(0, 1, 2, None);
        assert!(points.len() <= 2);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn min_max_decimation_preserves_a_narrow_peak() {
        let mut text = String::from("x,y\n");
        for index in 0..100 {
            let y = if index == 51 { 500 } else { 0 };
            text.push_str(&format!("{index},{y}\n"));
        }
        let data = parse(text.as_bytes()).unwrap();
        let points = data.plot_points(0, 1, 10, Some([0.0, 99.0]));
        assert!(points.len() <= 10);
        assert!(points.iter().any(|point| point[1] == 500.0));
    }

    #[test]
    fn deletion_and_restoration_only_change_requested_rows() {
        let mut data = parse(b"x,y\n0,0\n1,1\n2,2\n").unwrap();
        assert_eq!(data.delete_rows(&[1, 99]), [1]);
        assert_eq!(
            data.row_points(0, 1)
                .map(|(row, _)| row)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        data.restore_rows(&[1]);
        assert!(data.alive.iter().all(|alive| *alive));
    }

    #[test]
    fn rectangle_selection_uses_complete_alive_data() {
        let mut data = parse(b"x,y\n0,0\n1,10\n2,2\n").unwrap();
        data.delete_rows(&[0]);
        assert_eq!(data.rows_in_bounds(0, 1, [0.0, 2.0], [1.0, 11.0]), [1, 2]);
    }
}
