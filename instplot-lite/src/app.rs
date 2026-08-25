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

struct PendingDeletion {
    dataset_index: usize,
    rows: Vec<usize>,
    x_column: usize,
    y_column: usize,
}

struct ProcessingSettings {
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
}

impl Default for ProcessingSettings {
    fn default() -> Self {
        Self {
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
    fit_open: bool,
    fit_settings: FitSettings,
    fit_overlay: Option<FitOverlay>,
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
            fit_open: false,
            fit_settings: FitSettings::default(),
            fit_overlay: None,
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
            self.fit_overlay = None;
            self.selected_coordinate = None;
            self.active_dataset = self.datasets.len().saturating_sub(1);
            let column_count = self
                .datasets
                .get(self.active_dataset)
                .map_or(0, |data| data.columns.len());
            self.x_column = self.x_column.min(column_count.saturating_sub(1));
            self.y_column = if column_count > 1 { 1 } else { 0 };
            self.reset_view = true;
            self.visible_x_range = None;
        }
        self.status = match (loaded_files, errors.is_empty()) {
            (0, _) => errors.join("；"),
            (_, true) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；{last_summary}"
            ),
            (_, false) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；另有 {} 个失败：{}",
                errors.len(),
                errors.join("；")
            ),
        };
    }

    fn export_active_data(&mut self, extension: &str) {
        let Some(dataset) = self.datasets.get(self.active_dataset) else {
            self.status = "请先导入数据".to_owned();
            return;
        };
        let stem = data_export::suggested_file_stem(dataset);
        let (filter_name, extensions): (&str, &[&str]) = match extension {
            "xlsx" => ("Excel 工作簿", &["xlsx"]),
            "tsv" => ("TSV 数据", &["tsv"]),
            "txt" => ("TXT 数据", &["txt"]),
            _ => ("CSV 数据", &["csv"]),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter_name, extensions)
            .set_file_name(format!("{stem}-cleaned.{extension}"))
            .save_file()
        else {
            return;
        };
        match data_export::save_retained_rows(&path, dataset) {
            Ok(row_count) => self.status = format!("已导出 {row_count} 行数据：{}", path.display()),
            Err(error) => self.status = format!("数据导出失败：{error}"),
        }
    }

    fn export_all_text(&mut self, format: data_export::TextExportFormat) {
        if self.datasets.is_empty() {
            self.status = "请先导入数据".to_owned();
            return;
        }
        let Some(directory) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        match data_export::save_all_text(&directory, &self.datasets, format) {
            Ok(summary) => {
                self.status = format!(
                    "已导出全部 {} 个数据集、共 {} 行：{}",
                    summary.dataset_count,
                    summary.row_count,
                    directory.display()
                )
            }
            Err(error) => self.status = format!("批量数据导出失败：{error}"),
        }
    }

    fn export_all_xlsx(&mut self) {
        if self.datasets.is_empty() {
            self.status = "请先导入数据".to_owned();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Excel 工作簿", &["xlsx"])
            .set_file_name("instplot-all-data.xlsx")
            .save_file()
        else {
            return;
        };
        match data_export::save_workbook(&path, &self.datasets) {
            Ok(summary) => {
                self.status = format!(
                    "已导出全部 {} 个数据集、共 {} 行：{}",
                    summary.dataset_count,
                    summary.row_count,
                    path.display()
                )
            }
            Err(error) => self.status = format!("Excel 导出失败：{error}"),
        }
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
        self.fit_overlay = None;
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

    fn open_processing_window(&mut self) {
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
        let dataset_index = self.active_dataset;
        let source_column = self.y_column;
        let Some(dataset) = self.datasets.get(dataset_index) else {
            self.status = "请先导入数据".to_owned();
            return;
        };
        let Some(source_name) = dataset
            .columns
            .get(source_column)
            .map(|column| column.name.clone())
        else {
            self.status = "当前 Y 列不存在".to_owned();
            return;
        };
        let result = match processing::apply_to_dataset(dataset, source_column, &operation) {
            Ok(result) => result,
            Err(error) => {
                self.status = format!("处理失败：{error}");
                return;
            }
        };
        let summary = processing_summary(&result.metadata);
        let dataset = &mut self.datasets[dataset_index];
        let column_name = unique_column_name(dataset, &format!("{source_name} [{suffix}]"));
        let column_index = dataset.columns.len();
        dataset.columns.push(data::NumericColumn {
            name: column_name.clone(),
            values: result.values,
        });
        self.history.record_add_column(
            dataset_index,
            column_index,
            column_name.clone(),
            source_column,
            operation,
        );
        self.fit_overlay = None;
        self.y_column = column_index;
        self.reset_view = true;
        self.visible_x_range = None;
        self.status = format!("已生成派生列“{column_name}”；{summary}；原始列未修改");
    }

    fn show_processing_window(&mut self, context: &egui::Context) {
        if !self.processing_open {
            return;
        }
        let mut open = true;
        let mut requested: Option<(ProcessingOperation, String)> = None;
        let viewport_id = egui::ViewportId::from_hash_of("instplot-lite-processing");
        context.send_viewport_cmd_to(
            viewport_id,
            egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark),
        );
        context.show_viewport_immediate(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("InstPlot Lite · 数据处理")
                .with_inner_size([570.0, 455.0])
                .with_min_inner_size([520.0, 420.0])
                .with_resizable(true),
            |ui, _class| {
                if ui.ctx().input(|input| input.viewport().close_requested()) {
                    open = false;
                    return;
                }
                let content_rect = ui
                    .available_rect_before_wrap()
                    .shrink2(egui::vec2(18.0, 14.0));
                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let ui = &mut content_ui;
                ui.label("处理当前数据集的 Y 列，并生成新列；原始数据不会被覆盖。");
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
                        .on_hover_text("生成新的局部展平派生列，不修改原始列")
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
            },
        );
        self.processing_open = open;
        if let Some((operation, suffix)) = requested {
            self.apply_processing(operation, &suffix);
        }
    }

    fn open_fit_window(&mut self) {
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
                    .filter(|value| !value.is_empty())
                    .map(str::parse::<f64>)
                    .collect::<Result<Vec<_>, _>>();
                let Ok(parameters) = parameters else {
                    self.fit_settings.message = "拟合失败：初始参数应为逗号分隔的数值".to_owned();
                    return;
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
                self.status = format!("拟合完成：R² = {:.6}，使用 {point_count} 个点", result.r2);
                self.fit_overlay = Some(FitOverlay {
                    points: result.points,
                    r2: result.r2,
                });
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
        let viewport_id = egui::ViewportId::from_hash_of("instplot-lite-fitting");
        context.send_viewport_cmd_to(
            viewport_id,
            egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark),
        );
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
                let content_rect = ui
                    .available_rect_before_wrap()
                    .shrink2(egui::vec2(18.0, 14.0));
                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let ui = &mut content_ui;
                ui.label("使用当前 X/Y 列进行拟合；已删除和非数值数据点不会参与计算。");
                ui.separator();
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
                        .selected_text(unit_conversion_name(self.fit_settings.unit_conversion))
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
                        ui.add(egui::DragValue::new(&mut self.fit_settings.degree).range(1..=10));
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
                        "例如：1, 1, 0",
                    );
                    ui.small("参数按 a、b、c、d、e_param、f、g、h 的顺序填写，用逗号分隔。");
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
                            self.fit_overlay.is_some(),
                            egui::Button::new("清除拟合曲线"),
                        )
                        .clicked()
                    {
                        clear = true;
                    }
                });
                ui.add_space(8.0);
                ui.label(&self.fit_settings.message);
            },
        );
        self.fit_open = open;
        if execute {
            self.execute_fit();
            context.request_repaint();
        }
        if clear {
            self.fit_overlay = None;
            self.fit_settings.message = "拟合曲线已清除。".to_owned();
            self.status = "已清除拟合曲线".to_owned();
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
        const MIN_WIDTH: f32 = 185.0;
        const MAX_WIDTH: f32 = 280.0;
        const COMBO_DECORATION_WIDTH: f32 = 52.0;

        let mut labels = Vec::with_capacity(3);
        if let Some(dataset) = self.datasets.get(self.active_dataset) {
            labels.push(dataset.display_name());
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
            egui::ComboBox::from_id_salt("active-dataset")
                .width(if vertical {
                    (ui.available_width() - 10.0).max(100.0)
                } else {
                    150.0
                })
                .selected_text(
                    dataset_names
                        .get(app.active_dataset)
                        .map(String::as_str)
                        .unwrap_or("未选择"),
                )
                .show_ui(ui, |ui| {
                    for (index, name) in dataset_names.iter().enumerate() {
                        ui.selectable_value(&mut app.active_dataset, index, name);
                    }
                });
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
            self.fit_overlay = None;
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
            self.fit_overlay = None;
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
        self.fit_open = false;
        self.fit_overlay = None;
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
            self.fit_overlay = None;
            self.clamp_columns();
            self.status = match effect {
                HistoryEffect::Rows(count) => format!("已撤销，恢复 {count} 个点"),
                HistoryEffect::Column(name) => format!("已撤销处理，移除派生列“{name}”"),
            };
        }
    }

    fn redo(&mut self) {
        if let Some(effect) = self.history.redo(&mut self.datasets) {
            self.fit_overlay = None;
            self.clamp_columns();
            self.status = match effect {
                HistoryEffect::Rows(count) => format!("已重做，删除 {count} 个点"),
                HistoryEffect::Column(name) => format!("已重做处理，恢复派生列“{name}”"),
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
                    ui.label(egui::RichText::new("当前数据集").strong());
                    if ui.button("CSV").clicked() {
                        ui.close();
                        self.export_active_data("csv");
                    }
                    if ui.button("Excel（XLSX）").clicked() {
                        ui.close();
                        self.export_active_data("xlsx");
                    }
                    if ui.button("TSV").clicked() {
                        ui.close();
                        self.export_active_data("tsv");
                    }
                    if ui.button("TXT（制表符分隔）").clicked() {
                        ui.close();
                        self.export_active_data("txt");
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("全部数据集").strong());
                    if ui.button("一个 Excel 工作簿").clicked() {
                        ui.close();
                        self.export_all_xlsx();
                    }
                    if ui.button("多个 CSV 文件").clicked() {
                        ui.close();
                        self.export_all_text(data_export::TextExportFormat::Csv);
                    }
                    if ui.button("多个 TSV 文件").clicked() {
                        ui.close();
                        self.export_all_text(data_export::TextExportFormat::Tsv);
                    }
                    if ui.button("多个 TXT 文件").clicked() {
                        ui.close();
                        self.export_all_text(data_export::TextExportFormat::Txt);
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
                self.open_processing_window();
            }
            if ui
                .add_enabled(
                    !self.datasets.is_empty(),
                    egui::Button::new(egui::RichText::new("曲线拟合").strong()),
                )
                .clicked()
            {
                self.open_fit_window();
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
            .allow_scroll(true)
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
                        plot_ui
                            .line(Line::new(dataset.display_name(), points.clone()).color(color));
                        plot_ui.points(Points::new("", points).color(color).radius(3.5));
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
                if let Some(fit) = &self.fit_overlay {
                    plot_ui.line(
                        Line::new(format!("拟合 R²={:.4}", fit.r2), fit.points.clone())
                            .color(Color32::from_rgb(255, 196, 64))
                            .width(2.5),
                    );
                }
            })
        });
        ui.add_space(PLOT_BOTTOM_GUTTER);
        self.last_export_rect = Some(Rect::from_min_max(
            plot_row.response.rect.min,
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

fn configure_interface_style(context: &egui::Context) {
    // InstPlot Lite is designed as a dark interface. Following the operating
    // system theme here can mix light panels with explicitly dark plot chrome,
    // which also makes labels unreadable on Windows in light mode.
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

#[cfg(test)]
mod tests {
    use super::{configure_interface_style, demo_curve};
    use eframe::egui;

    #[test]
    fn interface_style_always_uses_dark_theme() {
        let context = egui::Context::default();
        context.set_theme(egui::Theme::Light);

        configure_interface_style(&context);

        assert_eq!(context.theme(), egui::Theme::Dark);
        assert!(context.global_style().visuals.dark_mode);
    }

    #[test]
    fn demo_curve_has_requested_number_of_finite_points() {
        let points = demo_curve(512);
        assert_eq!(points.len(), 512);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }
}
