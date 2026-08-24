use std::fmt;
use std::path::{Path, PathBuf};

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{GBK, UTF_16BE, UTF_16LE};

const SUPPORTED_EXTENSIONS: &[&str] = &["txt", "csv", "dat"];

#[derive(Clone, Debug)]
pub struct NumericColumn {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct DataSet {
    pub source: PathBuf,
    pub encoding: String,
    pub separator: String,
    pub columns: Vec<NumericColumn>,
    pub row_count: usize,
    pub alive: Vec<bool>,
}

impl DataSet {
    pub fn display_name(&self) -> String {
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
    let maximum = bucket
        .iter()
        .max_by(|left, right| left.1[1].total_cmp(&right.1[1]))
        .expect("a non-empty bucket has a maximum");
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

pub fn read_data_file(path: &Path) -> Result<DataSet, ImportError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(ImportError::new(
            "unsupported_extension",
            format!("仅支持 TXT、CSV 和 DAT，收到 .{extension}"),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| ImportError::new("file_read_failed", error.to_string()))?;
    read_data_bytes(path, &bytes)
}

fn read_data_bytes(path: &Path, bytes: &[u8]) -> Result<DataSet, ImportError> {
    let (text, encoding) = decode_text(bytes)?;
    parse_text(path, &text, encoding)
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

    let matching_header_position = (0..data_position).rev().find(|position| {
        split_fields(significant[*position].1, separator)
            .map(|mut fields| {
                trim_excess_trailing_empty_fields(&mut fields, detected_column_count);
                fields.len() == detected_column_count
                    && fields.iter().any(|field| !is_number(field))
            })
            .unwrap_or(false)
    });
    let adjacent_header_position = data_position.checked_sub(1).filter(|position| {
        split_fields(significant[*position].1, separator)
            .map(|fields| fields.len() >= 2 && fields.iter().any(|field| !is_number(field)))
            .unwrap_or(false)
    });
    let header_position = matching_header_position.or(adjacent_header_position);
    let data_start_position = data_position;
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

    Ok(DataSet {
        source: path.to_path_buf(),
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

#[cfg(test)]
mod tests {
    use super::{ImportError, read_data_bytes};
    use std::path::Path;

    fn parse(content: &[u8]) -> Result<super::DataSet, ImportError> {
        read_data_bytes(Path::new("sample.txt"), content)
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
