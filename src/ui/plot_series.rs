use eframe::egui::{self, Color32, Rect};
use egui_plot::{Line, PlotUi, Points};

use crate::{
    data::{DataSet, DataSetKind},
    session::FitOverlay,
};

use super::formatting::legend_series_name;

pub struct PendingHighlight<'a> {
    pub dataset_index: usize,
    pub rows: &'a [usize],
    pub x_column: usize,
    pub y_column: usize,
}

pub struct PlotSeriesInput<'a> {
    pub demo_points: &'a [[f64; 2]],
    pub datasets: &'a [DataSet],
    pub active_dataset: usize,
    pub plot_x_name: &'a str,
    pub plot_y_name: &'a str,
    pub last_plot_rect: Option<Rect>,
    pub visible_x_range: Option<[f64; 2]>,
    pub pending_highlight: Option<PendingHighlight<'a>>,
    pub fit_overlays: &'a [FitOverlay],
}

pub fn show(plot_ui: &mut PlotUi<'_>, input: PlotSeriesInput<'_>) -> Vec<egui::Id> {
    let mut plotted_series_ids = Vec::new();
    if input.datasets.is_empty() {
        let color = series_color(0);
        let series_name = "示例曲线";
        let series_id = egui::Id::new("demo-curve-series");
        plotted_series_ids.push(series_id);
        plot_ui.line(
            Line::new(series_name, input.demo_points.to_vec())
                .id(series_id)
                .color(color),
        );
        plot_ui.points(
            Points::new(series_name, input.demo_points.to_vec())
                .id(series_id)
                .color(color)
                .radius(3.5),
        );
        return plotted_series_ids;
    }

    for (dataset_index, dataset) in input.datasets.iter().enumerate() {
        let Some((x_column, y_column)) = dataset_plot_columns(
            input.datasets,
            dataset_index,
            input.active_dataset,
            input.plot_x_name,
            input.plot_y_name,
        ) else {
            continue;
        };
        let point_limit = input
            .last_plot_rect
            .map_or(2_000, |rect| (rect.width() as usize * 2).clamp(512, 20_000));
        let points = dataset.plot_points(x_column, y_column, point_limit, input.visible_x_range);
        if !points.is_empty() {
            let color = series_color(dataset_index);
            let is_active = dataset_index == input.active_dataset;
            let series_name = legend_series_name(&dataset.display_name(), is_active);
            let series_id = data_curve_series_id(dataset_index, &dataset.plot_id);
            plotted_series_ids.push(series_id);
            plot_ui.line(
                Line::new(series_name.clone(), points.clone())
                    .id(series_id)
                    .color(color)
                    .width(if is_active { 3.0 } else { 1.2 }),
            );
            plot_ui.points(
                Points::new(series_name, points)
                    .id(series_id)
                    .color(color)
                    .radius(if is_active { 4.5 } else { 2.5 }),
            );
        }
        if let Some(pending) = input
            .pending_highlight
            .as_ref()
            .filter(|pending| pending.dataset_index == dataset_index)
        {
            let highlighted: Vec<[f64; 2]> = pending
                .rows
                .iter()
                .step_by(pending.rows.len().div_ceil(5_000).max(1))
                .filter_map(|row_index| {
                    let x = dataset
                        .columns
                        .get(pending.x_column)?
                        .values
                        .get(*row_index)?;
                    let y = dataset
                        .columns
                        .get(pending.y_column)?
                        .values
                        .get(*row_index)?;
                    (x.is_finite() && y.is_finite()).then_some([*x, *y])
                })
                .collect();
            plot_ui.points(
                Points::new("待删除", highlighted)
                    .color(Color32::YELLOW)
                    .radius(5.0),
            );
        }
    }

    for (fit_index, fit) in input.fit_overlays.iter().enumerate().filter(|(_, fit)| {
        fit_overlay_matches_coordinates(fit, input.datasets, input.plot_x_name, input.plot_y_name)
    }) {
        let is_active = fit.target.dataset_index == Some(input.active_dataset);
        let fit_name = legend_series_name(&fit.name, is_active);
        let fit_id = egui::Id::new((
            "fit-curve-series",
            fit.target.dataset_index,
            fit.target.source_dataset_ids.as_slice(),
            fit.target.x_column_name.as_str(),
            fit.target.y_column_name.as_str(),
        ));
        plotted_series_ids.push(fit_id);
        plot_ui.line(
            Line::new(fit_name, fit.points.clone())
                .id(fit_id)
                .color(fit_color(fit_index))
                .width(if is_active { 3.5 } else { 1.8 }),
        );
    }
    plotted_series_ids
}

