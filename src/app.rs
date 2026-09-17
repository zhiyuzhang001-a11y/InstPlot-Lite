use std::path::PathBuf;

use eframe::egui::{self, Color32, PointerButton, Rect, Stroke, StrokeKind};
use egui_plot::{Legend, Line, Plot, PlotPoint, Points};

use crate::{
    data, data_export,
    edit_history::{EditHistory, HistoryEffect},
    fitting::{self, FitMethod},
    fonts, image_export,
    processing::{self, Anchor, ProcessingMetadata, ProcessingOperation},
};

const PLOT_LEFT_GUTTER: f32 = 36.0;
const PLOT_BOTTOM_GUTTER: f32 = 12.0;
const PLOT_EXPORT_TOP_GUTTER: f32 = 8.0;

struct PendingDeletion {
    dataset_index: usize,
    rows: Vec<usize>,
    x_column: usize,
    y_column: usize,
}

struct ProcessingSettings {
    scope: ProcessingScope,
    selected_datasets: Vec<bool>,
    result_mode: ProcessingResultMode,
    fit_min: f64,
    fit_max: f64,
    background_order: usize,
    local_min: f64,
    local_max: f64,
    local_transition: f64,
    local_anchor: Anchor,
    local_strength: f64,
    denoise_window: usize,
    denoise_order: usize,
    denoise_in_range: bool,
    denoise_min: f64,
    denoise_max: f64,
    formula: String,
    formula_a: String,
    formula_b: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessingScope {
    Current,
    Selected,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessingResultMode {
    Overwrite,
    Retain,
}

struct ExportSelection {
    format: DataExportFormat,
    datasets: Vec<bool>,
    layout: ExportLayout,
    column_dataset: Option<usize>,
    columns: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExportLayout {
    Combined,
    Separate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataExportFormat {
    Csv,
    Xlsx,
    Tsv,
    Txt,
    Dat,
}

impl DataExportFormat {
    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Xlsx => "Excel（XLSX）",
            Self::Tsv => "TSV",
            Self::Txt => "TXT",
            Self::Dat => "DAT",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Tsv => "tsv",
            Self::Txt => "txt",
            Self::Dat => "dat",
        }
    }

    fn filter_name(self) -> &'static str {
        match self {
            Self::Csv => "CSV 数据",
            Self::Xlsx => "Excel 工作簿",
            Self::Tsv => "TSV 数据",
            Self::Txt => "TXT 数据",
            Self::Dat => "DAT 数据",
        }
    }

    fn text_format(self) -> Option<data_export::TextExportFormat> {
        match self {
            Self::Csv => Some(data_export::TextExportFormat::Csv),
            Self::Xlsx => None,
            Self::Tsv => Some(data_export::TextExportFormat::Tsv),
            Self::Txt => Some(data_export::TextExportFormat::Txt),
            Self::Dat => Some(data_export::TextExportFormat::Dat),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FitKind {
    Polynomial,
    Exponential,
    Logarithmic,
    Power,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum XUnitConversion {
    None,
    DegreesToRadians,
    RadiansToDegrees,
}

impl XUnitConversion {
    fn convert(self, value: f64) -> f64 {
        match self {
            Self::None => value,
            Self::DegreesToRadians => value.to_radians(),
            Self::RadiansToDegrees => value.to_degrees(),
        }
    }

    fn restore(self, value: f64) -> f64 {
        match self {
            Self::None => value,
            Self::DegreesToRadians => value.to_degrees(),
            Self::RadiansToDegrees => value.to_radians(),
        }
    }
}

struct FitSettings {
    merge_datasets: bool,
    kind: FitKind,
    degree: usize,
    use_x_range: bool,
    x_min: f64,
    x_max: f64,
    use_y_range: bool,
    y_min: f64,
    y_max: f64,
    unit_conversion: XUnitConversion,
    expression: String,
    initial_parameters: String,
    message: String,
}

impl Default for FitSettings {
    fn default() -> Self {
        Self {
            merge_datasets: false,
            kind: FitKind::Polynomial,
            degree: 2,
            use_x_range: false,
            x_min: 0.0,
            x_max: 1.0,
            use_y_range: false,
            y_min: 0.0,
            y_max: 1.0,
            unit_conversion: XUnitConversion::None,
            expression: "a * sin(b * x + c)".to_owned(),
            initial_parameters: "1, 1, 0".to_owned(),
            message: "设置参数后执行拟合；拟合曲线会直接显示在主图中。".to_owned(),
        }
    }
}

struct FitOverlay {
    points: Vec<[f64; 2]>,
    r2: f64,
    name: String,
    target: FitTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FitTarget {
    dataset_index: Option<usize>,
    x_column_name: String,
    y_column_name: String,
}

impl Default for ProcessingSettings {
    fn default() -> Self {
        Self {
            scope: ProcessingScope::Current,
            selected_datasets: Vec::new(),
            result_mode: ProcessingResultMode::Overwrite,
            fit_min: 0.0,
            fit_max: 1.0,
            background_order: 2,
            local_min: 0.0,
            local_max: 1.0,
            local_transition: 0.0,
            local_anchor: Anchor::Left,
            local_strength: 1.0,
            denoise_window: 11,
            denoise_order: 3,
            denoise_in_range: false,
            denoise_min: 0.0,
            denoise_max: 1.0,
            formula: "a * y + b".to_owned(),
            formula_a: "1".to_owned(),
            formula_b: "0".to_owned(),
        }
    }
}

pub struct InstPlotLiteApp {
    demo_points: Vec<[f64; 2]>,
    datasets: Vec<data::DataSet>,
    active_dataset: usize,
    x_column: usize,
    y_column: usize,
    reset_view: bool,
    visible_x_range: Option<[f64; 2]>,
    last_plot_rect: Option<Rect>,
    last_export_rect: Option<Rect>,
    pending_screenshot: Option<PathBuf>,
    startup_screenshot: Option<PathBuf>,
    close_after_screenshot: bool,
    history: EditHistory,
    pending_deletion: Option<PendingDeletion>,
    selection_start: Option<egui::Pos2>,
    selection_current: Option<egui::Pos2>,
    processing_open: bool,
    processing_settings: ProcessingSettings,
    export_selection: Option<ExportSelection>,
    fit_open: bool,
    fit_settings: FitSettings,
    fit_overlays: Vec<FitOverlay>,
    selected_coordinate: Option<[f64; 2]>,
    status: String,
}

impl InstPlotLiteApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        startup_files: Vec<PathBuf>,
        startup_screenshot: Option<PathBuf>,
    ) -> Self {
        fonts::install_interface_fonts(&creation_context.egui_ctx);
        configure_interface_style(&creation_context.egui_ctx);
        let mut app = Self {
            demo_points: demo_curve(512),
            datasets: Vec::new(),
            active_dataset: 0,
            x_column: 0,
            y_column: 1,
            reset_view: false,
            visible_x_range: None,
            last_plot_rect: None,
            last_export_rect: None,
            pending_screenshot: None,
            startup_screenshot,
            close_after_screenshot: false,
            history: EditHistory::default(),
            pending_deletion: None,
            selection_start: None,
            selection_current: None,
            processing_open: false,
            processing_settings: ProcessingSettings::default(),
            export_selection: None,
            fit_open: false,
            fit_settings: FitSettings::default(),
            fit_overlays: Vec::new(),
            selected_coordinate: None,
            status: "打开或拖入数据：TXT、CSV、DAT、TSV、XLSX、XLS".to_owned(),
        };
        if !startup_files.is_empty() {
            app.load_paths(startup_files);
        }
        app
    }

    fn open_files(&mut self) {
        let paths = rfd::FileDialog::new()
            .add_filter("数据文件", &["txt", "csv", "dat", "tsv", "xlsx", "xls"])
            .add_filter("文本数据", &["txt", "csv", "dat", "tsv"])
            .add_filter("Excel 工作簿", &["xlsx", "xls"])
            .pick_files();
        if let Some(paths) = paths {
            self.load_paths(paths);
        }
    }

    fn load_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let had_datasets = !self.datasets.is_empty();
        let previous_column_names = self.datasets.get(self.active_dataset).and_then(|dataset| {
            Some((
                dataset.columns.get(self.x_column)?.name.clone(),
                dataset.columns.get(self.y_column)?.name.clone(),
            ))
        });
        let first_new_dataset = self.datasets.len();
        let mut loaded_files = 0_usize;
        let mut loaded_datasets = 0_usize;
        let mut last_summary = String::new();
        let mut errors = Vec::new();
        for path in paths {
            match data::read_data_file(&path) {
                Ok(datasets) => {
                    loaded_files += 1;
                    loaded_datasets += datasets.len();
                    if let Some(dataset) = datasets.last() {
                        last_summary = format!(
                            "{}：{} 行，{} 个数值列，编码 {}，分隔符 {}",
                            dataset.display_name(),
                            dataset.row_count,
                            dataset.columns.len(),
                            dataset.encoding,
                            dataset.separator
                        );
                    }
                    self.datasets.extend(datasets);
                }
                Err(error) => errors.push(format!("{}：{error}", path.display())),
            }
        }
        if loaded_datasets > 0 {
            self.selected_coordinate = None;
            self.active_dataset = first_new_dataset;
            let column_names = self.column_names();
            (self.x_column, self.y_column) =
                preferred_import_columns(&column_names, previous_column_names.as_ref());
            self.reset_view = true;
            self.visible_x_range = None;
        }
        self.status = match (loaded_files, errors.is_empty()) {
            (0, _) => errors.join("；"),
            (_, true) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；{last_summary}{}",
                if had_datasets && previous_column_names.is_some() {
                    "；新文件已优先匹配已有 X/Y 列"
                } else {
                    ""
                }
            ),
            (_, false) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；另有 {} 个失败：{}",
                errors.len(),
                errors.join("；")
            ),
        };
    }

    fn open_data_export(&mut self, format: DataExportFormat) {
        if self.datasets.is_empty() {
            self.status = "请先导入数据".to_owned();
            return;
        }
        let mut selected = vec![false; self.datasets.len()];
        if let Some(value) = selected.get_mut(self.active_dataset) {
            *value = true;
        }
        let columns = self
            .datasets
            .get(self.active_dataset)
            .map(|dataset| vec![true; dataset.columns.len()])
            .unwrap_or_default();
        self.export_selection = Some(ExportSelection {
            format,
            datasets: selected,
            layout: ExportLayout::Combined,
            column_dataset: Some(self.active_dataset),
            columns,
        });
    }

    fn show_export_columns_window(&mut self, context: &egui::Context) {
        let Some(settings) = self.export_selection.as_ref() else {
            return;
        };
        let format_label = settings.format.label();
        let dataset_names = self
            .datasets
            .iter()
            .map(data::DataSet::display_name)
            .collect::<Vec<_>>();
        let minimum_columns = (0..self.datasets.len())
            .map(|index| {
                let has_fit = !self.fit_exports_for_dataset(index).is_empty();
                usize::from(has_fit || self.datasets[index].kind == data::DataSetKind::Fit) + 1
            })
            .collect::<Vec<_>>();
        let selected_indices = settings
            .datasets
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index))
            .collect::<Vec<_>>();
        let sole_dataset = (selected_indices.len() == 1).then_some(selected_indices[0]);
        let column_names = sole_dataset
            .and_then(|index| self.datasets.get(index))
            .map(|dataset| {
                dataset
                    .columns
                    .iter()
                    .map(|column| column.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(settings) = self.export_selection.as_mut()
            && settings.column_dataset != sole_dataset
        {
            settings.column_dataset = sole_dataset;
            settings.columns = vec![true; column_names.len()];
        }

        let mut open = true;
        let mut export = false;
        egui::Window::new(format!("导出 {format_label}"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(430.0)
            .default_height(560.0)
            .min_height(360.0)
            .show(context, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(10.0);
                        ui.indent("export-content", |ui| {
                            ui.spacing_mut().item_spacing.y = 9.0;
                            let Some(settings) = self.export_selection.as_mut() else {
                                return;
                            };
                            ui.label("选择要导出的数据集或曲线。");
                            ui.small("实时拟合结果会随其关联数据集一起导出。");
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("当前").clicked() {
                                    settings.datasets.fill(false);
                                    if let Some(value) =
                                        settings.datasets.get_mut(self.active_dataset)
                                    {
                                        *value = true;
                                    }
                                }
                                if ui.button("全选").clicked() {
                                    settings.datasets.fill(true);
                                }
                                if ui.button("全不选").clicked() {
                                    settings.datasets.fill(false);
                                }
                            });
                            egui::ScrollArea::vertical()
                                .max_height(190.0)
                                .show(ui, |ui| {
                                    for (index, name) in dataset_names.iter().enumerate() {
                                        if let Some(selected) = settings.datasets.get_mut(index) {
                                            let marker = if index == self.active_dataset {
                                                "▶ "
                                            } else {
                                                ""
                                            };
                                            ui.checkbox(
                                                selected,
                                                format!("{marker}{}", compact_label(name, 52)),
                                            )
                                            .on_hover_text(name);
                                        }
                                    }
                                });

                            let selected_count = settings
                                .datasets
                                .iter()
                                .filter(|selected| **selected)
                                .count();
                            if selected_count > 1 {
                                ui.separator();
                                ui.label("保存方式");
                                ui.radio_value(
                                    &mut settings.layout,
                                    ExportLayout::Combined,
                                    if settings.format == DataExportFormat::Xlsx {
                                        "同一个工作簿（每个数据集一个工作表）"
                                    } else {
                                        "同一个分区文件（BEGIN/END）"
                                    },
                                );
                                ui.radio_value(
                                    &mut settings.layout,
                                    ExportLayout::Separate,
                                    "多个独立文件",
                                );
                                ui.small("多数据集导出会保留各自全部列和相关拟合结果。");
                            }

                            let sole = settings
                                .datasets
                                .iter()
                                .enumerate()
                                .filter_map(|(index, selected)| selected.then_some(index))
                                .collect::<Vec<_>>();
                            let sole = (sole.len() == 1).then_some(sole[0]);
                            if let Some(dataset_index) = sole {
                                ui.separator();
                                ui.label("选择列");
                                ui.horizontal(|ui| {
                                    if ui.button("全选列").clicked() {
                                        settings.columns.fill(true);
                                    }
                                    if ui.button("全不选列").clicked() {
                                        settings.columns.fill(false);
                                    }
                                });
                                egui::ScrollArea::vertical()
                                    .max_height(180.0)
                                    .show(ui, |ui| {
                                        for (index, name) in column_names.iter().enumerate() {
                                            if let Some(selected) = settings.columns.get_mut(index)
                                            {
                                                ui.checkbox(
                                                    selected,
                                                    format!("{}：{name}", index + 1),
                                                );
                                            }
                                        }
                                    });
                                let selected_columns = settings
                                    .columns
                                    .iter()
                                    .filter(|selected| **selected)
                                    .count();
                                let minimum = minimum_columns[dataset_index];
                                ui.horizontal(|ui| {
                                    ui.label(format!("已选 {selected_columns} 列"));
                                    if ui
                                        .add_enabled(
                                            selected_columns >= minimum,
                                            egui::Button::new("导出"),
                                        )
                                        .on_disabled_hover_text(if minimum == 2 {
                                            "该数据包含拟合结果，至少选择两列才能重新导入"
                                        } else {
                                            "请至少选择一列"
                                        })
                                        .clicked()
                                    {
                                        export = true;
                                    }
                                });
                            } else {
                                ui.separator();
                                if ui
                                    .add_enabled(
                                        selected_count > 0,
                                        egui::Button::new(format!(
                                            "导出已选 {selected_count} 个数据集"
                                        )),
                                    )
                                    .on_disabled_hover_text("请至少选择一个数据集")
                                    .clicked()
                                {
                                    export = true;
                                }
                            }
                        });
                        ui.add_space(10.0);
                    });
            });
        if export {
            let settings = self.export_selection.take().expect("export settings exist");
            self.export_selected_data(settings);
        } else if !open {
            self.export_selection = None;
        }
    }

    fn export_selected_data(&mut self, settings: ExportSelection) {
        let indices = settings
            .datasets
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index))
            .collect::<Vec<_>>();
        if indices.is_empty() {
            self.status = "请至少选择一个数据集".to_owned();
            return;
        }
        if indices.len() == 1 {
            let index = indices[0];
            let dataset = &self.datasets[index];
            let columns = settings
                .columns
                .iter()
                .enumerate()
                .filter_map(|(index, selected)| selected.then_some(index))
                .collect::<Vec<_>>();
            let extension = settings.format.extension();
            let Some(path) = rfd::FileDialog::new()
                .add_filter(settings.format.filter_name(), &[extension])
                .set_file_name(format!(
                    "{}-cleaned.{extension}",
                    data_export::suggested_file_stem(dataset)
                ))
                .save_file()
            else {
                return;
            };
            let fits = self.fit_exports_for_dataset(index);
            let fit_count = fits.len();
            match data_export::save_retained_rows_selected_with_fits(
                &path, dataset, &columns, &fits,
            ) {
                Ok(row_count) => {
                    self.status = format!(
                        "已导出 {row_count} 行、{} 列及 {fit_count} 个拟合数据区：{}",
                        columns.len(),
                        path.display()
                    )
                }
                Err(error) => self.status = format!("数据导出失败：{error}"),
            }
            return;
        }

        let datasets = indices
            .iter()
            .map(|index| &self.datasets[*index])
            .collect::<Vec<_>>();
        let fits_by_dataset = indices
            .iter()
            .map(|index| self.fit_exports_for_dataset(*index))
            .collect::<Vec<_>>();
        let fit_count = fits_by_dataset.iter().map(Vec::len).sum::<usize>();
        if settings.layout == ExportLayout::Separate {
            let Some(directory) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            let result = if let Some(format) = settings.format.text_format() {
                data_export::save_texts_separate_with_fits(
                    &directory,
                    &datasets,
                    format,
                    &fits_by_dataset,
                )
            } else {
                data_export::save_workbooks_separate_with_fits(
                    &directory,
                    &datasets,
                    &fits_by_dataset,
                )
            };
            self.status = match result {
                Ok(summary) => format!(
                    "已导出 {} 个独立文件、共 {} 行及 {fit_count} 个拟合结果：{}",
                    summary.dataset_count,
                    summary.row_count,
                    directory.display()
                ),
                Err(error) => format!("多个文件导出失败：{error}"),
            };
            return;
        }

        let extension = settings.format.extension();
        let Some(path) = rfd::FileDialog::new()
            .add_filter(settings.format.filter_name(), &[extension])
            .set_file_name(format!("instplot-selected-data.{extension}"))
            .save_file()
        else {
            return;
        };
        let fits = self
            .fit_overlays
            .iter()
            .filter(|fit| {
                fit.target
                    .dataset_index
                    .is_none_or(|index| indices.contains(&index))
            })
            .map(|fit| data_export::FitCurveExport {
                name: &fit.name,
                points: &fit.points,
                r_squared: fit.r2,
            })
            .collect::<Vec<_>>();
        let result = if let Some(format) = settings.format.text_format() {
            data_export::save_text_combined(&path, &datasets, format, &fits)
        } else {
            data_export::save_workbook_refs_with_fits(&path, &datasets, &fits)
        };
        self.status = match result {
            Ok(summary) => format!(
                "已导出同一个文件：{} 个数据集、{} 行及 {} 个拟合结果：{}",
                summary.dataset_count,
                summary.row_count,
                fits.len(),
                path.display()
            ),
            Err(error) => format!("合并导出失败：{error}"),
        };
    }

    fn fit_exports_for_dataset(
        &self,
        dataset_index: usize,
    ) -> Vec<data_export::FitCurveExport<'_>> {
        self.fit_overlays
            .iter()
            .filter(|fit| {
                fit.target
                    .dataset_index
                    .is_none_or(|target_index| target_index == dataset_index)
            })
            .map(|fit| data_export::FitCurveExport {
                name: &fit.name,
                points: &fit.points,
                r_squared: fit.r2,
            })
            .collect()
    }

    fn request_plot_png(&mut self, context: &egui::Context) {
        if self.last_export_rect.is_none() {
            self.status = "绘图区尚未准备好，暂时无法导出图片".to_owned();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG 图片", &["png"])
            .set_file_name("instplot-plot.png")
            .save_file()
        else {
            return;
        };
        self.pending_screenshot = Some(with_png_extension(path));
        context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        context.request_repaint();
        self.status = "正在生成绘图区 PNG…".to_owned();
    }

    fn handle_screenshot_result(&mut self, context: &egui::Context) {
        let image = context.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            if self.pending_screenshot.is_some() {
                context.request_repaint_after(std::time::Duration::from_millis(50));
            }
            return;
        };
        let (Some(path), Some(plot_rect)) = (self.pending_screenshot.take(), self.last_export_rect)
        else {
            return;
        };
        match image_export::save_plot_png(&path, &image, plot_rect, context.pixels_per_point()) {
            Ok(()) => self.status = format!("图片已导出：{}", path.display()),
            Err(error) => self.status = format!("图片导出失败：{error}"),
        }
        if self.close_after_screenshot {
            self.close_after_screenshot = false;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn column_names(&self) -> Vec<String> {
        self.datasets
            .get(self.active_dataset)
            .map(|dataset| {
                dataset
                    .columns
                    .iter()
                    .map(|column| column.name.clone())
                    .collect()
            })
            .unwrap_or_else(|| vec!["x".to_owned(), "y".to_owned()])
    }

    fn apply_pending_deletion(&mut self) {
        let Some(pending) = self.pending_deletion.take() else {
            return;
        };
        let Some(dataset) = self.datasets.get_mut(pending.dataset_index) else {
            return;
        };
        let changed = dataset.delete_rows(&pending.rows);
        let count = changed.len();
        self.history.record_delete(pending.dataset_index, changed);
        self.status = format!("已删除 {count} 个点；可使用撤销恢复");
    }

    fn show_delete_confirmation(&mut self, context: &egui::Context) {
        let Some(count) = self
            .pending_deletion
            .as_ref()
            .map(|pending| pending.rows.len())
        else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new("确认删除")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(context, |ui| {
                ui.label(format!("确定删除选中的 {count} 个数据点吗？"));
                ui.horizontal(|ui| {
                    if ui.button("删除").clicked() {
                        confirm = true;
                    }
                    if ui.button("取消").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            self.apply_pending_deletion();
        } else if cancel {
            self.pending_deletion = None;
            self.status = "已取消删除".to_owned();
        }
    }

    fn open_processing_window(&mut self, context: &egui::Context) {
        if self.processing_open {
            focus_viewport(context, processing_viewport_id());
            return;
        }
        self.processing_settings.scope = ProcessingScope::Current;
        self.processing_settings.selected_datasets = (0..self.datasets.len())
            .map(|index| index == self.active_dataset)
            .collect();
        let range = self
            .datasets
            .get(self.active_dataset)
            .and_then(|dataset| dataset.columns.get(self.x_column))
            .and_then(|column| finite_range(&column.values));
        if let Some([minimum, maximum]) = range {
            self.processing_settings.fit_min = minimum;
            self.processing_settings.fit_max = maximum;
            self.processing_settings.local_min = minimum;
            self.processing_settings.local_max = maximum;
            self.processing_settings.denoise_min = minimum;
            self.processing_settings.denoise_max = maximum;
        }
        self.processing_open = true;
    }

    fn apply_processing(&mut self, operation: ProcessingOperation, suffix: &str) {
        let formula_targets_x = match &operation {
            ProcessingOperation::Formula { expression, .. } => {
                match fitting::formula_output_axis(expression) {
                    Ok(fitting::FormulaAxis::X) => true,
                    Ok(fitting::FormulaAxis::Y) => false,
                    Err(error) => {
                        self.status = format!("公式无效：{error}");
                        return;
                    }
                }
            }
            _ => false,
        };
        let Some(active) = self.datasets.get(self.active_dataset) else {
            self.status = "请先导入数据".to_owned();
            return;
        };
        let Some(x_name) = active
            .columns
            .get(self.x_column)
            .map(|column| column.name.clone())
        else {
            self.status = "当前 X 列不存在".to_owned();
            return;
        };
        let Some(y_name) = active
            .columns
            .get(self.y_column)
            .map(|column| column.name.clone())
        else {
            self.status = "当前 Y 列不存在".to_owned();
            return;
        };
        let selected: Vec<usize> = match self.processing_settings.scope {
            ProcessingScope::Current => vec![self.active_dataset],
            ProcessingScope::Selected => self
                .processing_settings
                .selected_datasets
                .iter()
                .enumerate()
                .filter_map(|(index, selected)| selected.then_some(index))
                .collect(),
            ProcessingScope::All => (0..self.datasets.len()).collect(),
        };
        if selected.is_empty() {
            self.status = "请至少选择一条曲线".to_owned();
            return;
        }
        let mut pending = Vec::with_capacity(selected.len());
        for dataset_index in selected {
            let Some(dataset) = self.datasets.get(dataset_index) else {
                continue;
            };
            let find_column = |name: &str, active_index: usize| {
                if dataset_index == self.active_dataset {
                    return Some(active_index);
                }
                let matches: Vec<usize> = dataset
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(index, column)| (column.name == name).then_some(index))
                    .collect();
                if matches.len() == 1 {
                    Some(matches[0])
                } else {
                    None
                }
            };
            let Some(x_column) = find_column(&x_name, self.x_column) else {
                self.status = format!(
                    "处理未执行：{} 的 X 列“{x_name}”缺失或重名",
                    dataset.display_name()
                );
                return;
            };
            let Some(y_column) = find_column(&y_name, self.y_column) else {
                self.status = format!(
                    "处理未执行：{} 的 Y 列“{y_name}”缺失或重名",
                    dataset.display_name()
                );
                return;
            };
            let dataset_operation = processing_operation_with_x(&operation, x_column);
            let source_column = if formula_targets_x {
                x_column
            } else {
                y_column
            };
            let source_name = &dataset.columns[source_column].name;
            let result = match processing::apply_to_dataset(dataset, y_column, &dataset_operation) {
                Ok(result) => result,
                Err(error) => {
                    self.status = format!("处理未执行：{}：{error}", dataset.display_name());
                    return;
                }
            };
            let column_name = unique_column_name(dataset, &format!("{source_name} [{suffix}]"));
            pending.push((
                dataset_index,
                y_column,
                source_column,
                dataset_operation,
                column_name,
                result,
            ));
        }
        let summary = pending
            .first()
            .map(|(_, _, _, _, _, result)| processing_summary(&result.metadata))
            .unwrap_or_default();
        let mut added_columns = Vec::with_capacity(pending.len());
        let mut replaced_columns = Vec::with_capacity(pending.len());
        let mut active_result_column = None;
        for (dataset_index, y_column, source_column, dataset_operation, column_name, result) in
            pending
        {
            let dataset = &mut self.datasets[dataset_index];
            let column_index = if self.processing_settings.result_mode
                == ProcessingResultMode::Overwrite
            {
                let previous_values =
                    std::mem::replace(&mut dataset.columns[source_column].values, result.values);
                replaced_columns.push((
                    dataset_index,
                    source_column,
                    previous_values,
                    y_column,
                    dataset_operation,
                ));
                source_column
            } else {
                let column_index = dataset.columns.len();
                dataset.columns.push(data::NumericColumn {
                    name: column_name,
                    values: result.values,
                });
                added_columns.push((
                    dataset_index,
                    column_index,
                    dataset.columns[column_index].name.clone(),
                    y_column,
                    dataset_operation,
                ));
                column_index
            };
            if dataset_index == self.active_dataset {
                active_result_column = Some(column_index);
            }
        }
        let processed_count = added_columns.len() + replaced_columns.len();
        if self.processing_settings.result_mode == ProcessingResultMode::Overwrite {
            self.history.record_replace_columns(replaced_columns);
        } else {
            self.history.record_add_columns(added_columns);
        }
        if let Some(column_index) = active_result_column {
            if formula_targets_x {
                self.x_column = column_index;
            } else {
                self.y_column = column_index;
            }
        }
        self.reset_view = true;
        self.visible_x_range = None;
        let axis = if formula_targets_x { "X" } else { "Y" };
        let result_text = if self.processing_settings.result_mode == ProcessingResultMode::Overwrite
        {
            "已覆盖原列（可撤销）"
        } else {
            "已保留为派生列"
        };
        self.status = format!("已处理 {processed_count} 条曲线，{axis}：{result_text}；{summary}");
    }

    fn show_processing_window(&mut self, context: &egui::Context) {
        if !self.processing_open {
            return;
        }
        let mut open = true;
        let mut requested: Option<(ProcessingOperation, String)> = None;
        let viewport_id = processing_viewport_id();
        context.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("InstPlot Lite · 数据处理")
                .with_inner_size([570.0, 570.0])
                .with_min_inner_size([520.0, 500.0])
                .with_resizable(true),
            |ui, _class| {
                if ui.ctx().input(|input| input.viewport().close_requested()) {
                    open = false;
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                ui.add_space(12.0);
                ui.indent("processing-content", |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                let dataset_names: Vec<String> = self
                    .datasets
                    .iter()
                    .map(data::DataSet::display_name)
                    .collect();
                ui.horizontal(|ui| {
                    ui.label("结果写入：");
                    ui.selectable_value(
                        &mut self.processing_settings.result_mode,
                        ProcessingResultMode::Overwrite,
                        "覆盖原列",
                    );
                    ui.selectable_value(
                        &mut self.processing_settings.result_mode,
                        ProcessingResultMode::Retain,
                        "保留派生列",
                    );
                });
                ui.small("此设置适用于本窗口全部操作和所有选中曲线；覆盖操作仍可撤销。");
                ui.horizontal(|ui| {
                    ui.label("处理范围：");
                    ui.selectable_value(
                        &mut self.processing_settings.scope,
                        ProcessingScope::Current,
                        "当前曲线",
                    );
                    ui.selectable_value(
                        &mut self.processing_settings.scope,
                        ProcessingScope::Selected,
                        "选择曲线",
                    );
                    ui.selectable_value(
                        &mut self.processing_settings.scope,
                        ProcessingScope::All,
                        "全部曲线",
                    );
                });
                if self.processing_settings.selected_datasets.len() != dataset_names.len() {
                    self.processing_settings.selected_datasets = (0..dataset_names.len())
                        .map(|index| index == self.active_dataset)
                        .collect();
                }
                if self.processing_settings.scope == ProcessingScope::Current {
                    let previous_dataset = self.active_dataset;
                    ui.horizontal(|ui| {
                        ui.label("当前曲线：");
                        egui::ComboBox::from_id_salt("processing-dataset")
                            .width(300.0)
                            .selected_text(
                                dataset_names
                                    .get(self.active_dataset)
                                    .map(String::as_str)
                                    .unwrap_or("未选择"),
                            )
                            .show_ui(ui, |ui| {
                                for (index, name) in dataset_names.iter().enumerate() {
                                    ui.selectable_value(&mut self.active_dataset, index, name);
                                }
                            });
                    });
                    if self.active_dataset != previous_dataset {
                        self.clamp_columns();
                        self.reset_after_coordinate_change();
                    }
                } else if self.processing_settings.scope == ProcessingScope::Selected {
                    ui.horizontal(|ui| {
                        if ui.button("全选").clicked() {
                            self.processing_settings.selected_datasets.fill(true);
                        }
                        if ui.button("全不选").clicked() {
                            self.processing_settings.selected_datasets.fill(false);
                        }
                    });
                    egui::ScrollArea::vertical().max_height(105.0).show(ui, |ui| {
                        for (index, name) in dataset_names.iter().enumerate() {
                            ui.checkbox(
                                &mut self.processing_settings.selected_datasets[index],
                                name,
                            );
                        }
                    });
                } else {
                    ui.small(format!("将对全部 {} 条曲线应用相同处理。", dataset_names.len()));
                }
                ui.label("结果写入方式由上方全局设置决定；批量处理可一次撤销。");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("对称处理").clicked() {
                        requested = Some((ProcessingOperation::Center, "对称".to_owned()));
                    }
                    if ui.button("归一化").clicked() {
                        requested = Some((
                            ProcessingOperation::CenterNormalize { top_n: 20 },
                            "归一化".to_owned(),
                        ));
                    }
                    ui.small("归一化沿用原版：先对称，再取最高 20 个有限值的均值");
                });

                ui.separator();
                ui.strong("去背底（多项式）");
                ui.horizontal(|ui| {
                    ui.label("拟合 X：");
                    ui.add(egui::DragValue::new(&mut self.processing_settings.fit_min));
                    ui.label("至");
                    ui.add(egui::DragValue::new(&mut self.processing_settings.fit_max));
                    ui.label("阶数");
                    ui.add(
                        egui::DragValue::new(&mut self.processing_settings.background_order)
                            .range(0..=5),
                    );
                    if ui.button(egui::RichText::new("执行").strong()).clicked() {
                        let order = self.processing_settings.background_order;
                        requested = Some((
                            ProcessingOperation::PolynomialBackground {
                                x_column: self.x_column,
                                fit_min: self.processing_settings.fit_min,
                                fit_max: self.processing_settings.fit_max,
                                order,
                            },
                            format!("去背底{order}阶"),
                        ));
                    }
                });

                ui.separator();
                ui.strong("局部展平");
                ui.label(
                    egui::RichText::new(
                        "去除指定 X 区间的线性倾斜，使该段接近水平；锚点位置保持不变。",
                    )
                    .weak(),
                );
                ui.horizontal(|ui| {
                    ui.label("X：");
                    ui.add(egui::DragValue::new(
                        &mut self.processing_settings.local_min,
                    ));
                    ui.label("至");
                    ui.add(egui::DragValue::new(
                        &mut self.processing_settings.local_max,
                    ));
                    ui.label("过渡")
                        .on_hover_text("在区间两侧逐渐应用修正，减小边缘折角；0 表示不过渡");
                    ui.add(
                        egui::DragValue::new(&mut self.processing_settings.local_transition)
                            .range(0.0..=f64::INFINITY),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("锚点")
                        .on_hover_text("这个位置的 Y 值保持不变，可选择左端、右端或中心");
                    egui::ComboBox::from_id_salt("local-anchor")
                        .selected_text(anchor_name(self.processing_settings.local_anchor))
                        .show_ui(ui, |ui| {
                            for anchor in [Anchor::Left, Anchor::Right, Anchor::Center] {
                                ui.selectable_value(
                                    &mut self.processing_settings.local_anchor,
                                    anchor,
                                    anchor_name(anchor),
                                );
                            }
                        });
                    ui.label("强度")
                        .on_hover_text("1.0 表示完全去除拟合斜率，0.5 表示修正一半");
                    ui.add(
                        egui::Slider::new(&mut self.processing_settings.local_strength, 0.0..=1.0)
                            .show_value(true),
                    );
                    if ui
                        .button(egui::RichText::new("执行").strong())
                        .on_hover_text("按上方结果写入方式应用局部展平")
                        .clicked()
                    {
                        requested = Some((
                            ProcessingOperation::LocalFlatten {
                                x_column: self.x_column,
                                x1: self.processing_settings.local_min,
                                x2: self.processing_settings.local_max,
                                transition: self.processing_settings.local_transition,
                                anchor: self.processing_settings.local_anchor,
                                strength: self.processing_settings.local_strength,
                            },
                            "局部展平".to_owned(),
                        ));
                    }
                });

                ui.separator();
                ui.strong("Savitzky–Golay 去噪");
                ui.horizontal(|ui| {
                    ui.label("窗口");
                    ui.add(
                        egui::DragValue::new(&mut self.processing_settings.denoise_window)
                            .range(3..=999)
                            .speed(2),
                    );
                    ui.label("阶数");
                    ui.add(
                        egui::DragValue::new(&mut self.processing_settings.denoise_order)
                            .range(0..=9),
                    );
                    ui.checkbox(
                        &mut self.processing_settings.denoise_in_range,
                        "仅处理 X 区间",
                    );
                });
                if self.processing_settings.denoise_in_range {
                    ui.horizontal(|ui| {
                        ui.label("X：");
                        ui.add(egui::DragValue::new(
                            &mut self.processing_settings.denoise_min,
                        ));
                        ui.label("至");
                        ui.add(egui::DragValue::new(
                            &mut self.processing_settings.denoise_max,
                        ));
                    });
                }
                if ui
                    .button(egui::RichText::new("执行去噪").strong())
                    .clicked()
                {
                    let range = self.processing_settings.denoise_in_range.then_some((
                        self.x_column,
                        self.processing_settings.denoise_min,
                        self.processing_settings.denoise_max,
                    ));
                    requested = Some((
                        ProcessingOperation::Denoise {
                            window_length: self.processing_settings.denoise_window,
                            polyorder: self.processing_settings.denoise_order,
                            range,
                        },
                        "去噪".to_owned(),
                    ));
                }

                ui.separator();
                ui.strong("公式计算");
                ui.label(
                    egui::RichText::new(
                        "按行计算：含 x 的公式生成新 X 列，含 y 的公式生成新 Y 列；a、b 是下方参数。",
                    )
                    .weak(),
                );
                ui.horizontal_wrapped(|ui| {
                    for (label, formula) in [
                        ("a × y + b", "a * y + b"),
                        ("y + b", "y + b"),
                        ("a × y", "a * y"),
                        ("−y", "-y"),
                    ] {
                        if ui.button(label).clicked() {
                            self.processing_settings.formula = formula.to_owned();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("公式：");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.processing_settings.formula)
                            .desired_width(360.0)
                            .hint_text("例如：a * y + b"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("a");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.processing_settings.formula_a)
                            .desired_width(95.0)
                            .hint_text("例如：10/11"),
                    );
                    ui.label("b");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.processing_settings.formula_b)
                            .desired_width(95.0)
                            .hint_text("例如：(2+3)/7"),
                    );
                    if ui
                        .button(egui::RichText::new("执行公式").strong())
                        .on_hover_text("根据公式中的 x 或 y，按上方结果写入方式应用公式")
                        .clicked()
                    {
                        let a = match fitting::evaluate_constant_expression(
                            &self.processing_settings.formula_a,
                        ) {
                            Ok(value) => value,
                            Err(error) => {
                                self.status = format!("公式系数 a 无效：{}", error.reason);
                                return;
                            }
                        };
                        let b = match fitting::evaluate_constant_expression(
                            &self.processing_settings.formula_b,
                        ) {
                            Ok(value) => value,
                            Err(error) => {
                                self.status = format!("公式系数 b 无效：{}", error.reason);
                                return;
                            }
                        };
                        requested = Some((
                            ProcessingOperation::Formula {
                                x_column: self.x_column,
                                expression: self.processing_settings.formula.clone(),
                                a,
                                b,
                            },
                            "公式".to_owned(),
                        ));
                    }
                });
                ui.small(
                    "公式和系数支持 + − × ÷ ^、括号、sin、cos、tan、exp、ln/log、sqrt、abs、arctan，以及 pi、e。",
                );
                });
                ui.add_space(12.0);
            });
            },
        );
        self.processing_open = open;
        if let Some((operation, suffix)) = requested {
            self.apply_processing(operation, &suffix);
        }
    }

    fn open_fit_window(&mut self, context: &egui::Context) {
        if self.fit_open {
            focus_viewport(context, fitting_viewport_id());
            return;
        }
        let Some(dataset) = self.datasets.get(self.active_dataset) else {
            self.status = "请先导入数据".to_owned();
            return;
        };
        if let Some(range) = dataset
            .columns
            .get(self.x_column)
            .and_then(|column| finite_range(&column.values))
        {
            self.fit_settings.x_min = range[0];
            self.fit_settings.x_max = range[1];
        }
        if let Some(range) = dataset
            .columns
            .get(self.y_column)
            .and_then(|column| finite_range(&column.values))
        {
            self.fit_settings.y_min = range[0];
            self.fit_settings.y_max = range[1];
        }
        self.fit_settings.unit_conversion = XUnitConversion::None;
        if let Some(name) = dataset
            .columns
            .get(self.x_column)
            .map(|column| column.name.to_lowercase())
            && (name.contains("degree")
                || name.contains("deg")
                || name.contains('°')
                || name.contains('度'))
        {
            self.fit_settings.unit_conversion = XUnitConversion::DegreesToRadians;
        }
        self.fit_open = true;
    }

    fn collect_fit_values(&self) -> Result<(Vec<f64>, Vec<f64>), String> {
        let active = self
            .datasets
            .get(self.active_dataset)
            .ok_or_else(|| "请先导入数据".to_owned())?;
        let x_name = active
            .columns
            .get(self.x_column)
            .map(|column| column.name.as_str())
            .ok_or_else(|| "当前 X 列不存在".to_owned())?;
        let y_name = active
            .columns
            .get(self.y_column)
            .map(|column| column.name.as_str())
            .ok_or_else(|| "当前 Y 列不存在".to_owned())?;
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();
        for (dataset_index, dataset) in self.datasets.iter().enumerate() {
            if !self.fit_settings.merge_datasets && dataset_index != self.active_dataset {
                continue;
            }
            let x_column = if dataset_index == self.active_dataset {
                self.x_column
            } else if let Some(index) = dataset
                .columns
                .iter()
                .position(|column| column.name == x_name)
            {
                index
            } else {
                continue;
            };
            let y_column = if dataset_index == self.active_dataset {
                self.y_column
            } else if let Some(index) = dataset
                .columns
                .iter()
                .position(|column| column.name == y_name)
            {
                index
            } else {
                continue;
            };
            for (_, [raw_x, y]) in dataset.row_points(x_column, y_column) {
                let x = self.fit_settings.unit_conversion.convert(raw_x);
                if self.fit_settings.use_x_range
                    && (x < self.fit_settings.x_min.min(self.fit_settings.x_max)
                        || x > self.fit_settings.x_min.max(self.fit_settings.x_max))
                {
                    continue;
                }
                if self.fit_settings.use_y_range
                    && (y < self.fit_settings.y_min.min(self.fit_settings.y_max)
                        || y > self.fit_settings.y_min.max(self.fit_settings.y_max))
                {
                    continue;
                }
                x_values.push(x);
                y_values.push(y);
            }
        }
        if x_values.len() < 2 {
            return Err("筛选后至少需要两个有效数据点".to_owned());
        }
        Ok((x_values, y_values))
    }

    fn execute_fit(&mut self) {
        let Some(active) = self.datasets.get(self.active_dataset) else {
            self.fit_settings.message = "拟合失败：请先导入数据".to_owned();
            return;
        };
        let Some(x_column_name) = active
            .columns
            .get(self.x_column)
            .map(|column| column.name.clone())
        else {
            self.fit_settings.message = "拟合失败：当前 X 列不存在".to_owned();
            return;
        };
        let Some(y_column_name) = active
            .columns
            .get(self.y_column)
            .map(|column| column.name.clone())
        else {
            self.fit_settings.message = "拟合失败：当前 Y 列不存在".to_owned();
            return;
        };
        let target = FitTarget {
            dataset_index: (!self.fit_settings.merge_datasets).then_some(self.active_dataset),
            x_column_name: x_column_name.clone(),
            y_column_name: y_column_name.clone(),
        };
        let (x, y) = match self.collect_fit_values() {
            Ok(values) => values,
            Err(error) => {
                self.fit_settings.message = format!("拟合失败：{error}");
                return;
            }
        };
        let method = match self.fit_settings.kind {
            FitKind::Polynomial => FitMethod::Polynomial {
                degree: self.fit_settings.degree,
            },
            FitKind::Exponential => FitMethod::Exponential,
            FitKind::Logarithmic => FitMethod::Logarithmic,
            FitKind::Power => FitMethod::Power,
            FitKind::Custom => {
                let parameters = self
                    .fit_settings
                    .initial_parameters
                    .split(',')
                    .map(str::trim)
                    .enumerate()
                    .map(|(index, value)| {
                        if value.is_empty() {
                            return Err(format!("第 {} 个初始参数为空", index + 1));
                        }
                        fitting::evaluate_constant_expression(value).map_err(|error| {
                            format!("第 {} 个初始参数：{}", index + 1, error.reason)
                        })
                    })
                    .collect::<Result<Vec<_>, _>>();
                let parameters = match parameters {
                    Ok(parameters) => parameters,
                    Err(error) => {
                        self.fit_settings.message = format!("拟合失败：{error}");
                        return;
                    }
                };
                FitMethod::Custom {
                    expression: self.fit_settings.expression.clone(),
                    initial_parameters: parameters,
                }
            }
        };
        match fitting::fit_values(&x, &y, &method) {
            Ok(mut result) => {
                for point in &mut result.points {
                    point[0] = self.fit_settings.unit_conversion.restore(point[0]);
                }
                let point_count = x.len();
                self.fit_settings.message = format!(
                    "拟合方程：{}\nR² = {:.6}\n使用点数：{point_count}",
                    result.equation, result.r2
                );
                let source_name = self
                    .datasets
                    .get(self.active_dataset)
                    .map(data::DataSet::display_name)
                    .unwrap_or_else(|| "未命名数据".to_owned());
                let fit_name = format!(
                    "{source_name} · {x_column_name}/{y_column_name} 拟合 · R²={:.4}",
                    result.r2
                );
                let overlay = FitOverlay {
                    points: result.points,
                    r2: result.r2,
                    name: fit_name,
                    target,
                };
                let updated = store_fit_overlay(&mut self.fit_overlays, overlay);
                self.status = format!(
                    "拟合完成：R² = {:.6}，使用 {point_count} 个点；{}，当前保留 {} 条拟合曲线",
                    result.r2,
                    if updated {
                        "已更新当前曲线"
                    } else {
                        "已添加当前曲线"
                    },
                    self.fit_overlays.len()
                );
            }
            Err(error) => {
                self.fit_settings.message = format!("拟合失败：{}", error.reason);
                self.status = format!("拟合失败：{}", error.reason);
            }
        }
    }

    fn show_fit_window(&mut self, context: &egui::Context) {
        if !self.fit_open {
            return;
        }
        let mut open = true;
        let mut execute = false;
        let mut clear = false;
        let viewport_id = fitting_viewport_id();
        context.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("InstPlot Lite · 曲线拟合")
                .with_inner_size([620.0, 520.0])
                .with_min_inner_size([560.0, 470.0])
                .with_resizable(true),
            |ui, _class| {
                if ui.ctx().input(|input| input.viewport().close_requested()) {
                    open = false;
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(12.0);
                        ui.indent("fit-content", |ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;
                            ui.label("使用当前 X/Y 列进行拟合；已删除和非数值数据点不会参与计算。");
                            ui.separator();
                            ui.small(
                                "同一数据集的同一组 X/Y 重新拟合时，会更新原拟合曲线；其他曲线的拟合结果会保留。",
                            );
                            ui.horizontal(|ui| {
                                ui.label("数据源");
                                egui::ComboBox::from_id_salt("fit-source")
                                    .selected_text(if self.fit_settings.merge_datasets {
                                        "全部同名列数据（合并）"
                                    } else {
                                        "当前数据集"
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.fit_settings.merge_datasets,
                                            false,
                                            "当前数据集",
                                        );
                                        ui.selectable_value(
                                            &mut self.fit_settings.merge_datasets,
                                            true,
                                            "全部同名列数据（合并）",
                                        );
                                    });
                                ui.label("X 单位");
                                egui::ComboBox::from_id_salt("fit-unit")
                                    .selected_text(unit_conversion_name(
                                        self.fit_settings.unit_conversion,
                                    ))
                                    .show_ui(ui, |ui| {
                                        for conversion in [
                                            XUnitConversion::None,
                                            XUnitConversion::DegreesToRadians,
                                            XUnitConversion::RadiansToDegrees,
                                        ] {
                                            ui.selectable_value(
                                                &mut self.fit_settings.unit_conversion,
                                                conversion,
                                                unit_conversion_name(conversion),
                                            );
                                        }
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.fit_settings.use_x_range, "限制 X");
                                ui.add_enabled(
                                    self.fit_settings.use_x_range,
                                    egui::DragValue::new(&mut self.fit_settings.x_min),
                                );
                                ui.label("至");
                                ui.add_enabled(
                                    self.fit_settings.use_x_range,
                                    egui::DragValue::new(&mut self.fit_settings.x_max),
                                );
                                ui.checkbox(&mut self.fit_settings.use_y_range, "限制 Y");
                                ui.add_enabled(
                                    self.fit_settings.use_y_range,
                                    egui::DragValue::new(&mut self.fit_settings.y_min),
                                );
                                ui.label("至");
                                ui.add_enabled(
                                    self.fit_settings.use_y_range,
                                    egui::DragValue::new(&mut self.fit_settings.y_max),
                                );
                            });
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("拟合类型");
                                egui::ComboBox::from_id_salt("fit-kind")
                                    .selected_text(fit_kind_name(self.fit_settings.kind))
                                    .show_ui(ui, |ui| {
                                        for kind in [
                                            FitKind::Polynomial,
                                            FitKind::Exponential,
                                            FitKind::Logarithmic,
                                            FitKind::Power,
                                            FitKind::Custom,
                                        ] {
                                            ui.selectable_value(
                                                &mut self.fit_settings.kind,
                                                kind,
                                                fit_kind_name(kind),
                                            );
                                        }
                                    });
                                if self.fit_settings.kind == FitKind::Polynomial {
                                    ui.label("阶数");
                                    ui.add(
                                        egui::DragValue::new(&mut self.fit_settings.degree)
                                            .range(1..=10),
                                    );
                                }
                            });
                            if self.fit_settings.kind == FitKind::Custom {
                                ui.add_space(6.0);
                                editable_fit_field(
                                    ui,
                                    "函数表达式（可编辑）",
                                    "f(x) =",
                                    &mut self.fit_settings.expression,
                                    "例如：a * sin(b * x + c)",
                                );
                                ui.add_space(8.0);
                                editable_fit_field(
                                    ui,
                                    "初始参数（可编辑）",
                                    "a, b, c… =",
                                    &mut self.fit_settings.initial_parameters,
                                    "例如：1, 10/11, (2+3)/7",
                                );
                                ui.small(
                                "参数按 a、b、c、d、e_param、f、g、h 的顺序填写，用逗号分隔；每项可用分数和括号。",
                                );
                                ui.small("支持 + - * / ^、sin、cos、tan、exp、ln/log、sqrt、abs。");
                            }
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui
                                    .button(egui::RichText::new("执行拟合").strong())
                                    .clicked()
                                {
                                    execute = true;
                                }
                                if ui
                                    .add_enabled(
                                        !self.fit_overlays.is_empty(),
                                        egui::Button::new("清除全部拟合曲线"),
                                    )
                                    .clicked()
                                {
                                    clear = true;
                                }
                            });
                            ui.add_space(8.0);
                            ui.label(&self.fit_settings.message);
                        });
                        ui.add_space(12.0);
                    });
            },
        );
        self.fit_open = open;
        if execute {
            self.execute_fit();
            context.request_repaint();
        }
        if clear {
            self.fit_overlays.clear();
            self.fit_settings.message = "已清除全部拟合曲线。".to_owned();
            self.status = "已清除全部拟合曲线".to_owned();
        }
    }

    fn clamp_columns(&mut self) {
        let count = self
            .datasets
            .get(self.active_dataset)
            .map_or(0, |dataset| dataset.columns.len());
        self.x_column = self.x_column.min(count.saturating_sub(1));
        self.y_column = self.y_column.min(count.saturating_sub(1));
    }

    fn desired_sidebar_width(&self, ui: &egui::Ui) -> f32 {
        const MIN_WIDTH: f32 = 180.0;
        const MAX_WIDTH: f32 = 220.0;
        const COMBO_DECORATION_WIDTH: f32 = 52.0;

        let mut labels = Vec::with_capacity(3);
        if let Some(dataset) = self.datasets.get(self.active_dataset) {
            labels.push(format!("当前：{}", dataset.display_name()));
            if let Some(column) = dataset.columns.get(self.x_column) {
                labels.push(column.name.clone());
            }
            if let Some(column) = dataset.columns.get(self.y_column) {
                labels.push(column.name.clone());
            }
        }

        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let color = ui.visuals().text_color();
        let longest_label = labels
            .into_iter()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(label, font_id.clone(), color)
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);

        (longest_label + COMBO_DECORATION_WIDTH).clamp(MIN_WIDTH, MAX_WIDTH)
    }

    fn show_data_controls(&mut self, ui: &mut egui::Ui, vertical: bool) -> Vec<String> {
        if self.datasets.is_empty() {
            ui.label("尚未导入数据");
            return self.column_names();
        }

        let dataset_names: Vec<String> = self
            .datasets
            .iter()
            .map(data::DataSet::display_name)
            .collect();
        let previous_dataset = self.active_dataset;
        let dataset_combo = |ui: &mut egui::Ui, app: &mut Self| {
            let response = egui::ComboBox::from_id_salt("active-dataset")
                .width(if vertical {
                    (ui.available_width() - 10.0).max(100.0)
                } else {
                    150.0
                })
                .selected_text(
                    dataset_names
                        .get(app.active_dataset)
                        .map(|name| format!("当前：{}", compact_label(name, 28)))
                        .unwrap_or_else(|| "未选择".to_owned()),
                )
                .show_ui(ui, |ui| {
                    for (index, name) in dataset_names.iter().enumerate() {
                        ui.selectable_value(
                            &mut app.active_dataset,
                            index,
                            compact_label(name, 52),
                        )
                        .on_hover_text(name);
                    }
                })
                .response;
            if let Some(name) = dataset_names.get(app.active_dataset) {
                response.on_hover_text(name);
            }
        };
        if vertical {
            ui.label("数据集");
            dataset_combo(ui, self);
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label("数据集");
                dataset_combo(ui, self);
            });
        }
        if self.active_dataset != previous_dataset {
            self.clamp_columns();
            self.reset_after_coordinate_change();
        }

        let column_names = self.column_names();
        let previous_columns = (self.x_column, self.y_column);
        let column_combo =
            |ui: &mut egui::Ui, id: &'static str, selected: &mut usize, width: f32| {
                egui::ComboBox::from_id_salt(id)
                    .width(width)
                    .selected_text(
                        column_names
                            .get(*selected)
                            .map(String::as_str)
                            .unwrap_or("未选择"),
                    )
                    .show_ui(ui, |ui| {
                        for (index, name) in column_names.iter().enumerate() {
                            ui.selectable_value(selected, index, name);
                        }
                    });
            };
        if vertical {
            let control_width = (ui.available_width() - 10.0).max(100.0);
            ui.add_space(8.0);
            ui.label("X 列");
            column_combo(ui, "x-column", &mut self.x_column, control_width);
            ui.add_space(6.0);
            ui.label("Y 列");
            column_combo(ui, "y-column", &mut self.y_column, control_width);
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label("X 列");
                column_combo(ui, "x-column", &mut self.x_column, 120.0);
                ui.label("Y 列");
                column_combo(ui, "y-column", &mut self.y_column, 120.0);
                ui.separator();
                ui.label("左键点选/框选删除 · 滚轮缩放 · 右键拖动平移");
            });
        }
        if (self.x_column, self.y_column) != previous_columns {
            self.reset_after_coordinate_change();
            let x_name = column_names
                .get(self.x_column)
                .map(String::as_str)
                .unwrap_or("未选择");
            let y_name = column_names
                .get(self.y_column)
                .map(String::as_str)
                .unwrap_or("未选择");
            self.status = format!("已切换坐标：X = {x_name}，Y = {y_name}");
        }
        column_names
    }

    fn clear_data(&mut self) {
        self.datasets.clear();
        self.active_dataset = 0;
        self.x_column = 0;
        self.y_column = 1;
        self.reset_view = true;
        self.visible_x_range = None;
        self.history.clear();
        self.pending_deletion = None;
        self.processing_open = false;
        self.export_selection = None;
        self.fit_open = false;
        self.fit_overlays.clear();
        self.selected_coordinate = None;
        self.status = "已清空数据".to_owned();
    }

    fn reset_after_coordinate_change(&mut self) {
        self.reset_view = true;
        self.visible_x_range = None;
        self.pending_deletion = None;
        self.selection_start = None;
        self.selection_current = None;
        self.selected_coordinate = None;
    }

    fn undo(&mut self) {
        if let Some(effect) = self.history.undo(&mut self.datasets) {
            self.clamp_columns();
            self.status = match effect {
                HistoryEffect::Rows(count) => format!("已撤销，恢复 {count} 个点"),
                HistoryEffect::Column(name) => format!("已撤销数据处理“{name}”"),
                HistoryEffect::Columns(count) => format!("已撤销 {count} 条曲线的数据处理"),
            };
        }
    }

    fn redo(&mut self) {
        if let Some(effect) = self.history.redo(&mut self.datasets) {
            self.clamp_columns();
            self.status = match effect {
                HistoryEffect::Rows(count) => format!("已重做，删除 {count} 个点"),
                HistoryEffect::Column(name) => format!("已重做数据处理“{name}”"),
                HistoryEffect::Columns(count) => format!("已重做 {count} 条曲线的数据处理"),
            };
        }
    }
}

