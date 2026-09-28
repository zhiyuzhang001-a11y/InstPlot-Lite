use eframe::egui;

use super::theme::{CONTROL_BG, PANEL_BG, SPACE_MD, SPACE_SM, apply_shell_style};

pub(crate) fn processing_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("instplot-lite-processing")
}

pub(crate) fn fitting_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("instplot-lite-fitting")
}

pub(crate) fn export_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("instplot-lite-export")
}

pub(crate) fn show_tool_viewport<T>(
    context: &egui::Context,
    viewport_id: egui::ViewportId,
    builder: egui::ViewportBuilder,
    viewport_ui: impl FnMut(&mut egui::Ui, egui::ViewportClass) -> T,
) -> T {
    let previously_embedded = context.embed_viewports();
    if context_should_embed_tool_windows(context) {
        context.set_embed_viewports(true);
    }
    let result = context.show_viewport_immediate(viewport_id, builder, viewport_ui);
    context.set_embed_viewports(previously_embedded);
    result
}

fn context_should_embed_tool_windows(context: &egui::Context) -> bool {
    context.input(|input| {
        let viewport = input.viewport();
        tool_windows_should_be_embedded(
            viewport.fullscreen,
            viewport.maximized,
            cfg!(target_os = "windows"),
        )
    })
}

fn tool_windows_should_be_embedded(
    fullscreen: Option<bool>,
    maximized: Option<bool>,
    is_windows: bool,
) -> bool {
    fullscreen == Some(true) || (is_windows && maximized == Some(true))
}

pub(crate) fn show_embedded_window_close_control(
    ui: &mut egui::Ui,
    viewport_class: egui::ViewportClass,
    open: &mut bool,
) -> bool {
    if viewport_class != egui::ViewportClass::EmbeddedWindow {
        return false;
    }

    let escape_pressed = ui.ctx().top_layer_id() == Some(ui.layer_id())
        && ui
            .ctx()
            .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut close_clicked = false;
    egui::Frame::NONE
        .fill(PANEL_BG)
        .inner_margin(egui::Margin::symmetric(SPACE_MD as i8, SPACE_SM as i8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close_clicked = ui
                        .add(
                            egui::Button::new("关闭")
                                .fill(CONTROL_BG)
                                .stroke(egui::Stroke::NONE),
                        )
                        .on_hover_text("关闭此窗口（Esc）")
                        .clicked();
                });
            });
        });
    ui.separator();
    if escape_pressed || close_clicked {
        *open = false;
        true
    } else {
        false
    }
}

pub(crate) fn apply_tool_window_surface(ui: &mut egui::Ui) {
    ui.painter()
        .rect_filled(ui.max_rect(), egui::CornerRadius::ZERO, PANEL_BG);
    let mut style = ui.style().as_ref().clone();
    apply_shell_style(&mut style);
    ui.set_style(style);
}

pub(crate) fn focus_viewport(context: &egui::Context, viewport_id: egui::ViewportId) {
    if context.embed_viewports() || context_should_embed_tool_windows(context) {
        context.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new(viewport_id),
        ));
    } else {
        context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Minimized(false));
        context.send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_viewport_id, fitting_viewport_id, processing_viewport_id,
        tool_windows_should_be_embedded,
    };

    #[test]
    fn tool_windows_embed_in_fullscreen_or_when_windows_is_maximized() {
        assert!(tool_windows_should_be_embedded(
            Some(true),
            Some(false),
            false
        ));
        assert!(tool_windows_should_be_embedded(
            Some(true),
            Some(false),
            true
        ));
        assert!(tool_windows_should_be_embedded(
            Some(false),
            Some(true),
            true
        ));

        assert!(!tool_windows_should_be_embedded(
            Some(false),
            Some(true),
            false
        ));
        assert!(!tool_windows_should_be_embedded(
            Some(false),
            Some(false),
            true
        ));
        assert!(!tool_windows_should_be_embedded(None, None, true));
    }

    #[test]
    fn tool_windows_have_distinct_viewport_ids() {
        let processing = processing_viewport_id();
        let fitting = fitting_viewport_id();
        let export = export_viewport_id();

        assert_ne!(processing, fitting);
        assert_ne!(processing, export);
        assert_ne!(fitting, export);
    }
}
