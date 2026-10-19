use eframe::egui::{self, Color32, PointerButton, Rect, Stroke, StrokeKind};
use egui_plot::{GridInput, GridMark, Legend, Plot, PlotMemory, PlotPoint};

use crate::{data::DataSet, session::FitOverlay};

use super::{
    formatting::AxisDisplay,
    main_view,
    plot_series::{self, PendingHighlight, PlotSeriesInput},
};

const PLOT_LEFT_GUTTER: f32 = 20.0;
const PLOT_BOTTOM_GUTTER: f32 = 12.0;
const PLOT_EXPORT_TOP_GUTTER: f32 = 8.0;

pub struct PlotViewInput<'a> {
    pub demo_points: &'a [[f64; 2]],
    pub datasets: &'a [DataSet],
    pub active_dataset: usize,
    pub x_column: usize,
    pub y_column: usize,
    pub column_names: &'a [String],
    pub fit_overlays: &'a [FitOverlay],
    pub pending_highlight: Option<PendingHighlight<'a>>,
    pub pending_deletion: bool,
    pub reset_view: bool,
    pub visible_x_range: Option<[f64; 2]>,
    pub last_plot_rect: Option<Rect>,
    pub selection_start: Option<egui::Pos2>,
    pub selection_current: Option<egui::Pos2>,
    pub selected_coordinate: Option<[f64; 2]>,
    pub status: &'a str,
}

pub struct DeletionRequest {
    pub dataset_index: usize,
    pub rows: Vec<usize>,
    pub x_column: usize,
    pub y_column: usize,
}

pub struct PlotResponse {
    pub last_plot_rect: Rect,
    pub last_export_rect: Rect,
    pub visible_x_range: Option<[f64; 2]>,
    pub selection_start: Option<egui::Pos2>,
    pub selection_current: Option<egui::Pos2>,
    pub selected_coordinate: Option<[f64; 2]>,
    pub deletion_request: Option<DeletionRequest>,
    pub status_update: Option<String>,
}

