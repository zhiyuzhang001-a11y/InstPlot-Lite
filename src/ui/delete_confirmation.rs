use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAction {
    None,
    Confirm,
    Cancel,
}

pub fn show(context: &egui::Context, count: usize) -> DeleteAction {
    let mut action = DeleteAction::None;
    egui::Window::new("确认删除")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(context, |ui| {
            ui.label(format!("确定删除选中的 {count} 个数据点吗？"));
            ui.horizontal(|ui| {
                if ui.button("删除").clicked() {
                    action = DeleteAction::Confirm;
                }
                if ui.button("取消").clicked() {
                    action = DeleteAction::Cancel;
                }
            });
        });
    action
}
