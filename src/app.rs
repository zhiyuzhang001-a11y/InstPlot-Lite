use std::path::PathBuf;

use eframe::egui::{self, Rect};

use crate::{
    data, data_export,
    edit_history::{EditHistory, HistoryEffect},
    fitting::{self, FitMethod},
    fonts,
    processing::{self, ProcessingMetadata, ProcessingOperation},
    session::{FitOverlay, FitResultState, FitTarget},
    ui::delete_confirmation::{self, DeleteAction},
    ui::export_window::{
        self, DataExportFormat, ExportAction, ExportLayout, ExportSelection, ExportWindowData,
    },
    ui::fitting_window::{self, FitAction, FitKind, FitScope, FitSettings, XUnitConversion},
    ui::formatting::{anchor_name, compact_label, split_fit_display_equation},
    ui::main_view::{self, MainAction, SidebarAction},
    ui::plot_series::PendingHighlight,
    ui::plot_view::{self, PlotViewInput},
    ui::processing_window::{
        self, ProcessingAction, ProcessingResultMode, ProcessingScope, ProcessingSettings,
    },
    ui::selection::{sole_selected_index, synchronize_selection},
    ui::theme::configure_interface_style,
    ui::tool_window::{
        export_viewport_id, fitting_viewport_id, focus_viewport, processing_viewport_id,
    },
};

mod import;
mod screenshot;

