use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use rust_xlsxwriter::Workbook;

use crate::data::{DataSet, DataSetKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextExportFormat {
    Csv,
    Tsv,
    Txt,
    Dat,
}

impl TextExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Txt => "txt",
            Self::Dat => "dat",
        }
    }

    fn delimiter(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv | Self::Txt | Self::Dat => b'\t',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportSummary {
    pub dataset_count: usize,
    pub row_count: usize,
}

#[derive(Clone, Copy)]
pub struct FitCurveExport<'a> {
    pub name: &'a str,
    pub points: &'a [[f64; 2]],
    pub r_squared: f64,
    pub parent_dataset_id: Option<&'a str>,
    pub source_x_column: &'a str,
    pub source_y_column: &'a str,
}

pub fn suggested_file_stem(dataset: &DataSet) -> String {
    dataset_export_base(dataset)
}

pub fn save_retained_rows_selected_with_fits(
    path: &Path,
    dataset: &DataSet,
    columns: &[usize],
    fits: &[FitCurveExport<'_>],
) -> Result<usize, String> {
    validate_column_selection(dataset, columns)?;
    validate_fit_compatible_selection(dataset, columns, fits)?;
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if extension == "xlsx" {
        save_workbook_selected_with_fits(path, dataset, columns, fits)?;
        return Ok(retained_row_count(dataset));
    }
    let format = match extension.as_str() {
        "csv" => TextExportFormat::Csv,
        "tsv" => TextExportFormat::Tsv,
        "txt" => TextExportFormat::Txt,
        "dat" => TextExportFormat::Dat,
        _ => return Err(format!("不支持的数据导出格式：.{extension}")),
    };
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    write_text_export(
        &mut writer,
        dataset,
        columns,
        fits,
        format.delimiter(),
        false,
    )?;
    writer.flush().map_err(|error| error.to_string())?;
    Ok(retained_row_count(dataset))
}

pub fn save_workbook_selected_with_fits(
    path: &Path,
    dataset: &DataSet,
    columns: &[usize],
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    validate_column_selection(dataset, columns)?;
    validate_fit_compatible_selection(dataset, columns, fits)?;
    save_workbook_with_columns(path, std::slice::from_ref(dataset), Some(columns), fits)
}

#[cfg(test)]
pub fn save_all_text(
    directory: &Path,
    datasets: &[DataSet],
    format: TextExportFormat,
) -> Result<ExportSummary, String> {
    let fits = vec![Vec::new(); datasets.len()];
    save_all_text_with_fits(directory, datasets, format, &fits)
}

#[cfg(test)]
pub fn save_all_text_with_fits(
    directory: &Path,
    datasets: &[DataSet],
    format: TextExportFormat,
    fits_by_dataset: &[Vec<FitCurveExport<'_>>],
) -> Result<ExportSummary, String> {
    let dataset_refs = datasets.iter().collect::<Vec<_>>();
    save_texts_separate_with_fits(directory, &dataset_refs, format, fits_by_dataset)
}

pub fn save_texts_separate_with_fits(
    directory: &Path,
    datasets: &[&DataSet],
    format: TextExportFormat,
    fits_by_dataset: &[Vec<FitCurveExport<'_>>],
) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    if fits_by_dataset.len() != datasets.len() {
        return Err("拟合结果与数据集数量不匹配".to_owned());
    }
    let mut reserved_names = HashSet::new();
    let mut row_count = 0_usize;
    let preserve_links = datasets
        .iter()
        .any(|dataset| dataset.kind == DataSetKind::Fit && dataset.fit_link.is_some());
    for (dataset, fits) in datasets.iter().zip(fits_by_dataset) {
        let base = format!("{}-cleaned", dataset_export_base(dataset));
        let path = unique_text_path(directory, &base, format.extension(), &mut reserved_names);
        let columns = (0..dataset.columns.len()).collect::<Vec<_>>();
        let file = File::create(&path).map_err(|error| format!("{}：{error}", path.display()))?;
        let mut writer = BufWriter::new(file);
        write_text_export(
            &mut writer,
            dataset,
            &columns,
            fits,
            format.delimiter(),
            preserve_links,
        )
        .map_err(|error| format!("{}：{error}", path.display()))?;
        writer
            .flush()
            .map_err(|error| format!("{}：{error}", path.display()))?;
        row_count += retained_row_count(dataset);
    }
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count,
    })
}

