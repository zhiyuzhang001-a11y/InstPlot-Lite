use eframe::egui;

pub(crate) const SPACE_XS: f32 = 4.0;
pub(crate) const SPACE_SM: f32 = 8.0;
pub(crate) const SPACE_MD: f32 = 12.0;
pub(crate) const SPACE_LG: f32 = 16.0;

pub(crate) const CONTROL_HEIGHT: f32 = 32.0;
pub(crate) const CONTROL_RADIUS: u8 = 8;
pub(crate) const WINDOW_RADIUS: u8 = 10;

// Application-shell colors are intentionally separate from the plot palette.
// The plot background and exported image styling remain owned by plot_view.
pub(crate) const PLOT_BG_IMMUTABLE: egui::Color32 = egui::Color32::from_rgb(0, 0, 0);
pub(crate) const SHELL_BG: egui::Color32 = egui::Color32::from_rgb(22, 22, 22);
pub(crate) const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(38, 38, 38);
pub(crate) const CONTROL_BG: egui::Color32 = egui::Color32::from_rgb(57, 57, 57);
pub(crate) const CONTROL_HOVER_BG: egui::Color32 = egui::Color32::from_rgb(82, 82, 82);
pub(crate) const CONTROL_PRESSED_BG: egui::Color32 = egui::Color32::from_rgb(111, 111, 111);
pub(crate) const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(82, 82, 82);
pub(crate) const BORDER_CONTROL: egui::Color32 = egui::Color32::from_rgb(141, 141, 141);
pub(crate) const BORDER_FOCUS: egui::Color32 = egui::Color32::from_rgb(244, 244, 244);
pub(crate) const FG_PRIMARY: egui::Color32 = egui::Color32::from_rgb(244, 244, 244);
pub(crate) const FG_SECONDARY: egui::Color32 = egui::Color32::from_rgb(198, 198, 198);
pub(crate) const FG_DISABLED: egui::Color32 = egui::Color32::from_rgb(141, 141, 141);
pub(crate) const FG_DANGER: egui::Color32 = egui::Color32::from_rgb(250, 154, 159);

pub(crate) fn shell_scope<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.scope(|ui| {
        let mut style = ui.style().as_ref().clone();
        apply_shell_style(&mut style);
        ui.set_style(style);
        add_contents(ui)
    })
    .inner
}

pub(crate) fn apply_shell_style(style: &mut egui::Style) {
    use egui::Stroke;

    debug_assert_ne!(PLOT_BG_IMMUTABLE, SHELL_BG);
    style.visuals.override_text_color = Some(FG_PRIMARY);
    style.visuals.disabled_alpha = f32::from(FG_DISABLED.r()) / f32::from(FG_PRIMARY.r());
    style.visuals.weak_text_color = Some(FG_SECONDARY);
    style.visuals.panel_fill = PANEL_BG;
    style.visuals.window_fill = PANEL_BG;
    style.visuals.extreme_bg_color = CONTROL_BG;
    style.visuals.faint_bg_color = SHELL_BG;
    style.visuals.widgets.noninteractive.bg_fill = SHELL_BG;
    style.visuals.widgets.noninteractive.weak_bg_fill = SHELL_BG;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, FG_SECONDARY);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    style.visuals.widgets.inactive.bg_fill = CONTROL_BG;
    style.visuals.widgets.inactive.weak_bg_fill = CONTROL_BG;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, FG_PRIMARY);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_CONTROL);
    style.visuals.widgets.hovered.bg_fill = CONTROL_HOVER_BG;
    style.visuals.widgets.hovered.weak_bg_fill = CONTROL_HOVER_BG;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, FG_PRIMARY);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_FOCUS);
    style.visuals.widgets.active.bg_fill = CONTROL_PRESSED_BG;
    style.visuals.widgets.active.weak_bg_fill = CONTROL_PRESSED_BG;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, FG_PRIMARY);
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0, BORDER_FOCUS);
    style.visuals.widgets.open.bg_fill = CONTROL_PRESSED_BG;
    style.visuals.widgets.open.weak_bg_fill = CONTROL_PRESSED_BG;
    style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, FG_PRIMARY);
    style.visuals.widgets.open.bg_stroke = Stroke::new(2.0, BORDER_FOCUS);
    style.visuals.selection.bg_fill = CONTROL_HOVER_BG;
    style.visuals.selection.stroke = Stroke::new(2.0, BORDER_FOCUS);
    style.visuals.hyperlink_color = FG_SECONDARY;
    style.visuals.warn_fg_color = FG_SECONDARY;
    style.visuals.error_fg_color = FG_DANGER;
}

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
        style.spacing.button_padding = egui::vec2(SPACE_MD, 6.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        style.visuals.selection.bg_fill = Color32::from_gray(78);
        style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        style.visuals.hyperlink_color = Color32::from_gray(210);
        style.visuals.warn_fg_color = Color32::from_gray(220);
        style.visuals.error_fg_color = Color32::WHITE;
        let radius = egui::CornerRadius::same(CONTROL_RADIUS);
        style.visuals.widgets.inactive.corner_radius = radius;
        style.visuals.widgets.hovered.corner_radius = radius;
        style.visuals.widgets.active.corner_radius = radius;
        style.visuals.widgets.open.corner_radius = radius;
        style.visuals.window_corner_radius = egui::CornerRadius::same(WINDOW_RADIUS);
        style.visuals.menu_corner_radius = radius;
    });
}