struct PendingDeletion {
    dataset_index: usize,
    rows: Vec<usize>,
    x_column: usize,
    y_column: usize,
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
    fit_results: FitResultState,
    selected_coordinate: Option<[f64; 2]>,
    status: String,
    #[cfg(any(target_os = "windows", test))]
    windows_updater: crate::updater::WindowsUpdater,
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
            fit_results: FitResultState::default(),
            selected_coordinate: None,
            status: "打开或拖入数据：TXT、CSV、DAT、TSV、XLSX、XLS".to_owned(),
            #[cfg(any(target_os = "windows", test))]
            windows_updater: crate::updater::WindowsUpdater::new(creation_context.egui_ctx.clone()),
        };
        if !startup_files.is_empty() {
            app.load_paths(startup_files);
        }
        app
    }

    fn open_data_export(&mut self, context: &egui::Context, format: DataExportFormat) {
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
        focus_viewport(context, export_viewport_id());
    }

    fn show_export_columns_window(&mut self, context: &egui::Context) {
        if let Some(settings) = self.export_selection.as_mut() {
            synchronize_selection(&mut settings.datasets, self.datasets.len(), false);
        }
        let Some(settings) = self.export_selection.as_ref() else {
            return;
        };
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
        let sole_dataset = sole_selected_index(&settings.datasets);
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
        } else if let Some(settings) = self.export_selection.as_mut() {
            synchronize_selection(&mut settings.columns, column_names.len(), true);
        }
        let action = self
            .export_selection
            .as_mut()
            .map_or(ExportAction::None, |settings| {
                export_window::show(
                    context,
                    settings,
                    ExportWindowData {
                        active_dataset: self.active_dataset,
                        dataset_names: &dataset_names,
                        minimum_columns: &minimum_columns,
                        column_names: &column_names,
                    },
                )
            });
        match action {
            ExportAction::Export => {
                if let Some(settings) = self.export_selection.take() {
                    self.export_selected_data(settings);
                }
            }
            ExportAction::Close => self.export_selection = None,
            ExportAction::None => {}
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
        if let Err(error) = self.validate_fit_export_scope(&indices, settings.layout) {
            self.status = format!("数据导出失败：{error}");
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
            let result = if let Some(format) = data_export_text_format(settings.format) {
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
            .fit_results
            .overlays
            .iter()
            .filter(|fit| self.fit_is_fully_selected(fit, &indices))
            .map(|fit| data_export::FitCurveExport {
                name: &fit.name,
                equation: &fit.equation,
                display_equation: &fit.display_equation,
                points: &fit.points,
                r_squared: fit.r2,
                parent_dataset_id: (fit.target.source_dataset_ids.len() == 1)
                    .then(|| fit.target.source_dataset_ids[0].as_str()),
                source_x_column: &fit.target.x_column_name,
                source_y_column: &fit.target.y_column_name,
            })
            .collect::<Vec<_>>();
        let result = if let Some(format) = data_export_text_format(settings.format) {
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
        let Some(dataset) = self.datasets.get(dataset_index) else {
            return Vec::new();
        };
        self.fit_results
            .overlays
            .iter()
            .filter(|fit| {
                fit.target.source_dataset_ids.len() == 1
                    && fit.target.source_dataset_ids[0] == dataset.plot_id
            })
            .map(|fit| data_export::FitCurveExport {
                name: &fit.name,
                equation: &fit.equation,
                display_equation: &fit.display_equation,
                points: &fit.points,
                r_squared: fit.r2,
                parent_dataset_id: Some(fit.target.source_dataset_ids[0].as_str()),
                source_x_column: &fit.target.x_column_name,
                source_y_column: &fit.target.y_column_name,
            })
            .collect()
    }

    fn fit_is_fully_selected(&self, fit: &FitOverlay, selected_indices: &[usize]) -> bool {
        fit.target.source_dataset_ids.iter().all(|source_id| {
            selected_indices
                .iter()
                .filter_map(|index| self.datasets.get(*index))
                .any(|dataset| dataset.plot_id == *source_id)
        })
    }

    fn validate_fit_export_scope(
        &self,
        selected_indices: &[usize],
        layout: ExportLayout,
    ) -> Result<(), String> {
        for fit in self
            .fit_results
            .overlays
            .iter()
            .filter(|fit| fit.target.source_dataset_ids.len() > 1)
        {
            let any_source_selected = fit.target.source_dataset_ids.iter().any(|source_id| {
                selected_indices
                    .iter()
                    .filter_map(|index| self.datasets.get(*index))
                    .any(|dataset| dataset.plot_id == *source_id)
            });
            if any_source_selected && !self.fit_is_fully_selected(fit, selected_indices) {
                return Err(format!(
                    "合并拟合“{}”必须与全部参与的原始曲线一起导出",
                    fit.name
                ));
            }
            if any_source_selected && layout == ExportLayout::Separate {
                return Err(format!(
                    "合并拟合“{}”跨多条曲线，请选择“导出到同一个文件”以保留其来源",
                    fit.name
                ));
            }
        }
        Ok(())
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
        match delete_confirmation::show(context, count) {
            DeleteAction::Confirm => self.apply_pending_deletion(),
            DeleteAction::Cancel => {
                self.pending_deletion = None;
                self.status = "已取消删除".to_owned();
            }
            DeleteAction::None => {}
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
        let dataset_names = self
            .datasets
            .iter()
            .map(data::DataSet::display_name)
            .collect::<Vec<_>>();
        let response = processing_window::show(
            context,
            &mut self.processing_settings,
            &dataset_names,
            self.active_dataset,
        );
        if response.active_dataset != self.active_dataset {
            self.active_dataset = response.active_dataset;
            self.clamp_columns();
            self.reset_after_coordinate_change();
        }
        self.processing_open = response.open;
        let Some(action) = response.action else {
            return;
        };
        let (operation, suffix) = match action {
            ProcessingAction::Center => (ProcessingOperation::Center, "对称".to_owned()),
            ProcessingAction::CenterNormalize => (
                ProcessingOperation::CenterNormalize { top_n: 20 },
                "归一化".to_owned(),
            ),
            ProcessingAction::PolynomialBackground {
                fit_min,
                fit_max,
                order,
            } => (
                ProcessingOperation::PolynomialBackground {
                    x_column: self.x_column,
                    fit_min,
                    fit_max,
                    order,
                },
                format!("去背底{order}阶"),
            ),
            ProcessingAction::LocalFlatten {
                x1,
                x2,
                transition,
                anchor,
                strength,
            } => (
                ProcessingOperation::LocalFlatten {
                    x_column: self.x_column,
                    x1,
                    x2,
                    transition,
                    anchor,
                    strength,
                },
                "局部展平".to_owned(),
            ),
            ProcessingAction::Denoise {
                window_length,
                polyorder,
                range,
            } => (
                ProcessingOperation::Denoise {
                    window_length,
                    polyorder,
                    range: range.map(|(minimum, maximum)| (self.x_column, minimum, maximum)),
                },
                "去噪".to_owned(),
            ),
            ProcessingAction::Formula { expression, a, b } => {
                let a = match fitting::evaluate_constant_expression(&a) {
                    Ok(value) => value,
                    Err(error) => {
                        self.status = format!("公式系数 a 无效：{}", error.reason);
                        return;
                    }
                };
                let b = match fitting::evaluate_constant_expression(&b) {
                    Ok(value) => value,
                    Err(error) => {
                        self.status = format!("公式系数 b 无效：{}", error.reason);
                        return;
                    }
                };
                (
                    ProcessingOperation::Formula {
                        x_column: self.x_column,
                        expression,
                        a,
                        b,
                    },
                    "公式".to_owned(),
                )
            }
        };
        self.apply_processing(operation, &suffix);
    }

    fn open_fit_window(&mut self, context: &egui::Context) {
        if self.fit_open {
            focus_viewport(context, fitting_viewport_id());
            return;
        }
        if self.datasets.get(self.active_dataset).is_none() {
            self.status = "请先导入数据".to_owned();
            return;
        }
        self.refresh_fit_defaults_from_active_dataset();
        self.fit_settings.scope = FitScope::Current;
        self.fit_settings.selected_datasets = (0..self.datasets.len())
            .map(|index| index == self.active_dataset)
            .collect();
        self.fit_open = true;
    }

    fn refresh_fit_defaults_from_active_dataset(&mut self) {
        let Some(dataset) = self.datasets.get(self.active_dataset) else {
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
    }

    fn collect_fit_values(
        &self,
        dataset_index: usize,
        x_column: usize,
        y_column: usize,
    ) -> Result<(Vec<f64>, Vec<f64>), String> {
        let dataset = self
            .datasets
            .get(dataset_index)
            .ok_or_else(|| "曲线不存在".to_owned())?;
        let mut x_values = Vec::new();
        let mut y_values = Vec::new();
        for (_, [raw_x, y]) in dataset.row_points(x_column, y_column) {
            if self.fit_settings.use_x_range
                && !is_inside_range(raw_x, self.fit_settings.x_min, self.fit_settings.x_max)
            {
                continue;
            }
            if self.fit_settings.use_y_range
                && !is_inside_range(y, self.fit_settings.y_min, self.fit_settings.y_max)
            {
                continue;
            }
            x_values.push(self.fit_settings.unit_conversion.convert(raw_x));
            y_values.push(y);
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
        let selected = fit_dataset_indices(
            self.fit_settings.scope,
            &self.fit_settings.selected_datasets,
            self.active_dataset,
            self.datasets.len(),
        );
        if selected.is_empty() {
            self.fit_settings.message = "拟合失败：请至少选择一条曲线".to_owned();
            self.status = "拟合失败：请至少选择一条曲线".to_owned();
            return;
        }

        let mut pending = Vec::with_capacity(selected.len());
        let mut summaries = Vec::with_capacity(selected.len());
        for dataset_index in selected {
            let Some(dataset) = self.datasets.get(dataset_index) else {
                continue;
            };
            let find_column = |name: &str, active_index: usize| {
                if dataset_index == self.active_dataset {
                    return Some(active_index);
                }
                let matches = dataset
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(index, column)| (column.name == name).then_some(index))
                    .collect::<Vec<_>>();
                (matches.len() == 1).then_some(matches[0])
            };
            let Some(x_column) = find_column(&x_column_name, self.x_column) else {
                self.fit_settings.message = format!(
                    "拟合未执行：{} 的 X 列“{x_column_name}”缺失或重名",
                    dataset.display_name()
                );
                self.status = self.fit_settings.message.clone();
                return;
            };
            let Some(y_column) = find_column(&y_column_name, self.y_column) else {
                self.fit_settings.message = format!(
                    "拟合未执行：{} 的 Y 列“{y_column_name}”缺失或重名",
                    dataset.display_name()
                );
                self.status = self.fit_settings.message.clone();
                return;
            };
            let (x, y) = match self.collect_fit_values(dataset_index, x_column, y_column) {
                Ok(values) => values,
                Err(error) => {
                    self.fit_settings.message =
                        format!("拟合未执行：{}：{error}", dataset.display_name());
                    self.status = self.fit_settings.message.clone();
                    return;
                }
            };
            let mut result = match fitting::fit_values(&x, &y, &method) {
                Ok(result) => result,
                Err(error) => {
                    self.fit_settings.message =
                        format!("拟合未执行：{}：{}", dataset.display_name(), error.reason);
                    self.status = self.fit_settings.message.clone();
                    return;
                }
            };
            for point in &mut result.points {
                point[0] = self.fit_settings.unit_conversion.restore(point[0]);
            }
            let source_name = dataset.display_name();
            let point_count = x.len();
            summaries.push(format!(
                "{source_name}：R² = {:.6}，{point_count} 个点，{}",
                result.r2, result.equation
            ));
            pending.push(FitOverlay {
                points: result.points,
                r2: result.r2,
                equation: result.equation,
                display_equation: result.display_equation,
                name: format!(
                    "{source_name} · {x_column_name}/{y_column_name} 拟合 · R²={:.4}",
                    result.r2
                ),
                target: FitTarget {
                    dataset_index: Some(dataset_index),
                    source_dataset_ids: vec![dataset.plot_id.clone()],
                    x_column_name: x_column_name.clone(),
                    y_column_name: y_column_name.clone(),
                },
            });
        }

        let fitted_count = pending.len();
        let (added_count, updated_count) =
            store_fit_overlays(&mut self.fit_results.overlays, pending);
        self.fit_settings.message = if fitted_count == 1 {
            format!("拟合方程与结果：\n{}", summaries[0])
        } else {
            format!(
                "已分别完成 {fitted_count} 条曲线拟合：\n{}",
                summaries.join("\n")
            )
        };
        self.status = format!(
            "拟合完成：分别处理 {fitted_count} 条曲线，新增 {added_count} 条、更新 {updated_count} 条；当前保留 {} 条拟合曲线",
            self.fit_results.overlays.len()
        );
    }

    fn show_fit_window(&mut self, context: &egui::Context) {
        if !self.fit_open {
            return;
        }
        let dataset_names = self
            .datasets
            .iter()
            .map(data::DataSet::display_name)
            .collect::<Vec<_>>();
        let response = fitting_window::show(
            context,
            &mut self.fit_settings,
            &dataset_names,
            self.active_dataset,
            !self.fit_results.overlays.is_empty(),
        );
        if response.active_dataset != self.active_dataset {
            self.active_dataset = response.active_dataset;
            self.clamp_columns();
            self.reset_after_coordinate_change();
            self.refresh_fit_defaults_from_active_dataset();
        }
        self.fit_open = response.open;
        match response.action {
            FitAction::Execute => {
                self.execute_fit();
                context.request_repaint();
            }
            FitAction::Clear => {
                self.fit_results.overlays.clear();
                self.fit_settings.message = "已清除全部拟合曲线。".to_owned();
                self.status = "已清除全部拟合曲线".to_owned();
            }
            FitAction::None => {}
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

    fn active_fit_details(&self) -> Vec<(String, String, Option<f64>)> {
        let Some(dataset) = self.datasets.get(self.active_dataset) else {
            return Vec::new();
        };
        let mut details = Vec::new();
        if let Some(link) = dataset.fit_link.as_ref() {
            let display_equation = link.display_equation.as_ref().or(link.equation.as_ref());
            let precise_equation = link.equation.as_ref().or(display_equation);
            let r2 = dataset
                .columns
                .iter()
                .find(|column| column.name == "R²")
                .and_then(|column| {
                    column
                        .values
                        .iter()
                        .copied()
                        .find(|value| value.is_finite())
                });
            if let (Some(display_equation), Some(precise_equation)) =
                (display_equation, precise_equation)
            {
                details.push((display_equation.clone(), precise_equation.clone(), r2));
            }
        }

        let x_name = dataset
            .columns
            .get(self.x_column)
            .map(|column| &column.name);
        let y_name = dataset
            .columns
            .get(self.y_column)
            .map(|column| &column.name);
        details.extend(self.fit_results.overlays.iter().filter_map(|fit| {
            let belongs_to_active = fit
                .target
                .source_dataset_ids
                .iter()
                .any(|source_id| source_id == &dataset.plot_id);
            (belongs_to_active
                && x_name == Some(&fit.target.x_column_name)
                && y_name == Some(&fit.target.y_column_name))
            .then(|| {
                (
                    fit.display_equation.clone(),
                    fit.equation.clone(),
                    Some(fit.r2),
                )
            })
        }));
        details
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

        let linked_fit = self
            .datasets
            .get(self.active_dataset)
            .is_some_and(|dataset| {
                dataset.kind == data::DataSetKind::Fit && dataset.fit_link.is_some()
            });
        if linked_fit {
            self.x_column = 0;
            self.y_column = 1;
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
            ui.label(if linked_fit {
                "X 列（拟合关联）"
            } else {
                "X 列"
            });
            if linked_fit {
                ui.add_sized(
                    [control_width, 24.0],
                    egui::Label::new(column_names.first().map(String::as_str).unwrap_or("X")),
                );
            } else {
                column_combo(ui, "x-column", &mut self.x_column, control_width);
            }
            ui.add_space(6.0);
            ui.label(if linked_fit {
                "Y 列（拟合关联）"
            } else {
                "Y 列"
            });
            if linked_fit {
                ui.add_sized(
                    [control_width, 24.0],
                    egui::Label::new(column_names.get(1).map(String::as_str).unwrap_or("拟合 Y")),
                );
            } else {
                column_combo(ui, "y-column", &mut self.y_column, control_width);
            }
            if linked_fit {
                ui.small("关联拟合固定使用 X / 拟合 Y，因此这里显示固定列而不是下拉框。");
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label("X 列");
                if linked_fit {
                    ui.label(column_names.first().map(String::as_str).unwrap_or("X"));
                } else {
                    column_combo(ui, "x-column", &mut self.x_column, 120.0);
                }
                ui.label("Y 列");
                if linked_fit {
                    ui.label(column_names.get(1).map(String::as_str).unwrap_or("拟合 Y"));
                } else {
                    column_combo(ui, "y-column", &mut self.y_column, 120.0);
                }
                ui.separator();
                ui.label("左键点选/框选删除 · 滚轮缩放 · 右键拖动平移");
            });
            if linked_fit {
                ui.small("关联拟合固定使用 X / 拟合 Y，因此这里显示固定列而不是下拉框。");
            }
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
        let fit_details = self.active_fit_details();
        if !fit_details.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.strong("拟合结果");
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(6, 0))
                .show(ui, |ui| {
                    for (index, (display_equation, precise_equation, r2)) in
                        fit_details.into_iter().enumerate()
                    {
                        if index > 0 {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }
                        let (formula, parameters) = split_fit_display_equation(&display_equation);
                        ui.add(egui::Label::new(formula).wrap())
                            .on_hover_text(format!("完整精度：{precise_equation}"));
                        if let Some(parameters) = parameters {
                            ui.add_space(5.0);
                            ui.add(egui::Label::new(parameters).wrap())
                                .on_hover_text(format!("完整精度：{precise_equation}"));
                        }
                        if let Some(r2) = r2 {
                            ui.add_space(5.0);
                            ui.label(format!("R² = {r2:.6}"));
                        }
                    }
                });
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
        self.fit_results.overlays.clear();
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
        #[cfg(any(target_os = "windows", test))]
        self.windows_updater.poll(ui.ctx());
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

        let main_actions = main_view::show_toolbar(
            ui,
            !self.datasets.is_empty(),
            self.history.can_undo(),
            self.history.can_redo(),
        );
        for action in main_actions {
            match action {
                MainAction::OpenFiles => self.open_files(),
                MainAction::ExportImage => self.request_plot_png(ui.ctx()),
                MainAction::ExportData(format) => self.open_data_export(ui.ctx(), format),
                MainAction::OpenProcessing => self.open_processing_window(ui.ctx()),
                MainAction::OpenFitting => self.open_fit_window(ui.ctx()),
                MainAction::Undo => self.undo(),
                MainAction::Redo => self.redo(),
            }
        }
        #[cfg(any(target_os = "windows", test))]
        self.windows_updater.show_dialog(ui.ctx());
        ui.separator();
        let wide_layout = ui.available_width() >= 820.0;
        let column_names = if wide_layout {
            let sidebar_width = self.desired_sidebar_width(ui);
            let (names, sidebar_action) = main_view::show_wide_sidebar(ui, sidebar_width, |ui| {
                self.show_data_controls(ui, true)
            });
            match sidebar_action {
                SidebarAction::ResetView => {
                    self.reset_view = true;
                    self.visible_x_range = None;
                }
                SidebarAction::Clear => self.clear_data(),
                SidebarAction::None => {}
            }
            egui::Area::new(egui::Id::new("update-and-version-footer"))
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(6.0, -6.0))
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(egui::Margin::symmetric(2, 2))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                #[cfg(any(target_os = "windows", test))]
                                self.windows_updater.show_toolbar(ui);
                                #[cfg(not(any(target_os = "windows", test)))]
                                ui.label(
                                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                        .small()
                                        .weak(),
                                );
                            });
                        });
                });
            names
        } else {
            let (names, sidebar_action) =
                main_view::show_narrow_controls(ui, |ui| self.show_data_controls(ui, false));
            match sidebar_action {
                SidebarAction::ResetView => {
                    self.reset_view = true;
                    self.visible_x_range = None;
                }
                SidebarAction::Clear => self.clear_data(),
                SidebarAction::None => {}
            }
            ui.horizontal(|ui| {
                #[cfg(any(target_os = "windows", test))]
                self.windows_updater.show_toolbar(ui);
                #[cfg(not(any(target_os = "windows", test)))]
                ui.label(
                    egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .weak(),
                );
            });
            ui.separator();
            names
        };

        let pending_highlight = self
            .pending_deletion
            .as_ref()
            .map(|pending| PendingHighlight {
                dataset_index: pending.dataset_index,
                rows: &pending.rows,
                x_column: pending.x_column,
                y_column: pending.y_column,
            });
        let plot_response = plot_view::show(
            ui,
            PlotViewInput {
                demo_points: &self.demo_points,
                datasets: &self.datasets,
                active_dataset: self.active_dataset,
                x_column: self.x_column,
                y_column: self.y_column,
                column_names: &column_names,
                fit_overlays: &self.fit_results.overlays,
                pending_highlight,
                pending_deletion: self.pending_deletion.is_some(),
                reset_view: self.reset_view,
                visible_x_range: self.visible_x_range,
                last_plot_rect: self.last_plot_rect,
                selection_start: self.selection_start,
                selection_current: self.selection_current,
                selected_coordinate: self.selected_coordinate,
                status: &self.status,
            },
        );
        self.reset_view = false;
        self.last_plot_rect = Some(plot_response.last_plot_rect);
        self.last_export_rect = Some(plot_response.last_export_rect);
        self.visible_x_range = plot_response.visible_x_range;
        self.selection_start = plot_response.selection_start;
        self.selection_current = plot_response.selection_current;
        self.selected_coordinate = plot_response.selected_coordinate;
        if let Some(request) = plot_response.deletion_request {
            self.pending_deletion = Some(PendingDeletion {
                dataset_index: request.dataset_index,
                rows: request.rows,
                x_column: request.x_column,
                y_column: request.y_column,
            });
        }
        if let Some(status) = plot_response.status_update {
            self.status = status;
        }
        self.begin_startup_screenshot_if_requested(ui.ctx());
        self.show_delete_confirmation(ui.ctx());
        self.show_processing_window(ui.ctx());
        self.show_export_columns_window(ui.ctx());
        self.show_fit_window(ui.ctx());
    }
}