impl eframe::App for InstPlotLiteApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_screenshot_result(ui.ctx());
        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let redo_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        if ui
            .ctx()
            .input_mut(|input| input.consume_shortcut(&undo_shortcut))
        {
            self.undo();
        }
        if ui
            .ctx()
            .input_mut(|input| input.consume_shortcut(&redo_shortcut))
        {
            self.redo();
        }
        let dropped_paths = ui.ctx().input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if !dropped_paths.is_empty() {
            self.load_paths(dropped_paths);
        }

        ui.horizontal_wrapped(|ui| {
            ui.heading("InstPlot Lite");
            ui.separator();
            if ui
                .button(egui::RichText::new("打开文件").strong())
                .clicked()
            {
                self.open_files();
            }
            if ui.button("导出图片").clicked() {
                self.request_plot_png(ui.ctx());
            }
            ui.add_enabled_ui(!self.datasets.is_empty(), |ui| {
                ui.menu_button(egui::RichText::new("导出数据…").strong(), |ui| {
                    if ui.button("CSV").clicked() {
                        ui.close();
                        self.open_data_export(DataExportFormat::Csv);
                    }
                    if ui.button("Excel（XLSX）").clicked() {
                        ui.close();
                        self.open_data_export(DataExportFormat::Xlsx);
                    }
                    if ui.button("TSV").clicked() {
                        ui.close();
                        self.open_data_export(DataExportFormat::Tsv);
                    }
                    if ui.button("TXT（制表符分隔）").clicked() {
                        ui.close();
                        self.open_data_export(DataExportFormat::Txt);
                    }
                    if ui.button("DAT（制表符分隔）").clicked() {
                        ui.close();
                        self.open_data_export(DataExportFormat::Dat);
                    }
                });
            });
            if ui
                .add_enabled(
                    !self.datasets.is_empty(),
                    egui::Button::new(egui::RichText::new("数据处理").strong()),
                )
                .clicked()
            {
                self.open_processing_window(ui.ctx());
            }
            if ui
                .add_enabled(
                    !self.datasets.is_empty(),
                    egui::Button::new(egui::RichText::new("曲线拟合").strong()),
                )
                .clicked()
            {
                self.open_fit_window(ui.ctx());
            }
            if ui
                .add_enabled(self.history.can_undo(), egui::Button::new("← 撤销"))
                .on_hover_text("撤销最近一次删除或数据处理")
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(self.history.can_redo(), egui::Button::new("重做 →"))
                .on_hover_text("重新执行刚刚撤销的操作")
                .clicked()
            {
                self.redo();
            }
        });
        ui.separator();
        let wide_layout = ui.available_width() >= 820.0;
        let column_names = if wide_layout {
            let sidebar_width = self.desired_sidebar_width(ui);
            egui::Panel::left("data-controls")
                .exact_size(sidebar_width)
                .resizable(false)
                .show(ui, |ui| {
                    ui.heading("数据");
                    ui.separator();
                    let names = self.show_data_controls(ui, true);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("复位视图").clicked() {
                            self.reset_view = true;
                            self.visible_x_range = None;
                        }
                        if ui.button("清空").clicked() {
                            self.clear_data();
                        }
                    });
                    ui.add_space(14.0);
                    ui.separator();
                    ui.label("左键：点选或框选删除");
                    ui.label("滚轮：缩放");
                    ui.label("右键拖动：平移");
                    names
                })
                .inner
        } else {
            let names = self.show_data_controls(ui, false);
            ui.horizontal_wrapped(|ui| {
                if ui.button("复位视图").clicked() {
                    self.reset_view = true;
                    self.visible_x_range = None;
                }
                if ui.button("清空").clicked() {
                    self.clear_data();
                }
            });
            ui.separator();
            names
        };

        egui::Panel::bottom("plot-status")
            .exact_size(34.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&self.status);
                    if let Some([x, y]) = self.selected_coordinate {
                        ui.separator();
                        ui.label(format!("({x}, {y})"));
                    }
                });
            });

        let plot_height = (ui.available_height() - PLOT_BOTTOM_GUTTER).max(220.0);
        let mut plot = Plot::new("main-plot")
            .legend(Legend::default())
            .height(plot_height)
            .allow_zoom(true)
            .allow_scroll(false)
            .allow_drag(true)
            .allow_boxed_zoom(false)
            .pan_pointer_button(PointerButton::Secondary);
        if let Some(label) = column_names.get(self.x_column) {
            plot = plot.x_axis_label(egui::RichText::new(label.clone()).size(17.0).strong());
        }
        if let Some(label) = column_names.get(self.y_column) {
            plot = plot.y_axis_label(egui::RichText::new(label.clone()).size(17.0).strong());
        }
        if self.reset_view {
            plot = plot.reset();
            self.reset_view = false;
        }
        let plot_row = ui.horizontal(|ui| {
            // egui_plot paints the vertical axis title just outside its own
            // plot rectangle, so reserve a real gutter inside the viewport.
            ui.add_space(PLOT_LEFT_GUTTER);
            plot.show(ui, |plot_ui| {
                if plot_ui.response().contains_pointer() {
                    let wheel_delta = plot_ui.ctx().input(|input| input.smooth_scroll_delta.y);
                    if wheel_delta != 0.0 {
                        plot_ui.zoom_bounds_around_hovered(egui::Vec2::splat(wheel_zoom_factor(
                            wheel_delta,
                        )));
                    }
                }
                if self.datasets.is_empty() {
                    let color = series_color(0);
                    plot_ui.line(Line::new("示例曲线", self.demo_points.clone()).color(color));
                    plot_ui.points(
                        Points::new("", self.demo_points.clone())
                            .color(color)
                            .radius(3.5),
                    );
                    return;
                }
                let x_name = column_names.get(self.x_column);
                let y_name = column_names.get(self.y_column);
                for (dataset_index, dataset) in self.datasets.iter().enumerate() {
                    let x_column = x_name.and_then(|name| {
                        dataset
                            .columns
                            .iter()
                            .position(|column| &column.name == name)
                    });
                    let y_column = y_name.and_then(|name| {
                        dataset
                            .columns
                            .iter()
                            .position(|column| &column.name == name)
                    });
                    let (Some(x_column), Some(y_column)) = (x_column, y_column) else {
                        continue;
                    };
                    let point_limit = self
                        .last_plot_rect
                        .map_or(2_000, |rect| (rect.width() as usize * 2).clamp(512, 20_000));
                    let points =
                        dataset.plot_points(x_column, y_column, point_limit, self.visible_x_range);
                    if !points.is_empty() {
                        let color = series_color(dataset_index);
                        let is_active = dataset_index == self.active_dataset;
                        plot_ui.line(
                            Line::new(
                                legend_series_name(&dataset.display_name(), is_active),
                                points.clone(),
                            )
                            .color(color)
                            .width(if is_active { 3.0 } else { 1.2 }),
                        );
                        plot_ui.points(Points::new("", points).color(color).radius(if is_active {
                            4.5
                        } else {
                            2.5
                        }));
                    }
                    if let Some(pending) = self
                        .pending_deletion
                        .as_ref()
                        .filter(|pending| pending.dataset_index == dataset_index)
                    {
                        let highlighted: Vec<[f64; 2]> = pending
                            .rows
                            .iter()
                            .step_by(pending.rows.len().div_ceil(5_000).max(1))
                            .filter_map(|row_index| {
                                let x = dataset
                                    .columns
                                    .get(pending.x_column)?
                                    .values
                                    .get(*row_index)?;
                                let y = dataset
                                    .columns
                                    .get(pending.y_column)?
                                    .values
                                    .get(*row_index)?;
                                (x.is_finite() && y.is_finite()).then_some([*x, *y])
                            })
                            .collect();
                        plot_ui.points(
                            Points::new("待删除", highlighted)
                                .color(Color32::YELLOW)
                                .radius(5.0),
                        );
                    }
                }
                for (fit_index, fit) in self.fit_overlays.iter().enumerate() {
                    let is_active = fit.target.dataset_index == Some(self.active_dataset);
                    plot_ui.line(
                        Line::new(legend_series_name(&fit.name, is_active), fit.points.clone())
                            .color(fit_color(fit_index))
                            .width(if is_active { 3.5 } else { 1.8 }),
                    );
                }
            })
        });
        ui.add_space(PLOT_BOTTOM_GUTTER);
        self.last_export_rect = Some(Rect::from_min_max(
            plot_row.response.rect.min - egui::vec2(0.0, PLOT_EXPORT_TOP_GUTTER),
            plot_row.response.rect.max + egui::vec2(0.0, PLOT_BOTTOM_GUTTER),
        ));
        let response = plot_row.inner;
        self.last_plot_rect = Some(response.response.rect);
        let bounds = response.transform.bounds();
        self.visible_x_range = Some([bounds.min()[0], bounds.max()[0]]);

        if self.pending_deletion.is_none() {
            if response.response.drag_started_by(PointerButton::Primary) {
                self.selected_coordinate = None;
                self.selection_start = response.response.interact_pointer_pos();
                self.selection_current = self.selection_start;
            }
            if response.response.dragged_by(PointerButton::Primary) {
                self.selection_current = response.response.interact_pointer_pos();
            }
            if let (Some(start), Some(current)) = (self.selection_start, self.selection_current) {
                let selection = Rect::from_two_pos(start, current);
                if selection.width() >= 2.0 || selection.height() >= 2.0 {
                    ui.painter().rect_stroke(
                        selection,
                        0.0,
                        Stroke::new(1.5, Color32::YELLOW),
                        StrokeKind::Inside,
                    );
                }
            }
            if response.response.drag_stopped_by(PointerButton::Primary) {
                let start = self.selection_start.take();
                let end = response
                    .response
                    .interact_pointer_pos()
                    .or_else(|| self.selection_current.take());
                self.selection_current = None;
                if let (Some(start), Some(end)) = (start, end)
                    && start.distance(end) >= 4.0
                {
                    let first = response.transform.value_from_position(start);
                    let second = response.transform.value_from_position(end);
                    if let Some(dataset) = self.datasets.get(self.active_dataset) {
                        let rows = dataset.rows_in_bounds(
                            self.x_column,
                            self.y_column,
                            [first.x, second.x],
                            [first.y, second.y],
                        );
                        if rows.is_empty() {
                            self.status = "框选区域内没有可删除的数据点".to_owned();
                        } else {
                            self.pending_deletion = Some(PendingDeletion {
                                dataset_index: self.active_dataset,
                                rows,
                                x_column: self.x_column,
                                y_column: self.y_column,
                            });
                        }
                    }
                }
            } else if response.response.clicked_by(PointerButton::Primary)
                && let Some(pointer) = response.response.interact_pointer_pos()
            {
                let clicked = response.transform.value_from_position(pointer);
                self.selected_coordinate = Some([clicked.x, clicked.y]);
                self.status = "已显示点击位置坐标".to_owned();

                if let Some(dataset) = self.datasets.get(self.active_dataset) {
                    let nearest = dataset
                        .row_points(self.x_column, self.y_column)
                        .filter_map(|(row_index, [x, y])| {
                            let screen = response
                                .transform
                                .position_from_point(&PlotPoint::new(x, y));
                            let distance_sq = screen.distance_sq(pointer);
                            (distance_sq <= 64.0).then_some((row_index, distance_sq))
                        })
                        .min_by(|left, right| left.1.total_cmp(&right.1));
                    if let Some((row_index, _)) = nearest {
                        let x = dataset.columns[self.x_column].values[row_index];
                        let y = dataset.columns[self.y_column].values[row_index];
                        self.selected_coordinate = Some([x, y]);
                        self.pending_deletion = Some(PendingDeletion {
                            dataset_index: self.active_dataset,
                            rows: vec![row_index],
                            x_column: self.x_column,
                            y_column: self.y_column,
                        });
                    }
                }
            }
        }
        if let Some(path) = self.startup_screenshot.take() {
            self.pending_screenshot = Some(with_png_extension(path));
            self.close_after_screenshot = true;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            ui.ctx().request_repaint();
        }
        self.show_delete_confirmation(ui.ctx());
        self.show_processing_window(ui.ctx());
        self.show_export_columns_window(ui.ctx());
        self.show_fit_window(ui.ctx());
    }
}

