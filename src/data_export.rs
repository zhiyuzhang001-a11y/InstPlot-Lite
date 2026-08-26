use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rust_xlsxwriter::Workbook;

use crate::data::DataSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextExportFormat {
    Csv,
    Tsv,
    Txt,
}

impl TextExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Txt => "txt",
        }
    }

    fn delimiter(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv | Self::Txt => b'\t',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportSummary {
    pub dataset_count: usize,
    pub row_count: usize,
}

pub fn suggested_file_stem(dataset: &DataSet) -> String {
    dataset_export_base(dataset)
}

pub fn save_retained_rows(path: &Path, dataset: &DataSet) -> Result<usize, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if extension == "xlsx" {
        return save_workbook(path, std::slice::from_ref(dataset)).map(|summary| summary.row_count);
    }
    let format = match extension.as_str() {
        "csv" => TextExportFormat::Csv,
        "tsv" => TextExportFormat::Tsv,
        "txt" => TextExportFormat::Txt,
        _ => return Err(format!("不支持的数据导出格式：.{extension}")),
    };
    let bytes = encode_retained_rows(dataset, format.delimiter())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(retained_row_count(dataset))
}

pub fn save_all_text(
    directory: &Path,
    datasets: &[DataSet],
    format: TextExportFormat,
) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    let mut reserved_names = HashSet::new();
    let mut row_count = 0_usize;
    for dataset in datasets {
        let base = format!("{}-cleaned", dataset_export_base(dataset));
        let path = unique_text_path(directory, &base, format.extension(), &mut reserved_names);
        let bytes = encode_retained_rows(dataset, format.delimiter())?;
        std::fs::write(&path, bytes).map_err(|error| format!("{}：{error}", path.display()))?;
        row_count += retained_row_count(dataset);
    }
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count,
    })
}

pub fn save_workbook(path: &Path, datasets: &[DataSet]) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    let mut workbook = Workbook::new();
    let mut used_sheet_names = HashSet::new();
    let mut total_rows = 0_usize;

    for dataset in datasets {
        let sheet_name = unique_sheet_name(&dataset_export_base(dataset), &mut used_sheet_names);
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(&sheet_name)
            .map_err(|error| error.to_string())?;
        for (column_index, column) in dataset.columns.iter().enumerate() {
            let column_index =
                u16::try_from(column_index).map_err(|_| "列数超过 XLSX 支持范围".to_owned())?;
            worksheet
                .write_string(0, column_index, &column.name)
                .map_err(|error| error.to_string())?;
        }

        let mut output_row = 1_u32;
        for row_index in 0..dataset.row_count {
            if !dataset.alive.get(row_index).copied().unwrap_or(false) {
                continue;
            }
            for (column_index, column) in dataset.columns.iter().enumerate() {
                let value = column.values.get(row_index).copied().unwrap_or(f64::NAN);
                if value.is_finite() {
                    let column_index = u16::try_from(column_index)
                        .map_err(|_| "列数超过 XLSX 支持范围".to_owned())?;
                    worksheet
                        .write_number(output_row, column_index, value)
                        .map_err(|error| error.to_string())?;
                }
            }
            output_row = output_row
                .checked_add(1)
                .ok_or_else(|| "行数超过 XLSX 支持范围".to_owned())?;
        }
        total_rows += usize::try_from(output_row - 1).unwrap_or(usize::MAX);
    }

    workbook.save(path).map_err(|error| error.to_string())?;
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count: total_rows,
    })
}

fn encode_retained_rows(dataset: &DataSet, delimiter: u8) -> Result<Vec<u8>, String> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    writer
        .write_record(dataset.columns.iter().map(|column| column.name.as_str()))
        .map_err(|error| error.to_string())?;

    let mut fields = Vec::with_capacity(dataset.columns.len());
    for row_index in 0..dataset.row_count {
        if !dataset.alive.get(row_index).copied().unwrap_or(false) {
            continue;
        }
        fields.clear();
        for column in &dataset.columns {
            let value = column.values.get(row_index).copied().unwrap_or(f64::NAN);
            fields.push(if value.is_finite() {
                value.to_string()
            } else {
                String::new()
            });
        }
        writer
            .write_record(&fields)
            .map_err(|error| error.to_string())?;
    }
    writer.into_inner().map_err(|error| error.to_string())
}

