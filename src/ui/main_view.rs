use eframe::egui;

use super::{
    export_window::DataExportFormat,
    theme::{
        CONTROL_BG, FG_DANGER, FG_PRIMARY, FG_SECONDARY, PANEL_BG, SHELL_BG, SPACE_LG, SPACE_MD,
        SPACE_SM, SPACE_XS, shell_scope,
    },
};

pub const STATUS_ROW_HEIGHT: f32 = 22.0;
pub const STATUS_BOTTOM_INSET: f32 = 15.0;
const TOOLBAR_SINGLE_ROW_MIN_WIDTH: f32 = 900.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarLayout {
    SingleRow,
    TwoRows,
}

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
    let layout = toolbar_layout(ui.available_width());
    egui::Frame::NONE.fill(SHELL_BG).show(ui, |ui| {
        shell_scope(ui, |ui| {
            ui.add_space(SPACE_XS);
            match layout {
                ToolbarLayout::SingleRow => {
                    ui.horizontal(|ui| {
                        show_app_name(ui);
                        ui.add_space(SPACE_LG);
                        show_file_actions(ui, has_data, &mut actions);
                        ui.add_space(SPACE_LG);
                        show_analysis_actions(ui, has_data, &mut actions);
                        ui.add_space(SPACE_LG);
                        show_history_actions(ui, can_undo, can_redo, &mut actions);
                    });
                }
                ToolbarLayout::TwoRows => {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            show_app_name(ui);
                            ui.add_space(SPACE_LG);
                            show_file_actions(ui, has_data, &mut actions);
                        });
                        ui.horizontal(|ui| {
                            show_analysis_actions(ui, has_data, &mut actions);
                            ui.add_space(SPACE_LG);
                            show_history_actions(ui, can_undo, can_redo, &mut actions);
                        });
                    });
                }
            }
        });
    });
    actions
}

fn toolbar_layout(available_width: f32) -> ToolbarLayout {
    if available_width >= TOOLBAR_SINGLE_ROW_MIN_WIDTH {
        ToolbarLayout::SingleRow
    } else {
        ToolbarLayout::TwoRows
    }
}

fn show_app_name(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("InstPlot Lite")
            .size(17.0)
            .color(FG_SECONDARY),
    );
}

fn show_file_actions(ui: &mut egui::Ui, has_data: bool, actions: &mut Vec<MainAction>) {
    if ui
        .add(
            egui::Button::new(egui::RichText::new("打开文件").strong().color(SHELL_BG))
                .fill(FG_PRIMARY)
                .stroke(egui::Stroke::NONE),
        )
        .clicked()
    {
        actions.push(MainAction::OpenFiles);
    }
    if toolbar_button(ui, "导出图片").clicked() {
        actions.push(MainAction::ExportImage);
    }
    ui.add_enabled_ui(has_data, |ui| {
        ui.menu_button("导出数据…", |ui| {
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
}

fn show_analysis_actions(ui: &mut egui::Ui, has_data: bool, actions: &mut Vec<MainAction>) {
    if ui
        .add_enabled(has_data, toolbar_button_widget("数据处理"))
        .clicked()
    {
        actions.push(MainAction::OpenProcessing);
    }
    if ui
        .add_enabled(has_data, toolbar_button_widget("曲线拟合"))
        .clicked()
    {
        actions.push(MainAction::OpenFitting);
    }
}

fn show_history_actions(
    ui: &mut egui::Ui,
    can_undo: bool,
    can_redo: bool,
    actions: &mut Vec<MainAction>,
) {
    if ui
        .add_enabled(can_undo, history_button_widget("← 撤销"))
        .on_hover_text("撤销最近一次删除或数据处理")
        .clicked()
    {
        actions.push(MainAction::Undo);
    }
    if ui
        .add_enabled(can_redo, history_button_widget("重做 →"))
        .on_hover_text("重新执行刚刚撤销的操作")
        .clicked()
    {
        actions.push(MainAction::Redo);
    }
}

fn toolbar_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(toolbar_button_widget(label))
}

fn toolbar_button_widget(label: &str) -> egui::Button<'_> {
    egui::Button::new(label)
        .fill(PANEL_BG)
        .stroke(egui::Stroke::NONE)
}

fn history_button_widget(label: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).color(FG_SECONDARY))
        .fill(SHELL_BG)
        .stroke(egui::Stroke::NONE)
}