fn finite_range(values: &[f64]) -> Option<[f64; 2]> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    (minimum.is_finite() && maximum.is_finite()).then_some([minimum, maximum])
}

fn legend_series_name(name: &str, is_active: bool) -> String {
    let name = compact_label(name, 56);
    if is_active {
        format!("▶ {name}")
    } else {
        name
    }
}

fn compact_label(value: &str, maximum_chars: usize) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= maximum_chars || maximum_chars < 5 {
        return value.to_owned();
    }
    let suffix_count = maximum_chars / 3;
    let prefix_count = maximum_chars - suffix_count - 1;
    let prefix = characters[..prefix_count].iter().collect::<String>();
    let suffix = characters[characters.len() - suffix_count..]
        .iter()
        .collect::<String>();
    format!("{prefix}…{suffix}")
}

fn preferred_import_columns(
    column_names: &[String],
    previous: Option<&(String, String)>,
) -> (usize, usize) {
    let default_y = usize::from(column_names.len() > 1);
    let x_column = previous
        .and_then(|(x_name, _)| column_names.iter().position(|name| name == x_name))
        .unwrap_or(0);
    let y_column = previous
        .and_then(|(_, y_name)| column_names.iter().position(|name| name == y_name))
        .unwrap_or_else(|| {
            if default_y != x_column {
                default_y
            } else {
                (0..column_names.len())
                    .find(|index| *index != x_column)
                    .unwrap_or(x_column)
            }
        });
    (x_column, y_column)
}

