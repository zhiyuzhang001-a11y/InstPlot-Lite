use eframe::egui::{self, containers::scroll_area::ScrollBarVisibility};

use crate::processing::Anchor;
use crate::{
    data::DataSet,
    processing::{self, ProcessingMetadata, ProcessingOperation, ProcessingResult},
};

use super::{
    formatting::{anchor_name, compact_label},
    theme::{FG_SECONDARY, SPACE_MD, SPACE_SM, ShellButtonStyle, shell_button},
    tool_window::{
        apply_tool_window_surface, processing_viewport_id, show_embedded_window_close_control,
        show_tool_viewport,
    },
};

pub(crate) struct ProcessingSettings {
    pub(crate) scope: ProcessingScope,
    pub(crate) selected_datasets: Vec<bool>,
    pub(crate) result_mode: ProcessingResultMode,
    pub(crate) fit_min: f64,
    pub(crate) fit_max: f64,
    pub(crate) background_order: usize,
    pub(crate) linear_left: [f64; 2],
    pub(crate) linear_right: [f64; 2],
    pub(crate) linear_zero: bool,
    pub(crate) linear_auto: bool,
    pub(crate) linear_preview: Option<LinearPreview>,
    linear_error: String,
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
            background_order: 1,
            linear_left: [0.0, 0.05],
            linear_right: [0.95, 1.0],
            linear_zero: false,
            linear_auto: true,
            linear_preview: None,
            linear_error: String::new(),
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
    AutoLinearBackground {
        zero_baseline: bool,
    },
    Center,
    CenterNormalize,
    LinearBackground {
        left: [f64; 2],
        right: [f64; 2],
        zero_baseline: bool,
    },
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
    dataset: Option<&DataSet>,
    x_column: usize,
    y_column: usize,
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
            .with_title(format!(
                "InstPlot Lite · 数据处理 — {}",
                dataset_names
                    .get(active_dataset)
                    .map(String::as_str)
                    .unwrap_or("无数据")
            ))
            .with_inner_size([570.0, 570.0])
            .with_min_inner_size([520.0, 500.0])
            .with_resizable(true),
        |ui, viewport_class| {
            apply_tool_window_surface(ui);
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
                    let available_width = ui.available_width();
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::same(SPACE_MD as i8))
                        .show(ui, |ui| {
                        ui.set_min_width((available_width - 2.0 * SPACE_MD).max(0.0));
                        ui.spacing_mut().item_spacing.y = SPACE_SM;
                        section_heading(ui, "处理目标");
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
                        secondary_text(
                            ui,
                            "此设置适用于本窗口全部操作和所有选中曲线；覆盖操作仍可撤销。",
                        );
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
                                let response = egui::ComboBox::from_id_salt("processing-dataset")
                                    .width(300.0)
                                    .selected_text(
                                        dataset_names
                                            .get(selected_dataset)
                                            .map(|name| compact_label(name, 28))
                                            .unwrap_or_else(|| "未选择".to_owned()),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (index, name) in dataset_names.iter().enumerate() {
                                            ui.selectable_value(
                                                &mut selected_dataset,
                                                index,
                                                compact_label(name, 48),
                                            )
                                            .on_hover_text(name);
                                        }
                                    })
                                    .response;
                                if let Some(name) = dataset_names.get(selected_dataset) {
                                    response.on_hover_text(name);
                                }
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
                                ui.checkbox(
                                    &mut settings.selected_datasets[index],
                                    compact_label(name, 44),
                                )
                                .on_hover_text(name);
                            }
                        }
                        secondary_text(
                            ui,
                            "结果写入方式由上方全局设置决定；批量处理可一次撤销。",
                        );
                        begin_section(ui, "基础处理");
                        ui.horizontal(|ui| {
                            if shell_button(ui, "对称处理", ShellButtonStyle::SecondaryStrong)
                                .clicked()
                            {
                                requested = Some(ProcessingAction::Center);
                            }
                            if shell_button(ui, "归一化", ShellButtonStyle::SecondaryStrong)
                                .clicked()
                            {
                                requested = Some(ProcessingAction::CenterNormalize);
                            }
                        });
                        secondary_text(
                            ui,
                            "归一化沿用原版：先对称，再取最高 20 个有限值的均值。",
                        );

                        if selected_dataset == active_dataset
                            && let Some(action) = linear_background_controls(ui, settings, dataset, x_column, y_column, active_dataset) { requested = Some(action); }

                        begin_section(ui, "高级：区间多项式去背景");
                        secondary_text(ui, "用所选区间全部点拟合，再对全曲线相减。1阶是直线；含峰谷的区间可能把真实信号算进背景，高阶不一定更好。");
                        ui.horizontal(|ui| {
                            ui.label("拟合 X：");
                            ui.add(egui::DragValue::new(&mut settings.fit_min));
                            ui.label("至");
                            ui.add(egui::DragValue::new(&mut settings.fit_max));
                            ui.label("阶数");
                            ui.add(
                                egui::DragValue::new(&mut settings.background_order).range(0..=5),
                            );
                            if shell_button(ui, "执行", ShellButtonStyle::SecondaryStrong).clicked() {
                                requested = Some(ProcessingAction::PolynomialBackground {
                                    fit_min: settings.fit_min,
                                    fit_max: settings.fit_max,
                                    order: settings.background_order,
                                });
                            }
                        });

                        begin_section(ui, "局部展平");
                        secondary_text(
                            ui,
                            "去除指定 X 区间的线性倾斜，使该段接近水平；锚点位置保持不变。",
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
                            if shell_button(ui, "执行", ShellButtonStyle::SecondaryStrong)
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

                        begin_section(ui, "Savitzky–Golay 去噪");
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
                        if shell_button(ui, "执行去噪", ShellButtonStyle::SecondaryStrong).clicked()
                        {
                            requested = Some(ProcessingAction::Denoise {
                                window_length: settings.denoise_window,
                                polyorder: settings.denoise_order,
                                range: settings
                                    .denoise_in_range
                                    .then_some((settings.denoise_min, settings.denoise_max)),
                            });
                        }

                        begin_section(ui, "公式计算");
                        secondary_text(
                            ui,
                            "按行计算：含 x 的公式生成新 X 列，含 y 的公式生成新 Y 列；a、b 是下方参数。",
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
                            if shell_button(ui, "执行公式", ShellButtonStyle::SecondaryStrong)
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
                        secondary_text(
                            ui,
                            "公式和系数支持 + − × ÷ ^、括号、sin、cos、tan、exp、ln/log、sqrt、abs、arctan，以及 pi、e。",
                        );
                    });
                });
        },
    );

    ProcessingWindowResponse {
        open,
        active_dataset: selected_dataset,
        action: requested,
    }
}