fn retained_row_count(dataset: &DataSet) -> usize {
    dataset.alive.iter().filter(|alive| **alive).count()
}

fn dataset_export_base(dataset: &DataSet) -> String {
    let source_name = dataset
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let source_stem = dataset
        .source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("instplot-data");
    let raw = dataset.label.as_deref().map_or_else(
        || source_stem.to_owned(),
        |label| {
            label.strip_prefix(source_name).map_or_else(
                || label.to_owned(),
                |suffix| format!("{source_stem}{suffix}"),
            )
        },
    );
    sanitize_file_component(&raw)
}

fn sanitize_file_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim().trim_matches('.');
    if sanitized.is_empty() {
        "instplot-data".to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn unique_text_path(
    directory: &Path,
    base: &str,
    extension: &str,
    reserved_names: &mut HashSet<String>,
) -> PathBuf {
    for suffix in 1_usize.. {
        let name = if suffix == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base}-{suffix}.{extension}")
        };
        let comparison = name.to_lowercase();
        let path = directory.join(&name);
        if !reserved_names.contains(&comparison) && !path.exists() {
            reserved_names.insert(comparison);
            return path;
        }
    }
    unreachable!("the numeric suffix space is unbounded")
}

fn unique_sheet_name(value: &str, used: &mut HashSet<String>) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| {
            if matches!(character, '[' | ']' | ':' | '*' | '?' | '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('\'');
    let cleaned = if cleaned.is_empty() { "Data" } else { cleaned };
    for suffix in 1_usize.. {
        let suffix_text = if suffix == 1 {
            String::new()
        } else {
            format!("_{suffix}")
        };
        let available = 31_usize.saturating_sub(suffix_text.chars().count());
        let prefix: String = cleaned.chars().take(available).collect();
        let candidate = format!("{prefix}{suffix_text}");
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!("the numeric suffix space is unbounded")
}

#[cfg(test)]
mod tests {
    use super::{TextExportFormat, encode_retained_rows, save_all_text, save_workbook};
    use crate::data::{DataSet, NumericColumn, read_data_file};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn dataset(name: &str) -> DataSet {
        DataSet {
            source: PathBuf::from(name),
            label: None,
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "磁场,Oe".to_owned(),
                    values: vec![1.0, 2.0, 3.0],
                },
                NumericColumn {
                    name: "信号".to_owned(),
                    values: vec![4.0, 5.0, f64::NAN],
                },
            ],
            row_count: 3,
            alive: vec![true, false, true],
        }
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "instplot-lite-{name}-{}-{}",
            std::process::id(),
            NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn csv_export_keeps_headers_aligned_and_omits_deleted_rows() {
        let bytes = encode_retained_rows(&dataset("sample.csv"), b',').unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "\"磁场,Oe\",信号\n1,4\n3,\n");
    }

    #[test]
    fn xlsx_round_trip_preserves_multiple_datasets_and_deleted_rows() {
        let directory = temporary_directory("xlsx-round-trip");
        let path = directory.join("all.xlsx");
        let summary = save_workbook(&path, &[dataset("first.csv"), dataset("second.csv")]).unwrap();
        assert_eq!(summary.dataset_count, 2);
        assert_eq!(summary.row_count, 4);

        let imported = read_data_file(&path).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].columns[0].name, "磁场,Oe");
        assert_eq!(imported[0].columns[0].values, [1.0, 3.0]);
        assert!(imported[0].display_name().contains("first"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn batch_text_export_never_overwrites_existing_or_duplicate_names() {
        let directory = temporary_directory("batch-text");
        std::fs::write(directory.join("same-cleaned.csv"), b"existing").unwrap();
        let summary = save_all_text(
            &directory,
            &[dataset("same.csv"), dataset("same.csv")],
            TextExportFormat::Csv,
        )
        .unwrap();
        assert_eq!(summary.dataset_count, 2);
        assert!(directory.join("same-cleaned-2.csv").is_file());
        assert!(directory.join("same-cleaned-3.csv").is_file());
        assert_eq!(
            std::fs::read(directory.join("same-cleaned.csv")).unwrap(),
            b"existing"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
