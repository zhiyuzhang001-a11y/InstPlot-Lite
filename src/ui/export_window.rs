use eframe::egui::{self, containers::scroll_area::ScrollBarVisibility};

use super::{
    formatting::compact_label,
    selection::sole_selected_index,
    theme::{
        FG_SECONDARY, PANEL_BG, SPACE_LG, SPACE_MD, SPACE_SM, ShellButtonStyle,
        shell_button_enabled,
    },
    tool_window::{
        apply_tool_window_surface, export_viewport_id, show_embedded_window_close_control,
        show_tool_viewport,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportLayout {
    Combined,
    Separate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataExportFormat {
    Csv,
    Xlsx,
    Tsv,
    Txt,
    Dat,
}

impl DataExportFormat {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Xlsx => "Excel（XLSX）",
            Self::Tsv => "TSV",
            Self::Txt => "TXT",
            Self::Dat => "DAT",
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Tsv => "tsv",
            Self::Txt => "txt",
            Self::Dat => "dat",
        }
    }

    pub(crate) fn filter_name(self) -> &'static str {
        match self {
            Self::Csv => "CSV 数据",
            Self::Xlsx => "Excel 工作簿",
            Self::Tsv => "TSV 数据",
            Self::Txt => "TXT 数据",
            Self::Dat => "DAT 数据",
        }
    }
}

pub(crate) struct ExportSelection {
    pub(crate) format: DataExportFormat,
    pub(crate) datasets: Vec<bool>,
    pub(crate) layout: ExportLayout,
    pub(crate) column_dataset: Option<usize>,
    pub(crate) columns: Vec<bool>,
}

pub(crate) struct ExportWindowData<'a> {
    pub(crate) active_dataset: usize,
    pub(crate) dataset_names: &'a [String],
    pub(crate) minimum_columns: &'a [usize],
    pub(crate) column_names: &'a [String],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportAction {
    None,
    Export,
    Close,
}