fn section_heading(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).strong());
}

impl ProcessingSettings {
    pub(crate) fn set_linear_defaults(&mut self, min: f64, max: f64) {
        let width = (max - min) * 0.05;
        self.linear_left = [min, min + width];
        self.linear_right = [max - width, max];
        self.linear_preview = None;
        self.linear_error.clear();
    }
}

pub(crate) struct LinearPreview {
    automatic: bool,
    target: String,
    x_column: usize,
    y_column: usize,
    x_name: String,
    y_name: String,
    x: Vec<f64>,
    y: Vec<f64>,
    alive: Vec<bool>,
    left: [f64; 2],
    right: [f64; 2],
    zero: bool,
    result: ProcessingResult,
}

impl LinearPreview {
    fn build(
        data: &DataSet,
        x_column: usize,
        y_column: usize,
        settings: &ProcessingSettings,
    ) -> Result<Self, String> {
        let x = data.columns.get(x_column).ok_or("X列不存在")?;
        let y = data.columns.get(y_column).ok_or("Y列不存在")?;
        let result = processing::apply_to_dataset(
            data,
            y_column,
            &if settings.linear_auto {
                ProcessingOperation::AutoLinearBackground {
                    x_column,
                    zero_baseline: settings.linear_zero,
                }
            } else {
                ProcessingOperation::LinearBackground {
                    x_column,
                    left: settings.linear_left,
                    right: settings.linear_right,
                    zero_baseline: settings.linear_zero,
                }
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            automatic: settings.linear_auto,
            target: data.plot_id.clone(),
            x_column,
            y_column,
            x_name: x.name.clone(),
            y_name: y.name.clone(),
            x: x.values.clone(),
            y: y.values.clone(),
            alive: data.alive.clone(),
            left: settings.linear_left,
            right: settings.linear_right,
            zero: settings.linear_zero,
            result,
        })
    }
    fn matches(&self, data: &DataSet, x: usize, y: usize, settings: &ProcessingSettings) -> bool {
        let same = |a: &[f64], b: &[f64]| {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits())
        };
        self.target == data.plot_id
            && self.x_column == x
            && self.y_column == y
            && self.alive == data.alive
            && self.automatic == settings.linear_auto
            && (self.automatic
                || (self.left == settings.linear_left && self.right == settings.linear_right))
            && self.zero == settings.linear_zero
            && data
                .columns
                .get(x)
                .is_some_and(|c| c.name == self.x_name && same(&self.x, &c.values))
            && data
                .columns
                .get(y)
                .is_some_and(|c| c.name == self.y_name && same(&self.y, &c.values))
    }
}