fn configure_interface_style(context: &egui::Context) {
    // InstPlot Lite is designed as a dark interface. Following the operating
    // system theme here can mix light panels with explicitly dark plot chrome,
    // which also makes labels unreadable on Windows in light mode. Native title
    // bars remain under operating-system control.
    context.options_mut(|options| options.sync_window_theme = false);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};

        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(14.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(16.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(14.0, FontFamily::Monospace),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(22.0, FontFamily::Proportional),
        );
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        style.visuals.selection.bg_fill = Color32::from_gray(78);
        style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        style.visuals.hyperlink_color = Color32::from_gray(210);
        style.visuals.warn_fg_color = Color32::from_gray(220);
        style.visuals.error_fg_color = Color32::WHITE;
        let radius = egui::CornerRadius::same(8);
        style.visuals.widgets.inactive.corner_radius = radius;
        style.visuals.widgets.hovered.corner_radius = radius;
        style.visuals.widgets.active.corner_radius = radius;
        style.visuals.widgets.open.corner_radius = radius;
        style.visuals.window_corner_radius = egui::CornerRadius::same(10);
    });
}

fn unique_column_name(dataset: &data::DataSet, requested: &str) -> String {
    if !dataset
        .columns
        .iter()
        .any(|column| column.name == requested)
    {
        return requested.to_owned();
    }
    for number in 2.. {
        let candidate = format!("{requested} {number}");
        if !dataset
            .columns
            .iter()
            .any(|column| column.name == candidate)
        {
            return candidate;
        }
    }
    unreachable!()
}