#[cfg(test)]
pub fn save_all_text_combined(
    path: &Path,
    datasets: &[DataSet],
    format: TextExportFormat,
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    let dataset_refs = datasets.iter().collect::<Vec<_>>();
    save_text_combined(path, &dataset_refs, format, fits)
}

pub fn save_text_combined(
    path: &Path,
    datasets: &[&DataSet],
    format: TextExportFormat,
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    let mut row_count = 0_usize;
    let mut needs_separator = false;
    for dataset in datasets
        .iter()
        .filter(|dataset| dataset.kind == DataSetKind::Source)
        .chain(
            datasets
                .iter()
                .filter(|dataset| dataset.kind == DataSetKind::Fit),
        )
    {
        if needs_separator {
            writer.write_all(b"\n").map_err(|error| error.to_string())?;
        }
        let columns = (0..dataset.columns.len()).collect::<Vec<_>>();
        write_source_section(&mut writer, dataset, &columns, format.delimiter())?;
        row_count += retained_row_count(dataset);
        needs_separator = true;
    }
    for fit in fits {
        if needs_separator {
            writer.write_all(b"\n").map_err(|error| error.to_string())?;
        }
        write_fit_section(&mut writer, fit, format.delimiter())?;
        needs_separator = true;
    }
    writer.flush().map_err(|error| error.to_string())?;
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count,
    })
}

#[cfg(test)]
pub fn save_workbook(path: &Path, datasets: &[DataSet]) -> Result<ExportSummary, String> {
    save_workbook_with_columns(path, datasets, None, &[])
}

#[cfg(test)]
pub fn save_workbook_with_fits(
    path: &Path,
    datasets: &[DataSet],
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    let dataset_refs = datasets.iter().collect::<Vec<_>>();
    save_workbook_refs_with_columns(path, &dataset_refs, None, fits)
}

fn save_workbook_with_columns(
    path: &Path,
    datasets: &[DataSet],
    selected_columns: Option<&[usize]>,
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    let dataset_refs = datasets.iter().collect::<Vec<_>>();
    save_workbook_refs_with_columns(path, &dataset_refs, selected_columns, fits)
}

pub fn save_workbook_refs_with_fits(
    path: &Path,
    datasets: &[&DataSet],
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    save_workbook_refs_with_columns(path, datasets, None, fits)
}