fn linear_background_controls(
    ui: &mut egui::Ui,
    settings: &mut ProcessingSettings,
    dataset: Option<&DataSet>,
    x: usize,
    y: usize,
    active_dataset: usize,
) -> Option<ProcessingAction> {
    use egui_plot::{Legend, Line, Plot, Points};
    begin_section(ui, "智能线性去背景（无需分段）");
    secondary_text(
        ui,
        "自动联合估计周期信号与线性漂移，只扣除直线背景。适合周期曲线；特征不可靠时不处理。先看预览，再决定是否应用。",
    );
    egui::CollapsingHeader::new("高级：手动参考区（可选）").show(ui, |ui| {
        ui.checkbox(
            &mut settings.linear_auto,
            "自动识别（默认，无需选择参考区）",
        );
        if !settings.linear_auto {
            secondary_text(
                ui,
                "选两个信号应相同的参考区，以区间中位数连线；周期曲线应选同相位/同状态。",
            );
            ui.horizontal(|ui| {
                ui.label("左参考 X：");
                ui.add(egui::DragValue::new(&mut settings.linear_left[0]));
                ui.label("至");
                ui.add(egui::DragValue::new(&mut settings.linear_left[1]));
            });
            ui.horizontal(|ui| {
                ui.label("右参考 X：");
                ui.add(egui::DragValue::new(&mut settings.linear_right[0]));
                ui.label("至");
                ui.add(egui::DragValue::new(&mut settings.linear_right[1]));
            });
        }
    });
    ui.checkbox(
        &mut settings.linear_zero,
        "同时减去常数背景（归零）；不勾选则仅去斜率，保留左侧基准",
    );
    secondary_text(
        ui,
        "自动估计不等于物理背景已确定。批量时每条曲线独立识别，任一条不可靠则整批不改；此处只预览当前曲线。",
    );
    if shell_button(
        ui,
        "智能预览（不改数据）",
        ShellButtonStyle::SecondaryStrong,
    )
    .clicked()
    {
        match dataset
            .ok_or_else(|| "请先导入数据".to_owned())
            .and_then(|d| LinearPreview::build(d, x, y, settings))
        {
            Ok(p) => {
                settings.linear_preview = Some(p);
                settings.linear_error.clear();
            }
            Err(e) => {
                settings.linear_preview = None;
                settings.linear_error = e;
            }
        }
    }
    if !settings.linear_error.is_empty() {
        ui.colored_label(egui::Color32::YELLOW, &settings.linear_error);
    }
    let preview = settings
        .linear_preview
        .as_ref()
        .filter(|p| dataset.is_some_and(|d| p.matches(d, x, y, settings)));
    if let Some(p) = preview {
        let (left_anchor, right_anchor, slope, counts, automatic) = match &p.result.metadata {
            ProcessingMetadata::AutoLinearBackground {
                slope,
                anchor_x,
                baseline,
                period,
                noise,
                span,
                points,
            } => {
                ui.label(format!(
                    "识别周期：{period:.4}（原 X 单位）；斜率：{slope:.6e}；{points} 个有效点"
                ));
                ui.label(format!(
                    "全程漂移：{:.6e}；残差尺度：{noise:.3e}",
                    slope * span
                ));
                if (slope * span).abs() < 2.0 * noise {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "漂移不明显，建议保留原数据，不必强行去背景。",
                    );
                }
                (
                    [*anchor_x, *baseline],
                    [anchor_x + span, baseline + slope * span],
                    *slope,
                    [*points, *points],
                    true,
                )
            }
            ProcessingMetadata::LinearBackground {
                left_anchor,
                right_anchor,
                counts,
                slope,
                ..
            } => (*left_anchor, *right_anchor, *slope, *counts, false),
            _ => unreachable!(),
        };
        if !automatic {
            ui.label(format!(
                "参考点数：{} / {}；斜率：{slope:.6e}",
                counts[0], counts[1]
            ));
            if counts.iter().any(|n| *n < 3) {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "参考点过少，易受单点噪声影响；建议适当扩大区间。",
                );
            }
        }
        let points = |values: &[f64]| -> Vec<[f64; 2]> {
            p.x.iter()
                .zip(values)
                .enumerate()
                .filter(|(i, (x, y))| {
                    (p.alive.is_empty() || p.alive[*i]) && x.is_finite() && y.is_finite()
                })
                .step_by(p.x.len().div_ceil(10_000).max(1))
                .map(|(_, (&x, &y))| [x, y])
                .collect()
        };
        Plot::new("linear-background-original")
            .height(140.0)
            .legend(Legend::default())
            .show(ui, |plot| {
                plot.line(Line::new("原曲线（未修改）", points(&p.y)));
                let baseline =
                    p.x.iter()
                        .map(|x| left_anchor[1] + slope * (x - left_anchor[0]))
                        .collect::<Vec<_>>();
                plot.line(
                    Line::new("估计线性背景", points(&baseline)).color(egui::Color32::YELLOW),
                );
                if !automatic {
                    plot.points(
                        Points::new("参考区中位数", vec![left_anchor, right_anchor]).radius(5.0),
                    );
                }
            });
        Plot::new("linear-background-corrected")
            .height(140.0)
            .legend(Legend::default())
            .show(ui, |plot| {
                plot.line(
                    Line::new("处理后预览（尚未应用）", points(&p.result.values))
                        .color(egui::Color32::LIGHT_BLUE),
                );
            });
        let included = settings.scope == ProcessingScope::Current
            || settings
                .selected_datasets
                .get(active_dataset)
                .copied()
                .unwrap_or(false);
        if !included {
            ui.colored_label(
                egui::Color32::YELLOW,
                "当前预览曲线未勾选，先选中它再应用，避免预览与处理目标不同。",
            );
        }
        if ui
            .add_enabled(included, egui::Button::new("确认应用线性去背景（可撤销）"))
            .clicked()
        {
            if p.automatic {
                return Some(ProcessingAction::AutoLinearBackground {
                    zero_baseline: settings.linear_zero,
                });
            }
            return Some(ProcessingAction::LinearBackground {
                left: settings.linear_left,
                right: settings.linear_right,
                zero_baseline: settings.linear_zero,
            });
        }
    } else if settings.linear_preview.is_some() {
        secondary_text(ui, "数据、选列或参数已改变，请重新预览后再应用。");
    }
    None
}