fn anchor_name(anchor: Anchor) -> &'static str {
    match anchor {
        Anchor::Left => "左侧",
        Anchor::Right => "右侧",
        Anchor::Center => "中心",
    }
}

fn fit_kind_name(kind: FitKind) -> &'static str {
    match kind {
        FitKind::Polynomial => "多项式",
        FitKind::Exponential => "指数 y=a·exp(bx)",
        FitKind::Logarithmic => "对数 y=a·ln(x)+b",
        FitKind::Power => "幂函数 y=a·x^b",
        FitKind::Custom => "自定义函数",
    }
}

fn editable_fit_field(
    ui: &mut egui::Ui,
    title: &str,
    prefix: &str,
    value: &mut String,
    hint: &str,
) {
    let accent = Color32::from_gray(132);
    egui::Frame::new()
        .fill(Color32::from_gray(31))
        .stroke(Stroke::new(1.5, accent))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .color(Color32::from_gray(232)),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(prefix).strong());
                ui.scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_gray(56);
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.2, accent);
                    ui.visuals_mut().widgets.hovered.bg_stroke =
                        Stroke::new(1.8, Color32::from_gray(184));
                    ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(2.0, Color32::WHITE);
                    let width = ui.available_width().max(180.0);
                    ui.add_sized(
                        [width, 34.0],
                        egui::TextEdit::singleline(value).hint_text(hint),
                    );
                });
            });
        });
}