pub fn show(ui: &mut egui::Ui, input: PlotViewInput<'_>) -> PlotResponse {
    let plot_height = (ui.available_height()
        - PLOT_BOTTOM_GUTTER
        - main_view::STATUS_ROW_HEIGHT
        - main_view::STATUS_BOTTOM_INSET)
        .max(220.0);
    let (plot_x_name, plot_y_name) = plot_series::plot_coordinate_names(
        input.datasets,
        input.active_dataset,
        input.x_column,
        input.y_column,
    )
    .unwrap_or_else(|| {
        (
            input
                .column_names
                .get(input.x_column)
                .cloned()
                .unwrap_or_else(|| "x".to_owned()),
            input
                .column_names
                .get(input.y_column)
                .cloned()
                .unwrap_or_else(|| "y".to_owned()),
        )
    });
    let x_axis_display = AxisDisplay::from_range(plot_series::plotted_axis_range(
        input.datasets,
        input.active_dataset,
        &plot_x_name,
        &plot_y_name,
        0,
    ));
    let y_axis_display = AxisDisplay::from_range(plot_series::plotted_axis_range(
        input.datasets,
        input.active_dataset,
        &plot_x_name,
        &plot_y_name,
        1,
    ));
    let plot_id = egui::Id::new("main-plot");
    let hidden_series_before = PlotMemory::load(ui.ctx(), plot_id)
        .map(|memory| memory.hidden_items)
        .unwrap_or_default();
    if input.reset_view {
        reset_plot_bounds_preserving_visibility(ui.ctx(), plot_id);
    }
    let mut plot = Plot::new("main-plot")
        .id(plot_id)
        .legend(Legend::default())
        .height(plot_height)
        .allow_zoom(true)
        .allow_scroll(false)
        .allow_drag(true)
        .allow_boxed_zoom(false)
        .pan_pointer_button(PointerButton::Secondary)
        .x_grid_spacer(scale_aware_grid_marks)
        .y_grid_spacer(scale_aware_grid_marks)
        .x_axis_formatter(move |mark, _range| {
            x_axis_display.format_tick(mark.value, mark.step_size)
        })
        .y_axis_formatter(move |mark, _range| {
            y_axis_display.format_tick(mark.value, mark.step_size)
        });
    plot = plot.x_axis_label(
        egui::RichText::new(x_axis_display.label(&plot_x_name))
            .size(17.0)
            .strong(),
    );
    plot = plot.y_axis_label(
        egui::RichText::new(y_axis_display.label(&plot_y_name))
            .size(17.0)
            .strong(),
    );
    let mut plotted_series_ids = Vec::new();
    let plot_row = ui.horizontal(|ui| {
        // egui_plot paints the vertical axis title just outside its own plot
        // rectangle, so reserve a real gutter inside the viewport.
        ui.add_space(PLOT_LEFT_GUTTER);
        plot.show(ui, |plot_ui| {
            if plot_ui.response().contains_pointer() {
                let wheel_delta = plot_ui.ctx().input(|input| input.smooth_scroll_delta.y);
                if wheel_delta != 0.0 {
                    plot_ui.zoom_bounds_around_hovered(egui::Vec2::splat(wheel_zoom_factor(
                        wheel_delta,
                    )));
                }
            }
            plotted_series_ids = plot_series::show(
                plot_ui,
                PlotSeriesInput {
                    demo_points: input.demo_points,
                    datasets: input.datasets,
                    active_dataset: input.active_dataset,
                    plot_x_name: &plot_x_name,
                    plot_y_name: &plot_y_name,
                    last_plot_rect: input.last_plot_rect,
                    visible_x_range: input.visible_x_range,
                    pending_highlight: input.pending_highlight,
                    fit_overlays: input.fit_overlays,
                },
            );
        })
    });
    ui.add_space(PLOT_BOTTOM_GUTTER);
    let last_export_rect = Rect::from_min_max(
        plot_row.response.rect.min - egui::vec2(0.0, PLOT_EXPORT_TOP_GUTTER),
        plot_row.response.rect.max + egui::vec2(0.0, PLOT_BOTTOM_GUTTER),
    );
    main_view::show_status(ui, input.status, input.selected_coordinate);

    let response = plot_row.inner;
    let last_plot_rect = response.response.rect;
    let bounds = response.transform.bounds();
    let all_series_hidden_before = !plotted_series_ids.is_empty()
        && plotted_series_ids
            .iter()
            .all(|series_id| hidden_series_before.contains(series_id));
    let all_series_hidden_after = !plotted_series_ids.is_empty()
        && PlotMemory::load(ui.ctx(), plot_id).is_some_and(|memory| {
            plotted_series_ids
                .iter()
                .all(|series_id| memory.hidden_items.contains(series_id))
        });
    let mut visible_x_range = input.visible_x_range;
    if !all_series_hidden_before && !all_series_hidden_after {
        visible_x_range = Some([bounds.min()[0], bounds.max()[0]]);
    }

    let mut selection_start = input.selection_start;
    let mut selection_current = input.selection_current;
    let mut selected_coordinate = input.selected_coordinate;
    let mut deletion_request = None;
    let mut status_update = None;
    if !input.pending_deletion {
        if response.response.drag_started_by(PointerButton::Primary) {
            selected_coordinate = None;
            selection_start = response.response.interact_pointer_pos();
            selection_current = selection_start;
        }
        if response.response.dragged_by(PointerButton::Primary) {
            selection_current = response.response.interact_pointer_pos();
        }
        if let (Some(start), Some(current)) = (selection_start, selection_current) {
            let selection = Rect::from_two_pos(start, current);
            if selection.width() >= 2.0 || selection.height() >= 2.0 {
                ui.painter().rect_stroke(
                    selection,
                    0.0,
                    Stroke::new(1.5, Color32::YELLOW),
                    StrokeKind::Inside,
                );
            }
        }
        if response.response.drag_stopped_by(PointerButton::Primary) {
            let start = selection_start.take();
            let end = response
                .response
                .interact_pointer_pos()
                .or_else(|| selection_current.take());
            selection_current = None;
            if let (Some(start), Some(end)) = (start, end)
                && start.distance(end) >= 4.0
            {
                let first = response.transform.value_from_position(start);
                let second = response.transform.value_from_position(end);
                if let Some(dataset) = input.datasets.get(input.active_dataset) {
                    let rows = dataset.rows_in_bounds(
                        input.x_column,
                        input.y_column,
                        [first.x, second.x],
                        [first.y, second.y],
                    );
                    if rows.is_empty() {
                        status_update = Some("框选区域内没有可删除的数据点".to_owned());
                    } else {
                        deletion_request = Some(DeletionRequest {
                            dataset_index: input.active_dataset,
                            rows,
                            x_column: input.x_column,
                            y_column: input.y_column,
                        });
                    }
                }
            }
        } else if response.response.clicked_by(PointerButton::Primary)
            && let Some(pointer) = response.response.interact_pointer_pos()
        {
            let clicked = response.transform.value_from_position(pointer);
            selected_coordinate = Some([clicked.x, clicked.y]);
            status_update = Some("已显示点击位置坐标".to_owned());

            if let Some(dataset) = input.datasets.get(input.active_dataset) {
                let nearest = dataset
                    .row_points(input.x_column, input.y_column)
                    .filter_map(|(row_index, [x, y])| {
                        let screen = response
                            .transform
                            .position_from_point(&PlotPoint::new(x, y));
                        let distance_sq = screen.distance_sq(pointer);
                        (distance_sq <= 64.0).then_some((row_index, distance_sq))
                    })
                    .min_by(|left, right| left.1.total_cmp(&right.1));
                if let Some((row_index, _)) = nearest {
                    let x = dataset.columns[input.x_column].values[row_index];
                    let y = dataset.columns[input.y_column].values[row_index];
                    selected_coordinate = Some([x, y]);
                    deletion_request = Some(DeletionRequest {
                        dataset_index: input.active_dataset,
                        rows: vec![row_index],
                        x_column: input.x_column,
                        y_column: input.y_column,
                    });
                }
            }
        }
    }

    PlotResponse {
        last_plot_rect,
        last_export_rect,
        visible_x_range,
        selection_start,
        selection_current,
        selected_coordinate,
        deletion_request,
        status_update,
    }
}