fn save_workbook_refs_with_columns(
    path: &Path,
    datasets: &[&DataSet],
    selected_columns: Option<&[usize]>,
    fits: &[FitCurveExport<'_>],
) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    let mut workbook = Workbook::new();
    let mut used_sheet_names = HashSet::new();
    let mut total_rows = 0_usize;

    for dataset in datasets
        .iter()
        .filter(|dataset| dataset.kind == DataSetKind::Source)
        .chain(
            datasets
                .iter()
                .filter(|dataset| dataset.kind == DataSetKind::Fit),
        )
    {
        let columns: Vec<usize> = selected_columns
            .map(|columns| columns.to_vec())
            .unwrap_or_else(|| (0..dataset.columns.len()).collect());
        validate_column_selection(dataset, &columns)?;
        let sheet_name = unique_sheet_name(&dataset_export_base(dataset), &mut used_sheet_names);
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(&sheet_name)
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(0, 0, format!("# Type: {}", dataset.kind.metadata_value()))
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(1, 0, format!("# Dataset-ID: {}", dataset.plot_id))
            .map_err(|error| error.to_string())?;
        let mut header_row = 2_u32;
        if let Some(link) = &dataset.fit_link {
            worksheet
                .write_string(
                    header_row,
                    0,
                    format!(
                        "# Parent-ID: {}",
                        link.parent_dataset_id.as_deref().unwrap_or("*")
                    ),
                )
                .map_err(|error| error.to_string())?;
            worksheet
                .write_string(
                    header_row + 1,
                    0,
                    format!("# Source-X: {}", link.source_x_column),
                )
                .map_err(|error| error.to_string())?;
            worksheet
                .write_string(
                    header_row + 2,
                    0,
                    format!("# Source-Y: {}", link.source_y_column),
                )
                .map_err(|error| error.to_string())?;
            header_row += 3;
        }
        for (output_index, source_index) in columns.iter().copied().enumerate() {
            let column = &dataset.columns[source_index];
            let column_index =
                u16::try_from(output_index).map_err(|_| "列数超过 XLSX 支持范围".to_owned())?;
            worksheet
                .write_string(header_row, column_index, &column.name)
                .map_err(|error| error.to_string())?;
        }

        let first_data_row = header_row + 1;
        let mut output_row = first_data_row;
        for row_index in 0..dataset.row_count {
            if !dataset.alive.get(row_index).copied().unwrap_or(false) {
                continue;
            }
            for (output_index, source_index) in columns.iter().copied().enumerate() {
                let column = &dataset.columns[source_index];
                let value = column.values.get(row_index).copied().unwrap_or(f64::NAN);
                if value.is_finite() {
                    let column_index = u16::try_from(output_index)
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
        total_rows += usize::try_from(output_row - first_data_row).unwrap_or(usize::MAX);
    }

    for fit in fits {
        let sheet_name = unique_sheet_name(fit.name, &mut used_sheet_names);
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(&sheet_name)
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(0, 0, "# Type: fit")
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(
                1,
                0,
                format!("# Parent-ID: {}", fit.parent_dataset_id.unwrap_or("*")),
            )
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(2, 0, format!("# Source-X: {}", fit.source_x_column))
            .map_err(|error| error.to_string())?;
        worksheet
            .write_string(3, 0, format!("# Source-Y: {}", fit.source_y_column))
            .map_err(|error| error.to_string())?;
        for (column, header) in ["X", "拟合 Y", "R²"].into_iter().enumerate() {
            worksheet
                .write_string(4, column as u16, header)
                .map_err(|error| error.to_string())?;
        }
        for (row, [x, y]) in fit.points.iter().enumerate() {
            let row = u32::try_from(row + 5).map_err(|_| "行数超过 XLSX 支持范围".to_owned())?;
            worksheet
                .write_number(row, 0, *x)
                .map_err(|error| error.to_string())?;
            worksheet
                .write_number(row, 1, *y)
                .map_err(|error| error.to_string())?;
            worksheet
                .write_number(row, 2, fit.r_squared)
                .map_err(|error| error.to_string())?;
        }
        total_rows += fit.points.len();
    }

    workbook.save(path).map_err(|error| error.to_string())?;
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count: total_rows,
    })
}

pub fn save_workbooks_separate_with_fits(
    directory: &Path,
    datasets: &[&DataSet],
    fits_by_dataset: &[Vec<FitCurveExport<'_>>],
) -> Result<ExportSummary, String> {
    if datasets.is_empty() {
        return Err("没有可导出的数据集".to_owned());
    }
    if fits_by_dataset.len() != datasets.len() {
        return Err("拟合结果与数据集数量不匹配".to_owned());
    }
    let mut reserved_names = HashSet::new();
    let mut row_count = 0_usize;
    for (dataset, fits) in datasets.iter().zip(fits_by_dataset) {
        let base = format!("{}-cleaned", dataset_export_base(dataset));
        let path = unique_text_path(directory, &base, "xlsx", &mut reserved_names);
        let summary = save_workbook_refs_with_columns(&path, &[*dataset], None, fits)
            .map_err(|error| format!("{}：{error}", path.display()))?;
        row_count += summary.row_count;
    }
    Ok(ExportSummary {
        dataset_count: datasets.len(),
        row_count,
    })
}

#[cfg(test)]
fn encode_retained_rows(dataset: &DataSet, delimiter: u8) -> Result<Vec<u8>, String> {
    let columns: Vec<usize> = (0..dataset.columns.len()).collect();
    encode_retained_rows_selected(dataset, &columns, delimiter)
}

fn write_text_export(
    output: &mut impl Write,
    dataset: &DataSet,
    columns: &[usize],
    fits: &[FitCurveExport<'_>],
    delimiter: u8,
    force_section: bool,
) -> Result<(), String> {
    if !force_section && fits.is_empty() && dataset.kind == DataSetKind::Source {
        return write_retained_rows_selected(output, dataset, columns, delimiter);
    }
    validate_fit_compatible_selection(dataset, columns, fits)?;
    write_source_section(output, dataset, columns, delimiter)?;
    for fit in fits {
        output.write_all(b"\n").map_err(|error| error.to_string())?;
        write_fit_section(output, fit, delimiter)?;
    }
    Ok(())
}

fn write_source_section(
    output: &mut impl Write,
    dataset: &DataSet,
    columns: &[usize],
    delimiter: u8,
) -> Result<(), String> {
    const BEGIN: &str = "# -----BEGIN INSTPLOT DATA-----\n";
    const END: &str = "# -----END INSTPLOT DATA-----\n";
    output
        .write_all(BEGIN.as_bytes())
        .map_err(|error| error.to_string())?;
    output
        .write_all(format!("# Name: {}\n", metadata_text(&dataset.display_name())).as_bytes())
        .map_err(|error| error.to_string())?;
    output
        .write_all(format!("# Type: {}\n", dataset.kind.metadata_value()).as_bytes())
        .map_err(|error| error.to_string())?;
    output
        .write_all(format!("# Dataset-ID: {}\n", metadata_text(&dataset.plot_id)).as_bytes())
        .map_err(|error| error.to_string())?;
    if let Some(link) = &dataset.fit_link {
        output
            .write_all(
                format!(
                    "# Parent-ID: {}\n# Source-X: {}\n# Source-Y: {}\n",
                    link.parent_dataset_id.as_deref().unwrap_or("*"),
                    metadata_text(&link.source_x_column),
                    metadata_text(&link.source_y_column),
                )
                .as_bytes(),
            )
            .map_err(|error| error.to_string())?;
    }
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    write_retained_rows_selected(output, dataset, columns, delimiter)?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    output
        .write_all(END.as_bytes())
        .map_err(|error| error.to_string())
}

fn write_fit_section(
    output: &mut impl Write,
    fit: &FitCurveExport<'_>,
    delimiter: u8,
) -> Result<(), String> {
    const BEGIN: &str = "# -----BEGIN INSTPLOT DATA-----\n";
    const END: &str = "# -----END INSTPLOT DATA-----\n";
    output
        .write_all(BEGIN.as_bytes())
        .map_err(|error| error.to_string())?;
    output
        .write_all(format!("# Name: {}\n", metadata_text(fit.name)).as_bytes())
        .map_err(|error| error.to_string())?;
    output
        .write_all(
            format!(
                "# Type: fit\n# Parent-ID: {}\n# Source-X: {}\n# Source-Y: {}\n\n",
                fit.parent_dataset_id.unwrap_or("*"),
                metadata_text(fit.source_x_column),
                metadata_text(fit.source_y_column),
            )
            .as_bytes(),
        )
        .map_err(|error| error.to_string())?;
    {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(delimiter)
            .from_writer(&mut *output);
        writer
            .write_record(["X", "拟合 Y", "R²"])
            .map_err(|error| error.to_string())?;
        for [x, y] in fit.points {
            writer
                .write_record([x.to_string(), y.to_string(), fit.r_squared.to_string()])
                .map_err(|error| error.to_string())?;
        }
        writer.flush().map_err(|error| error.to_string())?;
    }
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    output
        .write_all(END.as_bytes())
        .map_err(|error| error.to_string())
}

fn metadata_text(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_owned()
}

#[cfg(test)]
fn encode_retained_rows_selected(
    dataset: &DataSet,
    columns: &[usize],
    delimiter: u8,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    write_retained_rows_selected(&mut output, dataset, columns, delimiter)?;
    Ok(output)
}

fn write_retained_rows_selected(
    output: &mut impl Write,
    dataset: &DataSet,
    columns: &[usize],
    delimiter: u8,
) -> Result<(), String> {
    validate_column_selection(dataset, columns)?;
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(output);
    writer
        .write_record(
            columns
                .iter()
                .map(|index| dataset.columns[*index].name.as_str()),
        )
        .map_err(|error| error.to_string())?;

    let mut fields = Vec::with_capacity(columns.len());
    for row_index in 0..dataset.row_count {
        if !dataset.alive.get(row_index).copied().unwrap_or(false) {
            continue;
        }
        fields.clear();
        for column_index in columns {
            let column = &dataset.columns[*column_index];
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
    writer.flush().map_err(|error| error.to_string())
}

fn validate_column_selection(dataset: &DataSet, columns: &[usize]) -> Result<(), String> {
    if columns.is_empty() {
        return Err("请至少选择一列导出".to_owned());
    }
    let mut seen = HashSet::new();
    for &column in columns {
        if column >= dataset.columns.len() {
            return Err("所选列不存在".to_owned());
        }
        if !seen.insert(column) {
            return Err("所选列重复".to_owned());
        }
    }
    Ok(())
}

fn validate_fit_compatible_selection(
    dataset: &DataSet,
    columns: &[usize],
    fits: &[FitCurveExport<'_>],
) -> Result<(), String> {
    if (dataset.kind == DataSetKind::Fit || !fits.is_empty()) && columns.len() < 2 {
        return Err("包含拟合结果时，请至少选择两列原始数据，以便文件能够重新导入".to_owned());
    }
    if dataset.kind == DataSetKind::Fit
        && dataset.fit_link.is_some()
        && (!columns.contains(&0) || !columns.contains(&1))
    {
        return Err("导出关联拟合时必须保留 X 和拟合 Y 两列，以便重新导入后正确叠加".to_owned());
    }
    for fit in fits {
        let x_column = dataset
            .columns
            .iter()
            .position(|column| column.name == fit.source_x_column);
        let y_column = dataset
            .columns
            .iter()
            .position(|column| column.name == fit.source_y_column);
        if x_column.is_none_or(|index| !columns.contains(&index))
            || y_column.is_none_or(|index| !columns.contains(&index))
        {
            return Err(format!(
                "导出拟合“{}”时必须保留原始列“{}”和“{}”，以便重新导入后正确叠加",
                fit.name, fit.source_x_column, fit.source_y_column
            ));
        }
    }
    Ok(())
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
    use super::{
        FitCurveExport, TextExportFormat, encode_retained_rows, encode_retained_rows_selected,
        save_all_text, save_all_text_combined, save_retained_rows_selected_with_fits,
        save_text_combined, save_texts_separate_with_fits, save_workbook, save_workbook_with_fits,
        save_workbooks_separate_with_fits,
    };
    use crate::data::{DataSet, DataSetKind, FitLink, NumericColumn, read_data_file};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn dataset(name: &str) -> DataSet {
        DataSet {
            source: PathBuf::from(name),
            label: None,
            kind: DataSetKind::Source,
            plot_id: format!("test-{name}"),
            fit_link: None,
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
    fn selected_column_export_keeps_requested_order_and_retained_rows() {
        let bytes = encode_retained_rows_selected(&dataset("sample.csv"), &[1], b',').unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "信号\n4\n\"\"\n");
    }

    #[test]
    fn sectioned_csv_round_trip_keeps_original_and_fit_independent() {
        let directory = temporary_directory("sectioned-curves");
        let path = directory.join("combined.csv");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        let written = save_retained_rows_selected_with_fits(
            &path,
            &dataset("sample.csv"),
            &[0, 1],
            &[FitCurveExport {
                name: "sample · 拟合 1",
                points: &points,
                r_squared: 0.98,
                parent_dataset_id: Some("test-sample.csv"),
                source_x_column: "磁场,Oe",
                source_y_column: "信号",
            }],
        )
        .unwrap();
        assert_eq!(written, 2);
        let csv = std::fs::read_to_string(path).unwrap();
        assert_eq!(csv.matches("# -----BEGIN INSTPLOT DATA-----").count(), 2);
        assert!(csv.contains("# Name: sample · 拟合 1"));
        let imported = read_data_file(&directory.join("combined.csv")).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].kind, DataSetKind::Source);
        assert_eq!(imported[1].kind, DataSetKind::Fit);
        assert_eq!(imported[0].plot_id, "test-sample.csv");
        let link = imported[1].fit_link.as_ref().unwrap();
        assert_eq!(link.parent_dataset_id.as_deref(), Some("test-sample.csv"));
        assert_eq!(link.source_x_column, "磁场,Oe");
        assert_eq!(link.source_y_column, "信号");
        assert_eq!(imported[0].columns[0].values, [1.0, 3.0]);
        assert_eq!(imported[1].columns[1].values, [1.0, 3.0]);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sectioned_tabular_text_formats_round_trip() {
        let directory = temporary_directory("sectioned-text-formats");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        for extension in ["tsv", "txt", "dat"] {
            let path = directory.join(format!("combined.{extension}"));
            save_retained_rows_selected_with_fits(
                &path,
                &dataset("sample.csv"),
                &[0, 1],
                &[FitCurveExport {
                    name: "sample fitted",
                    points: &points,
                    r_squared: 0.98,
                    parent_dataset_id: Some("test-sample.csv"),
                    source_x_column: "磁场,Oe",
                    source_y_column: "信号",
                }],
            )
            .unwrap();
            let imported = read_data_file(&path).unwrap();
            assert_eq!(imported.len(), 2, "failed for {extension}");
            assert_eq!(imported[0].kind, DataSetKind::Source);
            assert_eq!(imported[1].kind, DataSetKind::Fit);
            assert_eq!(imported[1].columns[1].values, [1.0, 3.0]);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn separate_text_files_preserve_source_fit_links() {
        let directory = temporary_directory("separate-linked-curves");
        let source = dataset("source.csv");
        let mut fit = dataset("fit.csv");
        fit.kind = DataSetKind::Fit;
        fit.plot_id = "fit-id".to_owned();
        fit.fit_link = Some(FitLink {
            parent_dataset_id: Some(source.plot_id.clone()),
            source_x_column: "磁场,Oe".to_owned(),
            source_y_column: "信号".to_owned(),
        });
        fit.columns[0].name = "X".to_owned();
        fit.columns[1].name = "拟合 Y".to_owned();

        save_texts_separate_with_fits(
            &directory,
            &[&source, &fit],
            TextExportFormat::Csv,
            &[Vec::new(), Vec::new()],
        )
        .unwrap();

        let mut imported = std::fs::read_dir(&directory)
            .unwrap()
            .flat_map(|entry| read_data_file(&entry.unwrap().path()).unwrap())
            .collect::<Vec<_>>();
        imported.sort_by_key(|dataset| dataset.kind == DataSetKind::Fit);
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].plot_id, source.plot_id);
        let link = imported[1].fit_link.as_ref().unwrap();
        assert_eq!(
            link.parent_dataset_id.as_deref(),
            Some(source.plot_id.as_str())
        );
        assert_eq!(link.source_x_column, "磁场,Oe");
        assert_eq!(link.source_y_column, "信号");

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fit_export_rejects_a_single_source_column() {
        let directory = temporary_directory("single-column-fit");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        for extension in ["csv", "xlsx"] {
            let path = directory.join(format!("single.{extension}"));
            let error = save_retained_rows_selected_with_fits(
                &path,
                &dataset("sample.csv"),
                &[0],
                &[FitCurveExport {
                    name: "sample fitted",
                    points: &points,
                    r_squared: 0.98,
                    parent_dataset_id: Some("test-sample.csv"),
                    source_x_column: "磁场,Oe",
                    source_y_column: "信号",
                }],
            )
            .unwrap_err();
            assert!(error.contains("至少选择两列"));
            assert!(!path.exists());
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fit_export_requires_the_linked_source_columns() {
        let directory = temporary_directory("missing-linked-column");
        let path = directory.join("missing-y.csv");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        let mut source = dataset("sample.csv");
        source.columns.push(NumericColumn {
            name: "other".to_owned(),
            values: vec![7.0, 8.0, 9.0],
        });
        let error = save_retained_rows_selected_with_fits(
            &path,
            &source,
            &[0, 2],
            &[FitCurveExport {
                name: "sample fitted",
                points: &points,
                r_squared: 0.98,
                parent_dataset_id: Some("test-sample.csv"),
                source_x_column: "磁场,Oe",
                source_y_column: "信号",
            }],
        )
        .unwrap_err();
        assert!(error.contains("必须保留原始列"));
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn imported_fit_dataset_rejects_single_column_export_without_live_fit() {
        let directory = temporary_directory("single-column-imported-fit");
        let path = directory.join("single.csv");
        let mut fit_dataset = dataset("fit.csv");
        fit_dataset.kind = DataSetKind::Fit;
        let error =
            save_retained_rows_selected_with_fits(&path, &fit_dataset, &[0], &[]).unwrap_err();
        assert!(error.contains("至少选择两列"));
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn combined_text_export_writes_all_sources_before_fits_and_round_trips() {
        let directory = temporary_directory("combined-all");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        for (extension, format) in [
            ("csv", TextExportFormat::Csv),
            ("tsv", TextExportFormat::Tsv),
            ("txt", TextExportFormat::Txt),
            ("dat", TextExportFormat::Dat),
        ] {
            let path = directory.join(format!("all.{extension}"));
            let summary = save_all_text_combined(
                &path,
                &[dataset("first.csv"), dataset("second.csv")],
                format,
                &[FitCurveExport {
                    name: "final fit",
                    points: &points,
                    r_squared: 0.98,
                    parent_dataset_id: Some("test-first.csv"),
                    source_x_column: "磁场,Oe",
                    source_y_column: "信号",
                }],
            )
            .unwrap();
            assert_eq!(summary.dataset_count, 2);
            assert_eq!(summary.row_count, 4);
            let text = std::fs::read_to_string(&path).unwrap();
            let first = text.find("# Name: first.csv").unwrap();
            let second = text.find("# Name: second.csv").unwrap();
            let fit = text.find("# Name: final fit").unwrap();
            assert!(first < second && second < fit);
            let imported = read_data_file(&path).unwrap();
            assert_eq!(imported.len(), 3, "failed for {extension}");
            assert!(imported[0].display_name().contains("first.csv"));
            assert!(imported[1].display_name().contains("second.csv"));
            assert!(imported[2].display_name().contains("final fit"));
            assert_eq!(imported[0].kind, DataSetKind::Source);
            assert_eq!(imported[1].kind, DataSetKind::Source);
            assert_eq!(imported[2].kind, DataSetKind::Fit);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn imported_fit_datasets_remain_typed_and_sort_after_sources() {
        let directory = temporary_directory("typed-dataset-order");
        let text_path = directory.join("typed.csv");
        let workbook_path = directory.join("typed.xlsx");
        let mut imported_fit = dataset("older fit.csv");
        imported_fit.kind = DataSetKind::Fit;
        let source = dataset("source.csv");
        let datasets = [imported_fit, source];

        save_all_text_combined(&text_path, &datasets, TextExportFormat::Csv, &[]).unwrap();
        let text_imported = read_data_file(&text_path).unwrap();
        assert_eq!(text_imported.len(), 2);
        assert!(text_imported[0].display_name().contains("source.csv"));
        assert!(text_imported[1].display_name().contains("older fit.csv"));
        assert_eq!(text_imported[0].kind, DataSetKind::Source);
        assert_eq!(text_imported[1].kind, DataSetKind::Fit);

        save_workbook(&workbook_path, &datasets).unwrap();
        let workbook_imported = read_data_file(&workbook_path).unwrap();
        assert_eq!(workbook_imported.len(), 2);
        assert!(workbook_imported[0].display_name().contains("source"));
        assert!(workbook_imported[1].display_name().contains("older fit"));
        assert_eq!(workbook_imported[0].kind, DataSetKind::Source);
        assert_eq!(workbook_imported[1].kind, DataSetKind::Fit);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn xlsx_appends_fit_as_the_last_sheet_and_round_trips() {
        let directory = temporary_directory("xlsx-fitted-curves");
        let path = directory.join("combined.xlsx");
        let points = [[0.0, 1.0], [1.0, 3.0]];
        save_workbook_with_fits(
            &path,
            &[dataset("sample.csv")],
            &[FitCurveExport {
                name: "sample fitted",
                points: &points,
                r_squared: 0.98,
                parent_dataset_id: Some("test-sample.csv"),
                source_x_column: "磁场,Oe",
                source_y_column: "信号",
            }],
        )
        .unwrap();

        let imported = read_data_file(&path).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[0].kind, DataSetKind::Source);
        assert_eq!(imported[1].kind, DataSetKind::Fit);
        assert_eq!(imported[0].plot_id, "test-sample.csv");
        let link = imported[1].fit_link.as_ref().unwrap();
        assert_eq!(link.parent_dataset_id.as_deref(), Some("test-sample.csv"));
        assert_eq!(link.source_x_column, "磁场,Oe");
        assert_eq!(link.source_y_column, "信号");
        assert!(imported[0].display_name().contains("sample"));
        assert!(imported[1].display_name().contains("sample fitted"));
        assert_eq!(imported[1].columns[0].values, [0.0, 1.0]);
        assert_eq!(imported[1].columns[1].values, [1.0, 3.0]);
        std::fs::remove_dir_all(directory).unwrap();
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
    fn selected_dataset_refs_export_combined_or_as_separate_workbooks() {
        let directory = temporary_directory("selected-refs");
        let first = dataset("first.csv");
        let second = dataset("second.csv");
        let combined_path = directory.join("selected.csv");
        save_text_combined(&combined_path, &[&second], TextExportFormat::Csv, &[]).unwrap();
        let imported = read_data_file(&combined_path).unwrap();
        assert_eq!(imported.len(), 1);
        assert!(imported[0].display_name().contains("second.csv"));

        let workbooks = directory.join("workbooks");
        std::fs::create_dir_all(&workbooks).unwrap();
        let summary = save_workbooks_separate_with_fits(
            &workbooks,
            &[&first, &second],
            &[Vec::new(), Vec::new()],
        )
        .unwrap();
        assert_eq!(summary.dataset_count, 2);
        assert_eq!(
            std::fs::read_dir(&workbooks)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "xlsx"))
                .count(),
            2
        );
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