fn unit_conversion_name(conversion: XUnitConversion) -> &'static str {
    match conversion {
        XUnitConversion::None => "不转换",
        XUnitConversion::DegreesToRadians => "角度 → 弧度",
        XUnitConversion::RadiansToDegrees => "弧度 → 角度",
    }
}

fn processing_operation_with_x(
    operation: &ProcessingOperation,
    x_column: usize,
) -> ProcessingOperation {
    match operation {
        ProcessingOperation::PolynomialBackground {
            fit_min,
            fit_max,
            order,
            ..
        } => ProcessingOperation::PolynomialBackground {
            x_column,
            fit_min: *fit_min,
            fit_max: *fit_max,
            order: *order,
        },
        ProcessingOperation::LocalFlatten {
            x1,
            x2,
            transition,
            anchor,
            strength,
            ..
        } => ProcessingOperation::LocalFlatten {
            x_column,
            x1: *x1,
            x2: *x2,
            transition: *transition,
            anchor: *anchor,
            strength: *strength,
        },
        ProcessingOperation::Denoise {
            window_length,
            polyorder,
            range,
        } => ProcessingOperation::Denoise {
            window_length: *window_length,
            polyorder: *polyorder,
            range: range.map(|(_, x1, x2)| (x_column, x1, x2)),
        },
        ProcessingOperation::Formula {
            expression, a, b, ..
        } => ProcessingOperation::Formula {
            x_column,
            expression: expression.clone(),
            a: *a,
            b: *b,
        },
        _ => operation.clone(),
    }
}

