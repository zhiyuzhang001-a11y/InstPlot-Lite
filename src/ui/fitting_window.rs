use eframe::egui::{self, containers::scroll_area::ScrollBarVisibility};

use super::theme::{CONTROL_RADIUS, SPACE_MD, SPACE_SM, SPACE_XS};
use super::tool_window::{
    fitting_viewport_id, show_embedded_window_close_control, show_tool_viewport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FitKind {
    Polynomial,
    Exponential,
    Logarithmic,
    Power,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XUnitConversion {
    None,
    DegreesToRadians,
    RadiansToDegrees,
}

impl XUnitConversion {
    pub(crate) fn convert(self, value: f64) -> f64 {
        match self {
            Self::None => value,
            Self::DegreesToRadians => value.to_radians(),
            Self::RadiansToDegrees => value.to_degrees(),
        }
    }

    pub(crate) fn restore(self, value: f64) -> f64 {
        match self {
            Self::None => value,
            Self::DegreesToRadians => value.to_degrees(),
            Self::RadiansToDegrees => value.to_radians(),
        }
    }
}

pub(crate) struct FitSettings {
    pub(crate) scope: FitScope,
    pub(crate) selected_datasets: Vec<bool>,
    pub(crate) kind: FitKind,
    pub(crate) degree: usize,
    pub(crate) use_x_range: bool,
    pub(crate) x_min: f64,
    pub(crate) x_max: f64,
    pub(crate) use_y_range: bool,
    pub(crate) y_min: f64,
    pub(crate) y_max: f64,
    pub(crate) unit_conversion: XUnitConversion,
    pub(crate) expression: String,
    pub(crate) initial_parameters: String,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FitScope {
    Current,
    Selected,
}

impl Default for FitSettings {
    fn default() -> Self {
        Self {
            scope: FitScope::Current,
            selected_datasets: Vec::new(),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FitAction {
    None,
    Execute,
    Clear,
}

pub(crate) struct FitWindowResponse {
    pub(crate) open: bool,
    pub(crate) active_dataset: usize,
    pub(crate) action: FitAction,
}

pub(crate) fn show(
    context: &egui::Context,
    settings: &mut FitSettings,
    dataset_names: &[String],
    active_dataset: usize,
    has_fit_results: bool,
) -> FitWindowResponse {
    let mut open = true;
    let mut selected_dataset = active_dataset;
    let mut action = FitAction::None;
    if settings.selected_datasets.len() != dataset_names.len() {
        settings.selected_datasets = (0..dataset_names.len())
            .map(|index| index == active_dataset)
            .collect();
    }

    show_tool_viewport(
        context,
        fitting_viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("InstPlot Lite · 曲线拟合")
            .with_inner_size([620.0, 520.0])
            .with_min_inner_size([560.0, 470.0])
            .with_resizable(true),
        |ui, viewport_class| {
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                open = false;
                return;
            }
            if show_embedded_window_close_control(ui, viewport_class, &mut open) {
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("fitting-window-scroll")
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .scroll_source(egui::scroll_area::ScrollSource::ALL)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(SPACE_MD);
                    ui.indent("fit-content", |ui| {
                        ui.spacing_mut().item_spacing.y = SPACE_MD;
                        ui.label(
                            "使用当前 X/Y 列进行拟合；已删除和非数值数据点不会参与计算。",
                        );
                        ui.separator();
                        ui.small(
                            "同一数据集的同一组 X/Y 重新拟合时，会更新原拟合曲线；其他曲线的拟合结果会保留。",
                        );
                        ui.horizontal(|ui| {
                            ui.label("拟合范围：");
                            ui.selectable_value(
                                &mut settings.scope,
                                FitScope::Current,
                                "当前曲线",
                            );
                            ui.selectable_value(
                                &mut settings.scope,
                                FitScope::Selected,
                                "选择曲线",
                            );
                        });
                        if settings.scope == FitScope::Current {
                            ui.horizontal(|ui| {
                                ui.label("当前曲线：");
                                egui::ComboBox::from_id_salt("fit-dataset")
                                    .width(300.0)
                                    .selected_text(
                                        dataset_names
                                            .get(selected_dataset)
                                            .map(String::as_str)
                                            .unwrap_or("未选择"),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (index, name) in dataset_names.iter().enumerate() {
                                            ui.selectable_value(
                                                &mut selected_dataset,
                                                index,
                                                name,
                                            );
                                        }
                                    });
                            });
                        } else {
                            ui.horizontal(|ui| {
                                if ui.button("全选").clicked() {
                                    settings.selected_datasets.fill(true);
                                }
                                if ui.button("全不选").clicked() {
                                    settings.selected_datasets.fill(false);
                                }
                            });
                            for (index, name) in dataset_names.iter().enumerate() {
                                ui.checkbox(&mut settings.selected_datasets[index], name);
                            }
                            ui.small(
                                "所选曲线将分别拟合，不会合并数据点；使用当前 X/Y 列名匹配其他曲线。",
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.label("X 单位");
                            egui::ComboBox::from_id_salt("fit-unit")
                                .selected_text(unit_conversion_name(settings.unit_conversion))
                                .show_ui(ui, |ui| {
                                    for conversion in [
                                        XUnitConversion::None,
                                        XUnitConversion::DegreesToRadians,
                                        XUnitConversion::RadiansToDegrees,
                                    ] {
                                        ui.selectable_value(
                                            &mut settings.unit_conversion,
                                            conversion,
                                            unit_conversion_name(conversion),
                                        );
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut settings.use_x_range, "限制 X");
                            ui.add_enabled(
                                settings.use_x_range,
                                egui::DragValue::new(&mut settings.x_min),
                            );
                            ui.label("至");
                            ui.add_enabled(
                                settings.use_x_range,
                                egui::DragValue::new(&mut settings.x_max),
                            );
                            ui.checkbox(&mut settings.use_y_range, "限制 Y");
                            ui.add_enabled(
                                settings.use_y_range,
                                egui::DragValue::new(&mut settings.y_min),
                            );
                            ui.label("至");
                            ui.add_enabled(
                                settings.use_y_range,
                                egui::DragValue::new(&mut settings.y_max),
                            );
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("拟合类型");
                            egui::ComboBox::from_id_salt("fit-kind")
                                .selected_text(fit_kind_name(settings.kind))
                                .show_ui(ui, |ui| {
                                    for kind in [
                                        FitKind::Polynomial,
                                        FitKind::Exponential,
                                        FitKind::Logarithmic,
                                        FitKind::Power,
                                        FitKind::Custom,
                                    ] {
                                        ui.selectable_value(
                                            &mut settings.kind,
                                            kind,
                                            fit_kind_name(kind),
                                        );
                                    }
                                });
                            if settings.kind == FitKind::Polynomial {
                                ui.label("阶数");
                                ui.add(egui::DragValue::new(&mut settings.degree).range(1..=10));
                            }
                        });
                        if settings.kind == FitKind::Custom {
                            ui.add_space(SPACE_SM);
                            editable_fit_field(
                                ui,
                                "函数表达式（可编辑）",
                                "f(x) =",
                                &mut settings.expression,
                                "例如：a * sin(b * x + c)",
                            );
                            ui.add_space(SPACE_SM);
                            editable_fit_field(
                                ui,
                                "初始参数（可编辑）",
                                "a, b, c… =",
                                &mut settings.initial_parameters,
                                "例如：1, 10/11, (2+3)/7",
                            );
                            ui.small(
                                "参数按 a、b、c、d、e_param、f、g、h 的顺序填写，用逗号分隔；每项可用分数和括号。",
                            );
                            ui.small(
                                "支持 + - * / ^、sin、cos、tan、exp、ln/log、sqrt、abs。",
                            );
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new("执行拟合").strong())
                                .clicked()
                            {
                                action = FitAction::Execute;
                            }
                            if ui
                                .add_enabled(
                                    has_fit_results,
                                    egui::Button::new("清除全部拟合曲线"),
                                )
                                .clicked()
                            {
                                action = FitAction::Clear;
                            }
                        });
                        ui.add_space(SPACE_SM);
                        ui.label(&settings.message);
                    });
                    ui.add_space(SPACE_MD);
                });
        },
    );

    FitWindowResponse {
        open,
        active_dataset: selected_dataset,
        action,
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
    let accent = egui::Color32::from_gray(132);
    egui::Frame::new()
        .fill(egui::Color32::from_gray(31))
        .stroke(egui::Stroke::new(1.5, accent))
        .corner_radius(egui::CornerRadius::same(CONTROL_RADIUS))
        .inner_margin(egui::Margin::same(SPACE_MD as i8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .color(egui::Color32::from_gray(232)),
            );
            ui.add_space(SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(prefix).strong());
                ui.scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_gray(56);
                    ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.2, accent);
                    ui.visuals_mut().widgets.hovered.bg_stroke =
                        egui::Stroke::new(1.8, egui::Color32::from_gray(184));
                    ui.visuals_mut().widgets.active.bg_stroke =
                        egui::Stroke::new(2.0, egui::Color32::WHITE);
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