pub fn plot_coordinate_names(
    datasets: &[DataSet],
    active_dataset: usize,
    x_column: usize,
    y_column: usize,
) -> Option<(String, String)> {
    let active = datasets.get(active_dataset)?;
    if active.kind == DataSetKind::Fit
        && let Some(link) = &active.fit_link
    {
        return Some((link.source_x_column.clone(), link.source_y_column.clone()));
    }
    Some((
        active.columns.get(x_column)?.name.clone(),
        active.columns.get(y_column)?.name.clone(),
    ))
}

pub fn dataset_plot_columns(
    datasets: &[DataSet],
    dataset_index: usize,
    active_dataset: usize,
    x_name: &str,
    y_name: &str,
) -> Option<(usize, usize)> {
    let dataset = datasets.get(dataset_index)?;
    if dataset.kind == DataSetKind::Fit
        && let Some(link) = &dataset.fit_link
    {
        if link.source_x_column != x_name || link.source_y_column != y_name {
            return None;
        }
        let parent_is_loaded = link.parent_dataset_id.as_ref().is_none_or(|parent_id| {
            datasets.iter().any(|candidate| {
                candidate.kind == DataSetKind::Source
                    && candidate.plot_id == *parent_id
                    && candidate.columns.iter().any(|column| column.name == x_name)
                    && candidate.columns.iter().any(|column| column.name == y_name)
            })
        });
        return (dataset_index == active_dataset || parent_is_loaded)
            .then_some((0, 1))
            .filter(|(x, y)| {
                dataset.columns.get(*x).is_some() && dataset.columns.get(*y).is_some()
            });
    }

    let x_column = dataset
        .columns
        .iter()
        .position(|column| column.name == x_name)?;
    let y_column = dataset
        .columns
        .iter()
        .position(|column| column.name == y_name)?;
    Some((x_column, y_column))
}

pub fn plotted_axis_range(
    datasets: &[DataSet],
    active_dataset: usize,
    x_name: &str,
    y_name: &str,
    axis: usize,
) -> Option<[f64; 2]> {
    let mut combined: Option<[f64; 2]> = None;
    for (dataset_index, dataset) in datasets.iter().enumerate() {
        let Some((x_column, y_column)) =
            dataset_plot_columns(datasets, dataset_index, active_dataset, x_name, y_name)
        else {
            continue;
        };
        let Some(column) = dataset
            .columns
            .get(if axis == 0 { x_column } else { y_column })
        else {
            continue;
        };
        let Some([minimum, maximum]) = finite_range(&column.values) else {
            continue;
        };
        combined = Some(match combined {
            Some([current_minimum, current_maximum]) => {
                [current_minimum.min(minimum), current_maximum.max(maximum)]
            }
            None => [minimum, maximum],
        });
    }
    combined
}

pub fn data_curve_series_id(dataset_index: usize, plot_id: &str) -> egui::Id {
    egui::Id::new(("data-curve-series", dataset_index, plot_id))
}

pub fn fit_overlay_matches_coordinates(
    fit: &FitOverlay,
    datasets: &[DataSet],
    x_name: &str,
    y_name: &str,
) -> bool {
    if fit.target.x_column_name == x_name && fit.target.y_column_name == y_name {
        return true;
    }
    let Some(dataset) = fit
        .target
        .dataset_index
        .and_then(|index| datasets.get(index))
    else {
        return false;
    };
    dataset
        .fit_link
        .as_ref()
        .is_some_and(|link| link.source_x_column == x_name && link.source_y_column == y_name)
}

fn finite_range(values: &[f64]) -> Option<[f64; 2]> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    (minimum.is_finite() && maximum.is_finite()).then_some([minimum, maximum])
}

fn series_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(214, 79, 79),
        Color32::from_rgb(76, 145, 222),
        Color32::from_rgb(72, 176, 116),
        Color32::from_rgb(232, 153, 67),
        Color32::from_rgb(161, 112, 214),
        Color32::from_rgb(65, 185, 190),
    ];
    COLORS[index % COLORS.len()]
}

fn fit_color(index: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(255, 196, 64),
        Color32::from_rgb(246, 126, 188),
        Color32::from_rgb(143, 220, 220),
        Color32::from_rgb(184, 153, 255),
        Color32::from_rgb(255, 160, 94),
        Color32::from_rgb(166, 223, 105),
    ];
    COLORS[index % COLORS.len()]
}