fn processing_summary(metadata: &ProcessingMetadata) -> String {
    match metadata {
        ProcessingMetadata::Center { midpoint } => format!("中点 {midpoint:.6}"),
        ProcessingMetadata::Normalize {
            midpoint,
            scale,
            top_n,
        } => format!("中点 {midpoint:.6}，尺度 {scale:.6}，取 {top_n} 点"),
        ProcessingMetadata::PolynomialBackground { order } => {
            format!("已减去 {order} 阶多项式背底")
        }
        ProcessingMetadata::LocalFlatten { slope, anchor } => {
            format!("斜率 {slope:.6}，{}锚定", anchor_name(*anchor))
        }
        ProcessingMetadata::Denoise {
            window_length,
            polyorder,
        } => format!("窗口 {window_length}，阶数 {polyorder}"),
        ProcessingMetadata::Formula { expression, a, b } => {
            format!("公式 {expression}（a={a:.6}，b={b:.6}）")
        }
    }
}

fn with_png_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        path
    } else {
        path.with_extension("png")
    }
}

fn wheel_zoom_factor(wheel_delta: f32) -> f32 {
    (wheel_delta / 200.0).exp()
}

fn demo_curve(point_count: usize) -> Vec<[f64; 2]> {
    let divisor = point_count.saturating_sub(1).max(1) as f64;
    (0..point_count)
        .map(|index| {
            let x = index as f64 / divisor * std::f64::consts::TAU * 2.0;
            [x, x.sin()]
        })
        .collect()
}