pub fn show_wide_sidebar<R>(
    ui: &mut egui::Ui,
    width: f32,
    show_data_controls: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, SidebarAction) {
    let mut action = SidebarAction::None;
    let frame = egui::Frame::side_top_panel(ui.style())
        .fill(PANEL_BG)
        .inner_margin(egui::Margin::symmetric(SPACE_MD as i8, SPACE_SM as i8));
    let value = egui::Panel::left("data-controls")
        .exact_size(width)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            shell_scope(ui, |ui| {
                ui.label(egui::RichText::new("数据").size(17.0).strong());
                ui.add_space(SPACE_XS);
                ui.separator();
                ui.add_space(SPACE_SM);
                let value = show_data_controls(ui);
                ui.add_space(SPACE_LG);
                ui.horizontal(|ui| {
                    if ui.add(sidebar_button("复位视图")).clicked() {
                        action = SidebarAction::ResetView;
                    }
                    if ui.add(clear_button("清空")).clicked() {
                        action = SidebarAction::Clear;
                    }
                });
                ui.add_space(SPACE_LG);
                ui.separator();
                ui.add_space(SPACE_SM);
                ui.label(egui::RichText::new("左键：点选或框选删除").color(FG_SECONDARY));
                ui.label(egui::RichText::new("滚轮：缩放").color(FG_SECONDARY));
                ui.label(egui::RichText::new("右键拖动：平移").color(FG_SECONDARY));
                value
            })
        })
        .inner;
    (value, action)
}

pub fn show_narrow_controls<R>(
    ui: &mut egui::Ui,
    show_data_controls: impl FnOnce(&mut egui::Ui) -> R,
) -> (R, SidebarAction) {
    let mut action = SidebarAction::None;
    let value = show_data_controls(ui);
    ui.add_space(SPACE_SM);
    ui.horizontal_wrapped(|ui| {
        if ui.add(sidebar_button("复位视图")).clicked() {
            action = SidebarAction::ResetView;
        }
        if ui.add(clear_button("清空")).clicked() {
            action = SidebarAction::Clear;
        }
    });
    (value, action)
}

pub fn show_narrow_data_region<R>(
    ui: &mut egui::Ui,
    show_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available_width = ui.available_width();
    egui::Frame::NONE
        .fill(PANEL_BG)
        .inner_margin(egui::Margin::symmetric(SPACE_MD as i8, SPACE_SM as i8))
        .show(ui, |ui| {
            shell_scope(ui, |ui| {
                ui.set_min_width((available_width - 2.0 * SPACE_MD).max(0.0));
                show_contents(ui)
            })
        })
        .inner
}

fn sidebar_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(label)
        .fill(CONTROL_BG)
        .stroke(egui::Stroke::NONE)
}

fn clear_button(label: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).color(FG_DANGER))
        .fill(PANEL_BG)
        .stroke(egui::Stroke::NONE)
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
                    ui.add(
                        egui::Label::new(egui::RichText::new(status).color(FG_SECONDARY))
                            .truncate(),
                    )
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

#[cfg(test)]
mod tests {
    use super::{ToolbarLayout, toolbar_layout};

    #[test]
    fn toolbar_uses_the_documented_breakpoint() {
        assert_eq!(toolbar_layout(900.0), ToolbarLayout::SingleRow);
        assert_eq!(toolbar_layout(899.0), ToolbarLayout::TwoRows);
        assert_eq!(toolbar_layout(720.0), ToolbarLayout::TwoRows);
    }
}