fn is_inside_range(value: f64, first: f64, second: f64) -> bool {
    value >= first.min(second) && value <= first.max(second)
}

fn data_export_text_format(format: DataExportFormat) -> Option<data_export::TextExportFormat> {
    match format {
        DataExportFormat::Csv => Some(data_export::TextExportFormat::Csv),
        DataExportFormat::Xlsx => None,
        DataExportFormat::Tsv => Some(data_export::TextExportFormat::Tsv),
        DataExportFormat::Txt => Some(data_export::TextExportFormat::Txt),
        DataExportFormat::Dat => Some(data_export::TextExportFormat::Dat),
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

fn demo_curve(point_count: usize) -> Vec<[f64; 2]> {
    let divisor = point_count.saturating_sub(1).max(1) as f64;
    (0..point_count)
        .map(|index| {
            let x = index as f64 / divisor * std::f64::consts::TAU * 2.0;
            [x, x.sin()]
        })
        .collect()
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

fn store_fit_overlays(
    overlays: &mut Vec<FitOverlay>,
    pending: impl IntoIterator<Item = FitOverlay>,
) -> (usize, usize) {
    let mut added = 0;
    let mut updated = 0;
    for overlay in pending {
        if store_fit_overlay(overlays, overlay) {
            updated += 1;
        } else {
            added += 1;
        }
    }
    (added, updated)
}

fn fit_dataset_indices(
    scope: FitScope,
    selected_datasets: &[bool],
    active_dataset: usize,
    dataset_count: usize,
) -> Vec<usize> {
    match scope {
        FitScope::Current => (active_dataset < dataset_count)
            .then_some(active_dataset)
            .into_iter()
            .collect(),
        FitScope::Selected => selected_datasets
            .iter()
            .take(dataset_count)
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FitOverlay, FitScope, FitTarget, demo_curve, fit_dataset_indices, is_inside_range,
        store_fit_overlay, store_fit_overlays,
    };
    use crate::data::{DataSet, DataSetKind, FitLink, NumericColumn};
    use crate::ui::plot_series::{
        data_curve_series_id, dataset_plot_columns, fit_overlay_matches_coordinates,
        plot_coordinate_names,
    };
    use crate::ui::plot_view::{reset_plot_bounds_preserving_visibility, wheel_zoom_factor};
    use eframe::egui;
    use egui_plot::{Line, Plot, PlotMemory};
    use std::path::PathBuf;

    #[test]
    fn mouse_wheel_zoom_uses_conventional_direction() {
        assert!(wheel_zoom_factor(120.0) > 1.0);
        assert!(wheel_zoom_factor(-120.0) < 1.0);
        assert_eq!(wheel_zoom_factor(0.0), 1.0);
    }

    #[test]
    fn data_curve_identity_does_not_depend_on_selected_columns() {
        let first = data_curve_series_id(2, "dataset-id");
        let after_column_change = data_curve_series_id(2, "dataset-id");
        let different_dataset = data_curve_series_id(3, "other-id");

        assert_eq!(first, after_column_change);
        assert_ne!(first, different_dataset);
    }

    #[test]
    fn resetting_plot_bounds_preserves_hidden_series() {
        let context = egui::Context::default();
        let plot_id = egui::Id::new("visibility-preserving-reset-test");
        let series_id = data_curve_series_id(0, "hidden-dataset");
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            Plot::new("visibility-preserving-reset-test")
                .id(plot_id)
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new("curve", vec![[0.0, 0.0], [1.0, 1.0]]).id(series_id));
                });
        });
        output.drop_without_applying_deltas();
        let mut memory = PlotMemory::load(&context, plot_id).expect("plot memory should exist");
        memory.hidden_items.insert(series_id);
        memory.auto_bounds = false.into();
        memory.store(&context, plot_id);

        assert!(reset_plot_bounds_preserving_visibility(&context, plot_id));

        let memory = PlotMemory::load(&context, plot_id).expect("plot memory should remain");
        assert!(memory.hidden_items.contains(&series_id));
        assert!(memory.auto_bounds.x);
        assert!(memory.auto_bounds.y);
    }

    #[test]
    fn demo_curve_has_requested_number_of_finite_points() {
        let points = demo_curve(512);
        assert_eq!(points.len(), 512);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn degree_fit_limits_are_checked_in_the_original_axis_units() {
        // The fit window presents 30–60 when the source axis is degrees; it
        // must not compare those values against the converted radian inputs.
        assert!(is_inside_range(45.0, 30.0, 60.0));
        assert!(!is_inside_range(45.0_f64.to_radians(), 30.0, 60.0));
    }

    #[test]
    fn imported_fit_uses_its_source_axes_and_overlays_the_parent_curve() {
        let source = DataSet {
            source: PathBuf::from("source.csv"),
            label: Some("source".to_owned()),
            kind: DataSetKind::Source,
            plot_id: "source-id".to_owned(),
            fit_link: None,
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "Theta".to_owned(),
                    values: vec![0.0, 1.0],
                },
                NumericColumn {
                    name: "2-X".to_owned(),
                    values: vec![1.0, 2.0],
                },
            ],
            row_count: 2,
            alive: vec![true; 2],
        };
        let fit = DataSet {
            source: PathBuf::from("fit.csv"),
            label: Some("fit".to_owned()),
            kind: DataSetKind::Fit,
            plot_id: "fit-id".to_owned(),
            fit_link: Some(FitLink {
                parent_dataset_id: Some("source-id".to_owned()),
                source_x_column: "Theta".to_owned(),
                source_y_column: "2-X".to_owned(),
                equation: Some("y = x + 1".to_owned()),
                display_equation: Some("y = x + 1".to_owned()),
            }),
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "X".to_owned(),
                    values: vec![0.0, 1.0],
                },
                NumericColumn {
                    name: "拟合 Y".to_owned(),
                    values: vec![1.1, 1.9],
                },
                NumericColumn {
                    name: "R²".to_owned(),
                    values: vec![0.99, 0.99],
                },
            ],
            row_count: 2,
            alive: vec![true; 2],
        };
        let datasets = vec![source, fit];

        assert_eq!(
            plot_coordinate_names(&datasets, 0, 0, 1),
            Some(("Theta".to_owned(), "2-X".to_owned()))
        );
        assert_eq!(
            dataset_plot_columns(&datasets, 0, 0, "Theta", "2-X"),
            Some((0, 1))
        );
        assert_eq!(
            dataset_plot_columns(&datasets, 1, 0, "Theta", "2-X"),
            Some((0, 1))
        );
        assert_eq!(
            plot_coordinate_names(&datasets, 1, 0, 1),
            Some(("Theta".to_owned(), "2-X".to_owned()))
        );
        assert_eq!(
            dataset_plot_columns(&datasets, 0, 1, "Theta", "2-X"),
            Some((0, 1))
        );
        let refitted_import = FitOverlay {
            points: vec![[0.0, 1.0]],
            r2: 0.99,
            equation: "y = x + 1".to_owned(),
            display_equation: "y = x + 1".to_owned(),
            name: "refitted import".to_owned(),
            target: FitTarget {
                dataset_index: Some(1),
                source_dataset_ids: vec!["fit-id".to_owned()],
                x_column_name: "X".to_owned(),
                y_column_name: "拟合 Y".to_owned(),
            },
        };
        assert!(fit_overlay_matches_coordinates(
            &refitted_import,
            &datasets,
            "Theta",
            "2-X"
        ));
    }

    #[test]
    fn refitting_the_same_curve_replaces_only_its_previous_fit() {
        let target = FitTarget {
            dataset_index: Some(0),
            source_dataset_ids: vec!["source-0".to_owned()],
            x_column_name: "x".to_owned(),
            y_column_name: "y".to_owned(),
        };
        let other_target = FitTarget {
            dataset_index: Some(1),
            source_dataset_ids: vec!["source-1".to_owned()],
            x_column_name: "x".to_owned(),
            y_column_name: "y".to_owned(),
        };
        let mut overlays = vec![
            FitOverlay {
                points: vec![[0.0, 1.0]],
                r2: 0.5,
                equation: "y = x".to_owned(),
                display_equation: "y = x".to_owned(),
                name: "old".to_owned(),
                target: target.clone(),
            },
            FitOverlay {
                points: vec![[0.0, 2.0]],
                r2: 0.9,
                equation: "y = 2 × x".to_owned(),
                display_equation: "y = 2 × x".to_owned(),
                name: "other".to_owned(),
                target: other_target,
            },
        ];
        assert!(store_fit_overlay(
            &mut overlays,
            FitOverlay {
                points: vec![[0.0, 3.0]],
                r2: 0.99,
                equation: "y = 3 × x".to_owned(),
                display_equation: "y = 3 × x".to_owned(),
                name: "new".to_owned(),
                target,
            }
        ));
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0].name, "new");
        assert_eq!(overlays[1].name, "other");
    }

    #[test]
    fn fitting_selection_never_merges_or_implicitly_adds_other_curves() {
        assert_eq!(
            fit_dataset_indices(FitScope::Current, &[true, true, true], 1, 3),
            vec![1]
        );
        assert_eq!(
            fit_dataset_indices(FitScope::Selected, &[true, false, true], 1, 3),
            vec![0, 2]
        );
        assert!(fit_dataset_indices(FitScope::Selected, &[false; 3], 1, 3).is_empty());
    }

    #[test]
    fn fitted_overlay_only_appears_on_its_source_coordinate_pair() {
        let fit = FitOverlay {
            points: vec![[0.0, 1.0]],
            r2: 1.0,
            equation: "y = x + 1".to_owned(),
            display_equation: "y = x + 1".to_owned(),
            name: "fit".to_owned(),
            target: FitTarget {
                dataset_index: Some(0),
                source_dataset_ids: vec!["source-0".to_owned()],
                x_column_name: "time".to_owned(),
                y_column_name: "temperature".to_owned(),
            },
        };
        assert!(fit_overlay_matches_coordinates(
            &fit,
            &[],
            "time",
            "temperature"
        ));
        assert!(!fit_overlay_matches_coordinates(
            &fit,
            &[],
            "time",
            "magnetic field"
        ));
    }

    #[test]
    fn batch_fit_results_update_each_target_without_collapsing_curves() {
        let make_overlay = |dataset_index: usize, source_id: &str, r2: f64| FitOverlay {
            points: vec![[0.0, r2]],
            r2,
            equation: format!("y = {r2} × x"),
            display_equation: format!("y = {r2} × x"),
            name: format!("fit-{source_id}"),
            target: FitTarget {
                dataset_index: Some(dataset_index),
                source_dataset_ids: vec![source_id.to_owned()],
                x_column_name: "x".to_owned(),
                y_column_name: "y".to_owned(),
            },
        };
        let mut overlays = vec![make_overlay(0, "source-0", 0.5)];
        let (added, updated) = store_fit_overlays(
            &mut overlays,
            [
                make_overlay(0, "source-0", 0.9),
                make_overlay(1, "source-1", 0.8),
            ],
        );
        assert_eq!((added, updated), (1, 1));
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0].r2, 0.9);
        assert_eq!(overlays[1].target.source_dataset_ids, ["source-1"]);
    }
}
