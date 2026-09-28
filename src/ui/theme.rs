use eframe::egui;

pub fn configure_interface_style(context: &egui::Context) {
    // InstPlot Lite is designed as a dark interface. Following the operating
    // system theme here can mix light panels with explicitly dark plot chrome,
    // which also makes labels unreadable on Windows in light mode. Native title
    // bars remain under operating-system control.
    context.options_mut(|options| options.sync_window_theme = false);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        use egui::{Color32, FontFamily, FontId, Stroke, TextStyle};

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
        style.spacing.scroll = egui::style::ScrollStyle::thin();
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

#[cfg(test)]
mod tests {
    use super::configure_interface_style;
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
}
