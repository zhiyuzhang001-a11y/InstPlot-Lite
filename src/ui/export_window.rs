use eframe::egui::{self, containers::scroll_area::ScrollBarVisibility};

use super::{
    formatting::compact_label,
    selection::sole_selected_index,
    tool_window::{export_viewport_id, show_embedded_window_close_control, show_tool_viewport},
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
            if ui.ctx().input(|input| input.viewport().close_requested()) {
                open = false;
                return;
            }
            if show_embedded_window_close_control(ui, viewport_class, &mut open) {
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("export-window-scroll")
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
                .scroll_source(egui::scroll_area::ScrollSource::ALL)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(10.0);
                    ui.indent("export-content", |ui| {
                        ui.spacing_mut().item_spacing.y = 9.0;
                        ui.label("选择要导出的数据集或曲线。");
                        ui.small("实时拟合结果会随其关联数据集一起导出。");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("当前").clicked() {
                                settings.datasets.fill(false);
                                if let Some(value) = settings.datasets.get_mut(data.active_dataset)
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
                        for (index, name) in data.dataset_names.iter().enumerate() {
                            if let Some(selected) = settings.datasets.get_mut(index) {
                                let marker = if index == data.active_dataset {
                                    "▶ "
                                } else {
                                    ""
                                };
                                ui.checkbox(
                                    selected,
                                    format!("{marker}{}", compact_label(name, 38)),
                                )
                                .on_hover_text(name);
                            }
                        }

                        let selected_count = settings
                            .datasets
                            .iter()
                            .filter(|selected| **selected)
                            .count();
                        if selected_count > 1 {
                            ui.separator();
                            ui.label("保存方式");
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
                            ui.small("多数据集导出会保留各自全部列和相关拟合结果。");
                        }

                        let sole = sole_selected_index(&settings.datasets);
                        if let Some(dataset_index) = sole {
                            ui.separator();
                            ui.label("选择列");
                            ui.horizontal(|ui| {
                                if ui.button("全选列").clicked() {
                                    settings.columns.fill(true);
                                }
                                if ui.button("全不选列").clicked() {
                                    settings.columns.fill(false);
                                }
                            });
                            for (index, name) in data.column_names.iter().enumerate() {
                                if let Some(selected) = settings.columns.get_mut(index) {
                                    ui.checkbox(selected, format!("{}：{name}", index + 1));
                                }
                            }
                            let selected_columns = settings
                                .columns
                                .iter()
                                .filter(|selected| **selected)
                                .count();
                            let minimum = data
                                .minimum_columns
                                .get(dataset_index)
                                .copied()
                                .unwrap_or(1);
                            ui.horizontal(|ui| {
                                ui.label(format!("已选 {selected_columns} 列"));
                                if ui
                                    .add_enabled(
                                        selected_columns >= minimum,
                                        egui::Button::new("导出"),
                                    )
                                    .on_disabled_hover_text(if minimum == 2 {
                                        "该数据包含拟合结果，至少选择两列才能重新导入"
                                    } else {
                                        "请至少选择一列"
                                    })
                                    .clicked()
                                {
                                    export = true;
                                }
                            });
                        } else {
                            ui.separator();
                            if ui
                                .add_enabled(
                                    selected_count > 0,
                                    egui::Button::new(format!(
                                        "导出已选 {selected_count} 个数据集"
                                    )),
                                )
                                .on_disabled_hover_text("请至少选择一个数据集")
                                .clicked()
                            {
                                export = true;
                            }
                        }
                    });
                    ui.add_space(10.0);
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