fn begin_section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(SPACE_MD);
    ui.separator();
    ui.add_space(SPACE_SM / 2.0);
    section_heading(ui, title);
}

fn secondary_text(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(FG_SECONDARY));
}

#[cfg(test)]
mod linear_preview_tests {
    use super::*;
    #[test]
    fn preview_does_not_modify_data_and_cannot_apply_to_changed_values_columns_or_settings() {
        let mut data = DataSet {
            source: "preview.csv".into(),
            label: None,
            kind: crate::data::DataSetKind::Source,
            plot_id: "preview".into(),
            fit_link: None,
            encoding: "utf8".into(),
            separator: ",".into(),
            row_count: 4,
            alive: vec![true; 4],
            columns: vec![
                crate::data::NumericColumn {
                    name: "x".into(),
                    values: vec![0.0, 1.0, 9.0, 10.0],
                },
                crate::data::NumericColumn {
                    name: "y".into(),
                    values: vec![5.0, 7.0, 23.0, 25.0],
                },
            ],
        };
        let mut settings = ProcessingSettings {
            linear_auto: false,
            linear_left: [0.0, 1.0],
            linear_right: [9.0, 10.0],
            ..Default::default()
        };
        let p = LinearPreview::build(&data, 0, 1, &settings).unwrap();
        assert!(p.matches(&data, 0, 1, &settings));
        assert_eq!(data.columns[1].values, vec![5.0, 7.0, 23.0, 25.0]);
        data.columns[1].values[0] += 1.0;
        assert!(!p.matches(&data, 0, 1, &settings));
        data.columns[1].values[0] -= 1.0;
        assert!(!p.matches(&data, 1, 0, &settings));
        settings.linear_zero = true;
        assert!(!p.matches(&data, 0, 1, &settings));
        settings.linear_zero = false;
        data.alive[0] = false;
        assert!(!p.matches(&data, 0, 1, &settings));
    }

    #[test]
    fn automatic_is_default_and_preview_requires_same_data_and_mode() {
        let mut settings = ProcessingSettings::default();
        assert!(settings.linear_auto);
        let mut data = DataSet {
            source: "auto.csv".into(),
            label: None,
            kind: crate::data::DataSetKind::Source,
            plot_id: "auto".into(),
            fit_link: None,
            encoding: "utf8".into(),
            separator: ",".into(),
            row_count: 361,
            alive: vec![true; 361],
            columns: vec![
                crate::data::NumericColumn {
                    name: "x".into(),
                    values: (0..361).map(|i| i as f64).collect(),
                },
                crate::data::NumericColumn {
                    name: "y".into(),
                    values: (0..361)
                        .map(|i| {
                            (i as f64 * std::f64::consts::TAU / 180.0).sin() + 0.003 * i as f64
                        })
                        .collect(),
                },
            ],
        };
        let original = data.columns[1].values.clone();
        let p = LinearPreview::build(&data, 0, 1, &settings).unwrap();
        assert_eq!(data.columns[1].values, original);
        assert!(p.matches(&data, 0, 1, &settings));
        settings.linear_left = [50.0, 60.0];
        assert!(p.matches(&data, 0, 1, &settings));
        settings.linear_auto = false;
        assert!(!p.matches(&data, 0, 1, &settings));
        settings.linear_auto = true;
        data.columns[1].values[0] += 1.0;
        assert!(!p.matches(&data, 0, 1, &settings));
    }
}