#[cfg(test)]
mod tests {
    use super::{
        BORDER_CONTROL, BORDER_FOCUS, CONTROL_BG, CONTROL_HEIGHT, CONTROL_RADIUS, FG_PRIMARY,
        PANEL_BG, PLOT_BG_IMMUTABLE, SHELL_BG, WINDOW_RADIUS, configure_interface_style,
        shell_scope,
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
        let style = context.global_style();
        assert_eq!(style.spacing.interact_size.y, CONTROL_HEIGHT);
        assert_eq!(
            style.visuals.widgets.inactive.corner_radius,
            egui::CornerRadius::same(CONTROL_RADIUS)
        );
        assert_eq!(
            style.visuals.window_corner_radius,
            egui::CornerRadius::same(WINDOW_RADIUS)
        );
    }

    #[test]
    fn shell_tokens_keep_plot_and_application_surfaces_distinct() {
        assert_eq!(PLOT_BG_IMMUTABLE, egui::Color32::BLACK);
        assert_ne!(SHELL_BG, PLOT_BG_IMMUTABLE);
        assert_ne!(PANEL_BG, SHELL_BG);
        assert!(contrast_ratio(FG_PRIMARY, SHELL_BG) >= 4.5);
        assert!(contrast_ratio(BORDER_CONTROL, CONTROL_BG) >= 3.0);
        assert!(contrast_ratio(BORDER_FOCUS, CONTROL_BG) >= 3.0);
    }

    #[test]
    fn shell_scope_does_not_mutate_the_context_style() {
        let context = egui::Context::default();
        configure_interface_style(&context);
        let original = context.global_style();

        let mut output = context.run_ui(Default::default(), |ui| {
            shell_scope(ui, |ui| {
                assert_eq!(ui.visuals().panel_fill, PANEL_BG);
            });
        });
        output.textures_delta.clear();

        assert_eq!(
            context.global_style().visuals.panel_fill,
            original.visuals.panel_fill
        );
    }

    fn contrast_ratio(foreground: egui::Color32, background: egui::Color32) -> f32 {
        let lighter = relative_luminance(foreground).max(relative_luminance(background));
        let darker = relative_luminance(foreground).min(relative_luminance(background));
        (lighter + 0.05) / (darker + 0.05)
    }

    fn relative_luminance(color: egui::Color32) -> f32 {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }
}