fn series_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(214, 79, 79),
        Color32::from_rgb(76, 145, 222),
        Color32::from_rgb(72, 176, 116),
        Color32::from_rgb(232, 153, 67),
        Color32::from_rgb(161, 112, 214),
        Color32::from_rgb(65, 185, 190),
    ];
    COLORS[index % COLORS.len()]
}

fn fit_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(255, 196, 64),
        Color32::from_rgb(246, 126, 188),
        Color32::from_rgb(143, 220, 220),
        Color32::from_rgb(184, 153, 255),
        Color32::from_rgb(255, 160, 94),
        Color32::from_rgb(166, 223, 105),
    ];
    COLORS[index % COLORS.len()]
}

fn processing_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("instplot-lite-processing")
}

fn fitting_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("instplot-lite-fitting")
}

fn focus_viewport(context: &egui::Context, viewport_id: egui::ViewportId) {
    context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Minimized(false));
    context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
}

fn store_fit_overlay(overlays: &mut Vec<FitOverlay>, overlay: FitOverlay) -> bool {
    if let Some(existing) = overlays
        .iter_mut()
        .find(|existing| existing.target == overlay.target)
    {
        *existing = overlay;
        true
    } else {
        overlays.push(overlay);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FitOverlay, FitTarget, compact_label, configure_interface_style, demo_curve,
        legend_series_name, preferred_import_columns, store_fit_overlay, wheel_zoom_factor,
    };
    use eframe::egui;

    #[test]
    fn interface_style_always_uses_dark_theme() {
        let context = egui::Context::default();
        context.set_theme(egui::Theme::Light);

        configure_interface_style(&context);

        assert_eq!(context.theme(), egui::Theme::Dark);
        assert!(context.global_style().visuals.dark_mode);
        assert!(!context.options(|options| options.sync_window_theme));
    }

    #[test]
    fn mouse_wheel_zoom_uses_conventional_direction() {
        assert!(wheel_zoom_factor(120.0) > 1.0);
        assert!(wheel_zoom_factor(-120.0) < 1.0);
        assert_eq!(wheel_zoom_factor(0.0), 1.0);
    }

    #[test]
    fn demo_curve_has_requested_number_of_finite_points() {
        let points = demo_curve(512);
        assert_eq!(points.len(), 512);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn imported_dataset_prefers_existing_coordinate_column_names() {
        let columns = ["signal", "temperature", "field"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            preferred_import_columns(&columns, Some(&("field".to_owned(), "signal".to_owned())),),
            (2, 0)
        );
    }

    #[test]
    fn imported_dataset_preserves_a_same_column_x_y_choice() {
        let columns = ["signal", "field"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            preferred_import_columns(&columns, Some(&("field".to_owned(), "field".to_owned())),),
            (1, 1)
        );
    }

    #[test]
    fn active_curve_is_explicitly_marked_in_the_legend() {
        assert_eq!(legend_series_name("sample.csv", true), "▶ sample.csv");
        assert_eq!(legend_series_name("other.csv", false), "other.csv");
    }

    #[test]
    fn long_dataset_labels_keep_their_start_and_distinguishing_suffix() {
        assert_eq!(
            compact_label("abcdefghijklmnopqrstuvwxyz", 10),
            "abcdef…xyz"
        );
        assert_eq!(compact_label("short", 10), "short");
    }

    #[test]
    fn refitting_the_same_curve_replaces_only_its_previous_fit() {
        let target = FitTarget {
            dataset_index: Some(0),
            x_column_name: "x".to_owned(),
            y_column_name: "y".to_owned(),
        };
        let other_target = FitTarget {
            dataset_index: Some(1),
            x_column_name: "x".to_owned(),
            y_column_name: "y".to_owned(),
        };
        let mut overlays = vec![
            FitOverlay {
                points: vec![[0.0, 1.0]],
                r2: 0.5,
                name: "old".to_owned(),
                target: target.clone(),
            },
            FitOverlay {
                points: vec![[0.0, 2.0]],
                r2: 0.9,
                name: "other".to_owned(),
                target: other_target,
            },
        ];
        assert!(store_fit_overlay(
            &mut overlays,
            FitOverlay {
                points: vec![[0.0, 3.0]],
                r2: 0.99,
                name: "new".to_owned(),
                target,
            }
        ));
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0].name, "new");
        assert_eq!(overlays[1].name, "other");
    }
}
