use std::path::PathBuf;

use eframe::egui::{self, Color32, PointerButton, Rect, Stroke, StrokeKind};
use egui_plot::{Legend, Line, Plot, PlotPoint, Points};

use crate::{
    data, data_export,
    edit_history::{EditHistory, HistoryEffect},
    fonts, image_export,
    processing::{self, Anchor, ProcessingMetadata, ProcessingOperation},
};

const PLOT_LEFT_GUTTER: f32 = 36.0;

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
    selected_coordinate: Option<[f64; 2]>,
    status: String,
}

impl InstPlotLiteApp {
    pub fn new(
        creation_context: &eframe::CreationContext<'_>,
        startup_files: Vec<PathBuf>,
        startup_screenshot: Option<PathBuf>,
    ) -> Self {
        let font_status = fonts::install_cjk_font(&creation_context.egui_ctx);
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
            selected_coordinate: None,
            status: font_status,
        };
        if !startup_files.is_empty() {
            app.load_paths(startup_files);
        }
        app
    }

    fn open_files(&mut self) {
        let paths = rfd::FileDialog::new()
            .add_filter("数据文件", &["txt", "csv", "dat"])
            .pick_files();
        if let Some(paths) = paths {
            self.load_paths(paths);
        }
    }

    fn load_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut loaded = 0_usize;
        let mut last_summary = String::new();
        let mut errors = Vec::new();
        for path in paths {
            match data::read_data_file(&path) {
                Ok(dataset) => {
                    last_summary = format!(
                        "{}：{} 行，{} 个数值列，编码 {}，分隔符 {}",
                        dataset.display_name(),
                        dataset.row_count,
                        dataset.columns.len(),
                        dataset.encoding,
                        dataset.separator
                    );
                    self.datasets.push(dataset);
                    loaded += 1;
                }
                Err(error) => errors.push(format!("{}：{error}", path.display())),
            }
        }
        if loaded > 0 {
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
        self.status = match (loaded, errors.is_empty()) {
            (0, _) => errors.join("；"),
            (_, true) => format!("已导入 {loaded} 个文件；{last_summary}"),
            (_, false) => format!(
                "已导入 {loaded} 个文件；另有 {} 个失败：{}",
                errors.len(),
                errors.join("；")
            ),
        };
    }

    fn export_active_data(&mut self) {
        let Some(dataset) = self.datasets.get(self.active_dataset) else {
            self.status = "请先导入数据".to_owned();
            return;
        };
        let stem = dataset
            .source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("instplot-data");
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV 数据", &["csv"])
            .add_filter("TXT 数据", &["txt"])
            .set_file_name(format!("{stem}-cleaned.csv"))
            .save_file()
        else {
            return;
        };
        match data_export::save_retained_rows(&path, dataset) {
            Ok(row_count) => self.status = format!("已导出 {row_count} 行数据：{}", path.display()),
            Err(error) => self.status = format!("数据导出失败：{error}"),
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
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("instplot-lite-processing"),
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
                HistoryEffect::Column(name) => format!("已撤销处理，移除派生列“{name}”"),
            };
        }
    }

    fn redo(&mut self) {
        if let Some(effect) = self.history.redo(&mut self.datasets) {
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
            if ui
                .add_enabled(!self.datasets.is_empty(), egui::Button::new("导出数据"))
                .clicked()
            {
                self.export_active_data();
            }
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

        let plot_height = ui.available_height().max(220.0);
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
            })
        });
        self.last_export_rect = Some(plot_row.response.rect);
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
    context.all_styles_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};

        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(13.0, FontFamily::Proportional),
        );
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
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
    use super::demo_curve;

    #[test]
    fn demo_curve_has_requested_number_of_finite_points() {
        let points = demo_curve(512);
        assert_eq!(points.len(), 512);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }
}
