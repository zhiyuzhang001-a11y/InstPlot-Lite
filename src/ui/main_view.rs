use eframe::egui;

use super::export_window::DataExportFormat;

pub const STATUS_ROW_HEIGHT: f32 = 22.0;
pub const STATUS_BOTTOM_INSET: f32 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainAction {
    OpenFiles,
    ExportImage,
    ExportData(DataExportFormat),
    OpenProcessing,
    OpenFitting,
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    None,
    ResetView,
    Clear,
}

pub fn show_toolbar(
    ui: &mut egui::Ui,
    has_data: bool,
    can_undo: bool,
    can_redo: bool,
) -> Vec<MainAction> {
    let mut actions = Vec::new();
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("InstPlot Lite");
        ui.separator();
        if ui
            .button(egui::RichText::new("打开文件").strong())
            .clicked()
        {
            actions.push(MainAction::OpenFiles);
        }
        if ui.button("导出图片").clicked() {
            actions.push(MainAction::ExportImage);
        }
        ui.add_enabled_ui(has_data, |ui| {
            ui.menu_button(egui::RichText::new("导出数据…").strong(), |ui| {
                for (label, format) in [
                    ("CSV", DataExportFormat::Csv),
                    ("Excel（XLSX）", DataExportFormat::Xlsx),
                    ("TSV", DataExportFormat::Tsv),
                    ("TXT（制表符分隔）", DataExportFormat::Txt),
                    ("DAT（制表符分隔）", DataExportFormat::Dat),
                ] {
                    if ui.button(label).clicked() {
                        ui.close();
                        actions.push(MainAction::ExportData(format));
                    }
                }
            });
        });
        if ui
            .add_enabled(
                has_data,
                egui::Button::new(egui::RichText::new("数据处理").strong()),
            )
            .clicked()
        {
            actions.push(MainAction::OpenProcessing);
        }
        if ui
            .add_enabled(
                has_data,
                egui::Button::new(egui::RichText::new("曲线拟合").strong()),
            )
            .clicked()
        {
            actions.push(MainAction::OpenFitting);
        }
        if ui
            .add_enabled(can_undo, egui::Button::new("← 撤销"))
            .on_hover_text("撤销最近一次删除或数据处理")
            .clicked()
        {
            actions.push(MainAction::Undo);
        }
        if ui
            .add_enabled(can_redo, egui::Button::new("重做 →"))
            .on_hover_text("重新执行刚刚撤销的操作")
            .clicked()
        {
            actions.push(MainAction::Redo);
        }
    });
    actions
}

pub fn show_wide_sidebar<R>(
    ui: &mut egui::Ui,
    width: f32,
    show_data_controls: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, SidebarAction) {
    let mut action = SidebarAction::None;
    let value = egui::Panel::left("data-controls")
        .exact_size(width)
        .resizable(false)
        .show(ui, |ui| {
            ui.heading("数据");
            ui.separator();
            let value = show_data_controls(ui);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("复位视图").clicked() {
                    action = SidebarAction::ResetView;
                }
                if ui.button("清空").clicked() {
                    action = SidebarAction::Clear;
                }
            });
            ui.add_space(14.0);
            ui.separator();
            ui.label("左键：点选或框选删除");
            ui.label("滚轮：缩放");
            ui.label("右键拖动：平移");
            value
        })
        .inner;
    (value, action)
}

pub fn show_narrow_controls<R>(
    ui: &mut egui::Ui,
    show_data_controls: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, SidebarAction) {
    let value = show_data_controls(ui);
    let mut action = SidebarAction::None;
    ui.horizontal_wrapped(|ui| {
        if ui.button("复位视图").clicked() {
            action = SidebarAction::ResetView;
        }
        if ui.button("清空").clicked() {
            action = SidebarAction::Clear;
        }
    });
    (value, action)
}

pub fn show_status(ui: &mut egui::Ui, status: &str, coordinate: Option<[f64; 2]>) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), STATUS_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.spacing_mut().interact_size.y = 20.0;
            let coordinate = coordinate.map(|[x, y]| format!("({x}, {y})"));
            let reserved_width = coordinate.as_ref().map_or(0.0, |text| {
                let font_id = egui::TextStyle::Body.resolve(ui.style());
                ui.painter()
                    .layout_no_wrap(text.clone(), font_id, ui.visuals().text_color())
                    .size()
                    .x
                    + 26.0
            });
            let status_width = (ui.available_width() - reserved_width).max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(status_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    ui.add(egui::Label::new(status).truncate())
                        .on_hover_text(status);
                },
            );
            if let Some(coordinate) = coordinate {
                ui.separator();
                ui.label(coordinate);
            }
        },
    );
    ui.add_space(STATUS_BOTTOM_INSET);
}