pub fn reset_plot_bounds_preserving_visibility(context: &egui::Context, plot_id: egui::Id) -> bool {
    let Some(mut memory) = PlotMemory::load(context, plot_id) else {
        return false;
    };
    memory.auto_bounds = true.into();
    memory.store(context, plot_id);
    true
}

pub fn wheel_zoom_factor(wheel_delta: f32) -> f32 {
    (wheel_delta / 200.0).exp()
}

fn scale_aware_grid_marks(input: GridInput) -> Vec<GridMark> {
    if !input.base_step_size.is_finite()
        || input.base_step_size <= 0.0
        || !input.bounds.0.is_finite()
        || !input.bounds.1.is_finite()
        || input.bounds.0 >= input.bounds.1
    {
        return Vec::new();
    }

    // egui_plot's default spacer rejects steps below EPSILON, even though
    // small scientific data still has a valid, nonzero coordinate range.
    // Normalize only the spacer input; plotted data and interaction stay in
    // their original units. Keep ordinary ranges on the existing code path.
    let scale = if input.base_step_size < f64::EPSILON || input.base_step_size > 1e300 {
        10.0_f64.powi((input.base_step_size.log10().floor() as i32).clamp(-308, 306))
    } else {
        1.0
    };
    egui_plot::log_grid_spacer(10)(GridInput {
        bounds: (input.bounds.0 / scale, input.bounds.1 / scale),
        base_step_size: input.base_step_size / scale,
    })
    .into_iter()
    .map(|mark| GridMark {
        value: mark.value * scale,
        // Coarser grid levels may exceed f64 even when all coordinates fit.
        step_size: (mark.step_size * scale).min(f64::MAX),
    })
    .collect()
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn tiny_axis_grid_matches_normal_units_and_formats_distinct_ticks() {
        for scale in [1e-15, 1e-24, 1e-100] {
            let marks = scale_aware_grid_marks(GridInput {
                bounds: (-2.0 * scale, 6.0 * scale),
                base_step_size: 0.2 * scale,
            });
            assert!(marks.len() >= 5);
            let display = AxisDisplay::from_range(Some([-2.0 * scale, 6.0 * scale]));
            let mut labels = std::collections::HashSet::new();
            for mark in &marks {
                let value = mark.value / scale;
                assert!((-2.0 - 1e-10..=6.0 + 1e-10).contains(&value));
                assert!((value - value.round()).abs() < 1e-10);
                assert!(mark.step_size.is_finite() && mark.step_size > 0.0);
                labels.insert(display.format_tick(mark.value, mark.step_size));
            }
            assert_eq!(labels.len(), marks.len());
        }
    }

    #[test]
    fn ordinary_axis_grid_is_unchanged() {
        for step in [1e-10, 0.2, 10.0, 1e8] {
            let input = || GridInput {
                bounds: (-10.0 * step, 30.0 * step),
                base_step_size: step,
            };
            assert_eq!(
                scale_aware_grid_marks(input()),
                egui_plot::log_grid_spacer(10)(input())
            );
        }
    }

    #[test]
    fn small_variation_on_tiny_baseline_keeps_multiple_labels() {
        let bounds = (-1.198912e-14, -1.198902e-14);
        let marks = scale_aware_grid_marks(GridInput {
            bounds,
            base_step_size: 5e-21,
        });
        assert!(marks.len() >= 5);
        let display = AxisDisplay::from_range(Some([bounds.0, bounds.1]));
        let labels: std::collections::HashSet<_> = marks
            .iter()
            .map(|mark| {
                assert!(mark.value >= bounds.0 && mark.value <= bounds.1);
                display.format_tick(mark.value, mark.step_size)
            })
            .collect();
        assert_eq!(labels.len(), marks.len());
    }

    #[test]
    fn invalid_grid_ranges_do_not_generate_marks() {
        for step in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                scale_aware_grid_marks(GridInput {
                    bounds: (0.0, 1.0),
                    base_step_size: step,
                })
                .is_empty()
            );
        }
    }

    #[test]
    fn extreme_finite_axis_scales_keep_valid_grid_marks() {
        for scale in [1e-310, 1e307] {
            let marks = scale_aware_grid_marks(GridInput {
                bounds: (-2.0 * scale, 6.0 * scale),
                base_step_size: 0.2 * scale,
            });
            assert!(marks.len() >= 5);
            assert!(marks.iter().all(|mark| mark.value.is_finite()
                && mark.step_size.is_finite()
                && mark.step_size > 0.0));
        }
    }
}