pub(crate) fn show(
    context: &egui::Context,
    settings: &mut ExportSelection,
    data: ExportWindowData<'_>,
) -> ExportAction {
    let mut open = true;
    let mut export = false;
    let format_label = settings.format.label();
    show_tool_viewport(
        context,
        export_viewport_id(),
        egui::ViewportBuilder::default()
            .with_title(format!("InstPlot Lite · 导出 {format_label}"))
            .with_inner_size([470.0, 620.0])
            .with_min_inner_size([380.0, 360.0])
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
            let body_height = (ui.available_height() - 54.0).max(120.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), body_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("export-window-scroll")
                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                        .scroll_source(egui::scroll_area::ScrollSource::ALL)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Frame::NONE
                                .inner_margin(egui::Margin::same(SPACE_MD as i8))
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = SPACE_SM;
                                    section_heading(ui, "1  选择数据");
                                    ui.label(
                                        egui::RichText::new("选择要导出的数据集或曲线。")
                                            .color(FG_SECONDARY),
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            "实时拟合结果会随其关联数据集一起导出。",
                                        )
                                        .color(FG_SECONDARY),
                                    );
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("当前").clicked() {
                                            settings.datasets.fill(false);
                                            if let Some(value) =
                                                settings.datasets.get_mut(data.active_dataset)
                                            {
                                                *value = true;
                                            }
                                        }
                                        if ui.button("全选").clicked() {
                                            settings.datasets.fill(true);
                                        }
                                        if ui.button("全不选").clicked() {
                                            settings.datasets.fill(false);
                                        }
                                    });
                                    egui::ScrollArea::vertical()
                                        .id_salt("export-dataset-list-scroll")
                                        .max_height(150.0)
                                        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                                        .auto_shrink([false, true])
                                        .show(ui, |ui| {
                                            for (index, name) in
                                                data.dataset_names.iter().enumerate()
                                            {
                                                if let Some(selected) =
                                                    settings.datasets.get_mut(index)
                                                {
                                                    let marker = if index == data.active_dataset {
                                                        "▶ "
                                                    } else {
                                                        ""
                                                    };
                                                    ui.checkbox(
                                                        selected,
                                                        format!(
                                                            "{marker}{}",
                                                            compact_label(name, 38)
                                                        ),
                                                    )
                                                    .on_hover_text(name);
                                                }
                                            }
                                        });

                                    let selected_count = selected_dataset_count(settings);
                                    if selected_count > 1 {
                                        ui.add_space(SPACE_LG);
                                        section_heading(ui, "2  保存方式");
                                        ui.radio_value(
                                            &mut settings.layout,
                                            ExportLayout::Combined,
                                            if settings.format == DataExportFormat::Xlsx {
                                                "同一个工作簿（每个数据集一个工作表）"
                                            } else {
                                                "同一个分区文件（BEGIN/END）"
                                            },
                                        );
                                        ui.radio_value(
                                            &mut settings.layout,
                                            ExportLayout::Separate,
                                            "多个独立文件",
                                        );
                                        ui.label(
                                            egui::RichText::new(
                                                "多数据集导出会保留各自全部列和相关拟合结果。",
                                            )
                                            .color(FG_SECONDARY),
                                        );
                                    }

                                    if sole_selected_index(&settings.datasets).is_some() {
                                        ui.add_space(SPACE_LG);
                                        section_heading(ui, "2  选择列");
                                        ui.horizontal(|ui| {
                                            if ui.button("全选列").clicked() {
                                                settings.columns.fill(true);
                                            }
                                            if ui.button("全不选列").clicked() {
                                                settings.columns.fill(false);
                                            }
                                        });
                                        egui::ScrollArea::vertical()
                                            .id_salt("export-column-list-scroll")
                                            .max_height(180.0)
                                            .scroll_bar_visibility(
                                                ScrollBarVisibility::AlwaysVisible,
                                            )
                                            .auto_shrink([false, true])
                                            .show(ui, |ui| {
                                                for (index, name) in
                                                    data.column_names.iter().enumerate()
                                                {
                                                    if let Some(selected) =
                                                        settings.columns.get_mut(index)
                                                    {
                                                        ui.checkbox(
                                                            selected,
                                                            format!("{}：{name}", index + 1),
                                                        );
                                                    }
                                                }
                                            });
                                    }
                                });
                        });
                },
            );

            let selected_count = selected_dataset_count(settings);
            let sole = sole_selected_index(&settings.datasets);
            let selected_columns = settings
                .columns
                .iter()
                .filter(|selected| **selected)
                .count();
            let minimum = sole
                .and_then(|index| data.minimum_columns.get(index).copied())
                .unwrap_or(1);
            let can_export = sole.map_or(selected_count > 0, |_| selected_columns >= minimum);
            let summary = if sole.is_some() {
                format!("已选 {selected_columns} 列")
            } else {
                format!("已选 {selected_count} 个数据集")
            };
            egui::Frame::NONE
                .fill(PANEL_BG)
                .inner_margin(egui::Margin::symmetric(SPACE_MD as i8, SPACE_SM as i8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(summary).color(FG_SECONDARY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if shell_button_enabled(
                                ui,
                                can_export,
                                "导出",
                                ShellButtonStyle::Primary,
                            )
                            .on_disabled_hover_text(export_disabled_reason(sole, minimum))
                            .clicked()
                            {
                                export = true;
                            }
                        });
                    });
                });
        },
    );

    if export {
        ExportAction::Export
    } else if !open {
        ExportAction::Close
    } else {
        ExportAction::None
    }
}

fn section_heading(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).strong());
}

fn selected_dataset_count(settings: &ExportSelection) -> usize {
    settings
        .datasets
        .iter()
        .filter(|selected| **selected)
        .count()
}

fn export_disabled_reason(sole: Option<usize>, minimum: usize) -> &'static str {
    if sole.is_none() {
        "请至少选择一个数据集"
    } else if minimum == 2 {
        "该数据包含拟合结果，至少选择两列才能重新导入"
    } else {
        "请至少选择一列"
    }
}
