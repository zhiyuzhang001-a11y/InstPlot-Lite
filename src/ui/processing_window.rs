use eframe::egui::{self, containers::scroll_area::ScrollBarVisibility};

use crate::processing::Anchor;

use super::{
    formatting::anchor_name,
    theme::SPACE_MD,
    tool_window::{processing_viewport_id, show_embedded_window_close_control, show_tool_viewport},
};

pub(crate) struct ProcessingSettings {
    pub(crate) scope: ProcessingScope,
    pub(crate) selected_datasets: Vec<bool>,
    pub(crate) result_mode: ProcessingResultMode,
    pub(crate) fit_min: f64,
    pub(crate) fit_max: f64,
    pub(crate) background_order: usize,
    pub(crate) local_min: f64,
    pub(crate) local_max: f64,
    pub(crate) local_transition: f64,
    pub(crate) local_anchor: Anchor,
    pub(crate) local_strength: f64,
    pub(crate) denoise_window: usize,
    pub(crate) denoise_order: usize,
    pub(crate) denoise_in_range: bool,
    pub(crate) denoise_min: f64,
    pub(crate) denoise_max: f64,
    pub(crate) formula: String,
    pub(crate) formula_a: String,
    pub(crate) formula_b: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessingScope {
    Current,
    Selected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessingResultMode {
    Overwrite,
    Retain,
}

impl Default for ProcessingSettings {
    fn default() -> Self {
        Self {
            scope: ProcessingScope::Current,
            selected_datasets: Vec::new(),
            result_mode: ProcessingResultMode::Overwrite,
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
            formula: "a * y + b".to_owned(),
            formula_a: "1".to_owned(),
            formula_b: "0".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProcessingAction {
    Center,
    CenterNormalize,
    PolynomialBackground {
        fit_min: f64,
        fit_max: f64,
        order: usize,
    },
    LocalFlatten {
        x1: f64,
        x2: f64,
        transition: f64,
        anchor: Anchor,
        strength: f64,
    },
    Denoise {
        window_length: usize,
        polyorder: usize,
        range: Option<(f64, f64)>,
    },
    Formula {
        expression: String,
        a: String,
        b: String,
    },
}

pub(crate) struct ProcessingWindowResponse {
    pub(crate) open: bool,
    pub(crate) active_dataset: usize,
    pub(crate) action: Option<ProcessingAction>,
}

pub(crate) fn show(
    context: &egui::Context,
    settings: &mut ProcessingSettings,
    dataset_names: &[String],
    active_dataset: usize,
) -> ProcessingWindowResponse {
    let mut open = true;
    let mut selected_dataset = active_dataset;
    let mut requested = None;
    if settings.selected_datasets.len() != dataset_names.len() {
        settings.selected_datasets = (0..dataset_names.len())
            .map(|index| index == active_dataset)
            .collect();
    }

    show_tool_viewport(
        context,
        processing_viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("InstPlot Lite · 数据处理")
            .with_inner_size([570.0, 570.0])
            .with_min_inner_size([520.0, 500.0])
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
                .id_salt("processing-window-scroll")
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .scroll_source(egui::scroll_area::ScrollSource::ALL)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(SPACE_MD);
                    ui.indent("processing-content", |ui| {
                        ui.spacing_mut().item_spacing.y = SPACE_MD;
                        ui.horizontal(|ui| {
                            ui.label("结果写入：");
                            ui.selectable_value(
                                &mut settings.result_mode,
                                ProcessingResultMode::Overwrite,
                                "覆盖原列",
                            );
                            ui.selectable_value(
                                &mut settings.result_mode,
                                ProcessingResultMode::Retain,
                                "保留派生列",
                            );
                        });
                        ui.small("此设置适用于本窗口全部操作和所有选中曲线；覆盖操作仍可撤销。");
                        ui.horizontal(|ui| {
                            ui.label("处理范围：");
                            ui.selectable_value(
                                &mut settings.scope,
                                ProcessingScope::Current,
                                "当前曲线",
                            );
                            ui.selectable_value(
                                &mut settings.scope,
                                ProcessingScope::Selected,
                                "选择曲线",
                            );
                        });
                        if settings.scope == ProcessingScope::Current {
                            ui.horizontal(|ui| {
                                ui.label("当前曲线：");
                                egui::ComboBox::from_id_salt("processing-dataset")
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
                        }
                        ui.label("结果写入方式由上方全局设置决定；批量处理可一次撤销。");
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("对称处理").clicked() {
                                requested = Some(ProcessingAction::Center);
                            }
                            if ui.button("归一化").clicked() {
                                requested = Some(ProcessingAction::CenterNormalize);
                            }
                            ui.small("归一化沿用原版：先对称，再取最高 20 个有限值的均值");
                        });

                        ui.separator();
                        ui.strong("去背底（多项式）");
                        ui.horizontal(|ui| {
                            ui.label("拟合 X：");
                            ui.add(egui::DragValue::new(&mut settings.fit_min));
                            ui.label("至");
                            ui.add(egui::DragValue::new(&mut settings.fit_max));
                            ui.label("阶数");
                            ui.add(
                                egui::DragValue::new(&mut settings.background_order).range(0..=5),
                            );
                            if ui.button(egui::RichText::new("执行").strong()).clicked() {
                                requested = Some(ProcessingAction::PolynomialBackground {
                                    fit_min: settings.fit_min,
                                    fit_max: settings.fit_max,
                                    order: settings.background_order,
                                });
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
                            ui.add(egui::DragValue::new(&mut settings.local_min));
                            ui.label("至");
                            ui.add(egui::DragValue::new(&mut settings.local_max));
                            ui.label("过渡").on_hover_text(
                                "在区间两侧逐渐应用修正，减小边缘折角；0 表示不过渡",
                            );
                            ui.add(
                                egui::DragValue::new(&mut settings.local_transition)
                                    .range(0.0..=f64::INFINITY),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("锚点")
                                .on_hover_text("这个位置的 Y 值保持不变，可选择左端、右端或中心");
                            egui::ComboBox::from_id_salt("local-anchor")
                                .selected_text(anchor_name(settings.local_anchor))
                                .show_ui(ui, |ui| {
                                    for anchor in [Anchor::Left, Anchor::Right, Anchor::Center] {
                                        ui.selectable_value(
                                            &mut settings.local_anchor,
                                            anchor,
                                            anchor_name(anchor),
                                        );
                                    }
                                });
                            ui.label("强度")
                                .on_hover_text("1.0 表示完全去除拟合斜率，0.5 表示修正一半");
                            ui.add(
                                egui::Slider::new(&mut settings.local_strength, 0.0..=1.0)
                                    .show_value(true),
                            );
                            if ui
                                .button(egui::RichText::new("执行").strong())
                                .on_hover_text("按上方结果写入方式应用局部展平")
                                .clicked()
                            {
                                requested = Some(ProcessingAction::LocalFlatten {
                                    x1: settings.local_min,
                                    x2: settings.local_max,
                                    transition: settings.local_transition,
                                    anchor: settings.local_anchor,
                                    strength: settings.local_strength,
                                });
                            }
                        });

                        ui.separator();
                        ui.strong("Savitzky–Golay 去噪");
                        ui.horizontal(|ui| {
                            ui.label("窗口");
                            ui.add(
                                egui::DragValue::new(&mut settings.denoise_window)
                                    .range(3..=999)
                                    .speed(2),
                            );
                            ui.label("阶数");
                            ui.add(
                                egui::DragValue::new(&mut settings.denoise_order).range(0..=9),
                            );
                            ui.checkbox(&mut settings.denoise_in_range, "仅处理 X 区间");
                        });
                        if settings.denoise_in_range {
                            ui.horizontal(|ui| {
                                ui.label("X：");
                                ui.add(egui::DragValue::new(&mut settings.denoise_min));
                                ui.label("至");
                                ui.add(egui::DragValue::new(&mut settings.denoise_max));
                            });
                        }
                        if ui
                            .button(egui::RichText::new("执行去噪").strong())
                            .clicked()
                        {
                            requested = Some(ProcessingAction::Denoise {
                                window_length: settings.denoise_window,
                                polyorder: settings.denoise_order,
                                range: settings
                                    .denoise_in_range
                                    .then_some((settings.denoise_min, settings.denoise_max)),
                            });
                        }

                        ui.separator();
                        ui.strong("公式计算");
                        ui.label(
                            egui::RichText::new(
                                "按行计算：含 x 的公式生成新 X 列，含 y 的公式生成新 Y 列；a、b 是下方参数。",
                            )
                            .weak(),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for (label, formula) in [
                                ("a × y + b", "a * y + b"),
                                ("y + b", "y + b"),
                                ("a × y", "a * y"),
                                ("−y", "-y"),
                            ] {
                                if ui.button(label).clicked() {
                                    settings.formula = formula.to_owned();
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("公式：");
                            ui.add(
                                egui::TextEdit::singleline(&mut settings.formula)
                                    .desired_width(360.0)
                                    .hint_text("例如：a * y + b"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("a");
                            ui.add(
                                egui::TextEdit::singleline(&mut settings.formula_a)
                                    .desired_width(95.0)
                                    .hint_text("例如：10/11"),
                            );
                            ui.label("b");
                            ui.add(
                                egui::TextEdit::singleline(&mut settings.formula_b)
                                    .desired_width(95.0)
                                    .hint_text("例如：(2+3)/7"),
                            );
                            if ui
                                .button(egui::RichText::new("执行公式").strong())
                                .on_hover_text(
                                    "根据公式中的 x 或 y，按上方结果写入方式应用公式",
                                )
                                .clicked()
                            {
                                requested = Some(ProcessingAction::Formula {
                                    expression: settings.formula.clone(),
                                    a: settings.formula_a.clone(),
                                    b: settings.formula_b.clone(),
                                });
                            }
                        });
                        ui.small(
                            "公式和系数支持 + − × ÷ ^、括号、sin、cos、tan、exp、ln/log、sqrt、abs、arctan，以及 pi、e。",
                        );
                    });
                    ui.add_space(SPACE_MD);
                });
        },
    );

    ProcessingWindowResponse {
        open,
        active_dataset: selected_dataset,
        action: requested,
    }
}
