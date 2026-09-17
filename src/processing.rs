use std::fmt;

use crate::{data::DataSet, fitting};

#[derive(Clone, Debug)]
pub enum ProcessingOperation {
    Center,
    CenterNormalize {
        top_n: usize,
    },
    PolynomialBackground {
        x_column: usize,
        fit_min: f64,
        fit_max: f64,
        order: usize,
    },
    LocalFlatten {
        x_column: usize,
        x1: f64,
        x2: f64,
        transition: f64,
        anchor: Anchor,
        strength: f64,
    },
    Denoise {
        window_length: usize,
        polyorder: usize,
        range: Option<(usize, f64, f64)>,
    },
    Formula {
        x_column: usize,
        expression: String,
        a: f64,
        b: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessingMetadata {
    Center {
        midpoint: f64,
    },
    Normalize {
        midpoint: f64,
        scale: f64,
        top_n: usize,
    },
    PolynomialBackground {
        order: usize,
    },
    LocalFlatten {
        slope: f64,
        anchor: Anchor,
    },
    Denoise {
        window_length: usize,
        polyorder: usize,
    },
    Formula {
        expression: String,
        a: f64,
        b: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessingResult {
    pub values: Vec<f64>,
    pub metadata: ProcessingMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingError {
    pub operation: &'static str,
    pub code: &'static str,
    pub reason: String,
}

impl ProcessingError {
    fn new(operation: &'static str, code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            operation,
            code,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}：{}",
            self.operation, self.code, self.reason
        )
    }
}

impl std::error::Error for ProcessingError {}

pub fn apply_to_dataset(
    dataset: &DataSet,
    y_column: usize,
    operation: &ProcessingOperation,
) -> Result<ProcessingResult, ProcessingError> {
    let y = dataset
        .columns
        .get(y_column)
        .ok_or_else(|| ProcessingError::new("processing", "missing_column", "Y 列不存在"))?
        .values
        .as_slice();
    match operation {
        ProcessingOperation::Center => center_values(y),
        ProcessingOperation::CenterNormalize { top_n } => {
            let centered = center_values(y)?;
            let midpoint = match centered.metadata {
                ProcessingMetadata::Center { midpoint } => midpoint,
                _ => unreachable!(),
            };
            let mut normalized = normalize_values(&centered.values, *top_n)?;
            if let ProcessingMetadata::Normalize {
                scale,
                top_n: actual_top_n,
                ..
            } = normalized.metadata
            {
                normalized.metadata = ProcessingMetadata::Normalize {
                    midpoint,
                    scale,
                    top_n: actual_top_n,
                };
            }
            Ok(normalized)
        }
        ProcessingOperation::PolynomialBackground {
            x_column,
            fit_min,
            fit_max,
            order,
        } => remove_polynomial_background(
            column_values(dataset, *x_column, "background")?,
            y,
            *fit_min,
            *fit_max,
            *order,
        ),
        ProcessingOperation::LocalFlatten {
            x_column,
            x1,
            x2,
            transition,
            anchor,
            strength,
        } => local_flatten_values(
            column_values(dataset, *x_column, "local_flatten")?,
            y,
            *x1,
            *x2,
            *transition,
            *anchor,
            *strength,
        ),
        ProcessingOperation::Denoise {
            window_length,
            polyorder,
            range,
        } => {
            let x_range = range
                .map(|(column, x1, x2)| Ok((column_values(dataset, column, "denoise")?, x1, x2)))
                .transpose()?;
            denoise_values(y, *window_length, *polyorder, x_range)
        }
        ProcessingOperation::Formula {
            x_column,
            expression,
            a,
            b,
        } => formula_values(
            column_values(dataset, *x_column, "formula")?,
            y,
            expression,
            *a,
            *b,
        ),
    }
}

fn formula_values(
    x: &[f64],
    y: &[f64],
    expression: &str,
    a: f64,
    b: f64,
) -> Result<ProcessingResult, ProcessingError> {
    let values = fitting::evaluate_formula_values(expression, x, y, a, b)
        .map_err(|error| ProcessingError::new("formula", error.code, error.reason))?;
    Ok(ProcessingResult {
        values,
        metadata: ProcessingMetadata::Formula {
            expression: expression.to_owned(),
            a,
            b,
        },
    })
}

fn column_values<'a>(
    dataset: &'a DataSet,
    column: usize,
    operation: &'static str,
) -> Result<&'a [f64], ProcessingError> {
    dataset
        .columns
        .get(column)
        .map(|column| column.values.as_slice())
        .ok_or_else(|| ProcessingError::new(operation, "missing_column", "X 列不存在"))
}

pub fn center_values(values: &[f64]) -> Result<ProcessingResult, ProcessingError> {
    if values.is_empty() {
        return Err(ProcessingError::new(
            "center",
            "empty_values",
            "数据不能为空",
        ));
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values.iter().copied().filter(|value| value.is_finite()) {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    if !minimum.is_finite() || !maximum.is_finite() {
        return Err(ProcessingError::new(
            "center",
            "no_finite_values",
            "没有可处理的有限值",
        ));
    }
    let midpoint = maximum / 2.0 + minimum / 2.0;
    if !midpoint.is_finite() {
        return Err(numeric_failure("center"));
    }
    let mut result = values.to_vec();
    for value in result.iter_mut().filter(|value| value.is_finite()) {
        *value -= midpoint;
        if !value.is_finite() {
            return Err(numeric_failure("center"));
        }
    }
    Ok(ProcessingResult {
        values: result,
        metadata: ProcessingMetadata::Center { midpoint },
    })
}

pub fn normalize_values(values: &[f64], top_n: usize) -> Result<ProcessingResult, ProcessingError> {
    if values.is_empty() {
        return Err(ProcessingError::new(
            "normalize",
            "empty_values",
            "数据不能为空",
        ));
    }
    if top_n == 0 {
        return Err(ProcessingError::new(
            "normalize",
            "invalid_top_n",
            "top_n 必须大于零",
        ));
    }
    let mut finite: Vec<f64> = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if finite.is_empty() {
        return Err(ProcessingError::new(
            "normalize",
            "no_finite_values",
            "没有可处理的有限值",
        ));
    }
    finite.sort_by(f64::total_cmp);
    let count = top_n.min(finite.len());
    let selected = &finite[finite.len() - count..];
    let magnitude = selected
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    let scale = if magnitude == 0.0 {
        0.0
    } else {
        magnitude * (selected.iter().map(|value| value / magnitude).sum::<f64>() / count as f64)
    };
    if !scale.is_finite() {
        return Err(numeric_failure("normalize"));
    }
    let mut result = values.to_vec();
    if scale != 0.0 {
        for value in result.iter_mut().filter(|value| value.is_finite()) {
            *value = if *value > scale {
                1.0
            } else if *value < -scale {
                -1.0
            } else {
                *value / scale
            };
        }
    }
    Ok(ProcessingResult {
        values: result,
        metadata: ProcessingMetadata::Normalize {
            midpoint: 0.0,
            scale,
            top_n: count,
        },
    })
}

pub fn remove_polynomial_background(
    x: &[f64],
    y: &[f64],
    fit_min: f64,
    fit_max: f64,
    order: usize,
) -> Result<ProcessingResult, ProcessingError> {
    validate_xy(x, y, "background")?;
    if !fit_min.is_finite() || !fit_max.is_finite() || fit_min > fit_max {
        return Err(ProcessingError::new(
            "background",
            "invalid_interval",
            "拟合区间无效",
        ));
    }
    if order > 5 {
        return Err(ProcessingError::new(
            "background",
            "invalid_order",
            "Lite 版支持 0 到 5 阶多项式",
        ));
    }
    let fit_points: Vec<(f64, f64)> = x
        .iter()
        .copied()
        .zip(y.iter().copied())
        .filter(|(x, y)| x.is_finite() && y.is_finite() && *x >= fit_min && *x <= fit_max)
        .collect();
    if fit_points.len() <= order {
        return Err(ProcessingError::new(
            "background",
            "insufficient_fit_points",
            "拟合点数量不足",
        ));
    }
    let model = fit_polynomial(&fit_points, order, "background")?;
    let mut result = y.to_vec();
    for (index, (x, y)) in x.iter().zip(y).enumerate() {
        if x.is_finite() && y.is_finite() {
            result[index] = *y - model.evaluate(*x);
            if !result[index].is_finite() {
                return Err(numeric_failure("background"));
            }
        }
    }
    Ok(ProcessingResult {
        values: result,
        metadata: ProcessingMetadata::PolynomialBackground { order },
    })
}

pub fn local_flatten_values(
    x: &[f64],
    y: &[f64],
    mut x1: f64,
    mut x2: f64,
    transition: f64,
    anchor: Anchor,
    strength: f64,
) -> Result<ProcessingResult, ProcessingError> {
    validate_xy(x, y, "local_flatten")?;
    if !x1.is_finite() || !x2.is_finite() {
        return Err(ProcessingError::new(
            "local_flatten",
            "invalid_interval",
            "处理区间无效",
        ));
    }
    if !transition.is_finite() || transition < 0.0 {
        return Err(ProcessingError::new(
            "local_flatten",
            "invalid_transition",
            "过渡宽度必须是非负有限值",
        ));
    }
    if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
        return Err(ProcessingError::new(
            "local_flatten",
            "invalid_strength",
            "处理强度必须在 0 到 1 之间",
        ));
    }
    if x1 > x2 {
        std::mem::swap(&mut x1, &mut x2);
    }
    let fit_points: Vec<(f64, f64)> = x
        .iter()
        .copied()
        .zip(y.iter().copied())
        .filter(|(x, y)| x.is_finite() && y.is_finite() && *x >= x1 && *x <= x2)
        .collect();
    if fit_points.len() < 2 {
        return Ok(ProcessingResult {
            values: y.to_vec(),
            metadata: ProcessingMetadata::LocalFlatten { slope: 0.0, anchor },
        });
    }
    let model = fit_polynomial(&fit_points, 1, "local_flatten")?;
    let slope = model.original_linear_slope();
    let anchor_x = match anchor {
        Anchor::Left => x1,
        Anchor::Right => x2,
        Anchor::Center => x1 / 2.0 + x2 / 2.0,
    };
    let left_edge = x1 - transition;
    let right_edge = x2 + transition;
    if !left_edge.is_finite() || !right_edge.is_finite() {
        return Err(numeric_failure("local_flatten"));
    }
    let mut result = y.to_vec();
    for (index, (x, y)) in x.iter().zip(y).enumerate() {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let weight = if *x >= x1 && *x <= x2 {
            1.0
        } else if transition > 0.0 && *x >= left_edge && *x < x1 {
            0.5 * (1.0 - (std::f64::consts::PI * (*x - left_edge) / transition).cos())
        } else if transition > 0.0 && *x > x2 && *x <= right_edge {
            0.5 * (1.0 + (std::f64::consts::PI * (*x - x2) / transition).cos())
        } else {
            0.0
        };
        result[index] = *y - weight * strength * slope * (*x - anchor_x);
    }
    Ok(ProcessingResult {
        values: result,
        metadata: ProcessingMetadata::LocalFlatten { slope, anchor },
    })
}

pub fn denoise_values(
    y: &[f64],
    window_length: usize,
    polyorder: usize,
    x_range: Option<(&[f64], f64, f64)>,
) -> Result<ProcessingResult, ProcessingError> {
    if y.is_empty() {
        return Err(ProcessingError::new(
            "denoise",
            "empty_values",
            "数据不能为空",
        ));
    }
    if window_length == 0 || window_length.is_multiple_of(2) || window_length <= polyorder {
        return Err(ProcessingError::new(
            "denoise",
            "invalid_window",
            "窗口必须为大于多项式阶数的正奇数",
        ));
    }
    if let Some((x, x1, x2)) = x_range {
        validate_xy(x, y, "denoise")?;
        if !x1.is_finite() || !x2.is_finite() || x1 > x2 {
            return Err(ProcessingError::new(
                "denoise",
                "invalid_interval",
                "处理区间无效",
            ));
        }
    }
    let eligible = |index: usize| {
        y[index].is_finite()
            && x_range
                .is_none_or(|(x, x1, x2)| x[index].is_finite() && x[index] >= x1 && x[index] <= x2)
    };
    let mut result = y.to_vec();
    let mut start = 0;
    while start < y.len() {
        while start < y.len() && !eligible(start) {
            start += 1;
        }
        if start == y.len() {
            break;
        }
        let mut end = start + 1;
        while end < y.len() && eligible(end) {
            end += 1;
        }
        smooth_segment(
            &y[start..end],
            &mut result[start..end],
            window_length,
            polyorder,
        )?;
        start = end;
    }
    Ok(ProcessingResult {
        values: result,
        metadata: ProcessingMetadata::Denoise {
            window_length,
            polyorder,
        },
    })
}

fn smooth_segment(
    input: &[f64],
    output: &mut [f64],
    requested: usize,
    order: usize,
) -> Result<(), ProcessingError> {
    let Some(window) = effective_window(input.len(), requested, order) else {
        return Ok(());
    };
    let half = window / 2;

    // SciPy's default Savitzky–Golay edge mode fits one polynomial to each
    // end window. The interior uses one fixed convolution kernel. Computing
    // those three objects once avoids a polynomial fit and allocation per row.
    let first_points: Vec<(f64, f64)> = input[..window]
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64, *value))
        .collect();
    let first_model = fit_polynomial(&first_points, order, "denoise")?;
    for (index, value) in output.iter_mut().take(half).enumerate() {
        *value = first_model.evaluate(index as f64);
    }

    let weights: Vec<f64> = (0..window)
        .map(|active| {
            let basis: Vec<(f64, f64)> = (0..window)
                .map(|index| {
                    (
                        index as f64 - half as f64,
                        if index == active { 1.0 } else { 0.0 },
                    )
                })
                .collect();
            fit_polynomial(&basis, order, "denoise").map(|model| model.evaluate(0.0))
        })
        .collect::<Result<_, _>>()?;
    for index in half..input.len() - half {
        output[index] = weights
            .iter()
            .zip(&input[index - half..index + half + 1])
            .map(|(weight, value)| weight * value)
            .sum();
    }

    let last_start = input.len() - window;
    let last_points: Vec<(f64, f64)> = input[last_start..]
        .iter()
        .enumerate()
        .map(|(index, value)| (index as f64, *value))
        .collect();
    let last_model = fit_polynomial(&last_points, order, "denoise")?;
    for (index, value) in output.iter_mut().enumerate().skip(input.len() - half) {
        *value = last_model.evaluate((index - last_start) as f64);
    }
    Ok(())
}

fn effective_window(length: usize, requested: usize, order: usize) -> Option<usize> {
    if length < 3 {
        return None;
    }
    let mut candidate = requested.min(length);
    if candidate.is_multiple_of(2) {
        candidate -= 1;
    }
    (candidate > order).then_some(candidate)
}

fn validate_xy(x: &[f64], y: &[f64], operation: &'static str) -> Result<(), ProcessingError> {
    if x.is_empty() || y.is_empty() {
        return Err(ProcessingError::new(
            operation,
            "empty_values",
            "数据不能为空",
        ));
    }
    if x.len() != y.len() {
        return Err(ProcessingError::new(
            operation,
            "length_mismatch",
            "X/Y 长度不同",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct PolynomialModel {
    coefficients: Vec<f64>,
    center: f64,
    scale: f64,
}

impl PolynomialModel {
    fn evaluate(&self, x: f64) -> f64 {
        let normalized = (x - self.center) / self.scale;
        self.coefficients
            .iter()
            .rev()
            .fold(0.0, |value, coefficient| value * normalized + coefficient)
    }

    fn original_linear_slope(&self) -> f64 {
        self.coefficients.get(1).copied().unwrap_or(0.0) / self.scale
    }
}

fn fit_polynomial(
    points: &[(f64, f64)],
    order: usize,
    operation: &'static str,
) -> Result<PolynomialModel, ProcessingError> {
    let minimum = points
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let maximum = points
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let center = maximum / 2.0 + minimum / 2.0;
    if order == 0 {
        let magnitude = points
            .iter()
            .map(|point| point.1.abs())
            .fold(0.0_f64, f64::max);
        let constant = if magnitude == 0.0 {
            0.0
        } else {
            magnitude
                * (points.iter().map(|point| point.1 / magnitude).sum::<f64>()
                    / points.len() as f64)
        };
        if !center.is_finite() || !constant.is_finite() {
            return Err(numeric_failure(operation));
        }
        return Ok(PolynomialModel {
            coefficients: vec![constant],
            center,
            scale: 1.0,
        });
    }
    let scale = points
        .iter()
        .map(|point| (point.0 - center).abs())
        .fold(0.0_f64, f64::max);
    if !center.is_finite() || !scale.is_finite() || scale == 0.0 {
        return Err(ProcessingError::new(
            operation,
            "degenerate_x",
            "拟合 X 值无法确定多项式",
        ));
    }

    let column_count = order + 1;
    let mut q_columns: Vec<Vec<f64>> = Vec::with_capacity(column_count);
    let mut upper = vec![vec![0.0; column_count]; column_count];
    for column in 0..column_count {
        let mut values: Vec<f64> = points
            .iter()
            .map(|point| ((point.0 - center) / scale).powi(column as i32))
            .collect();
        let mut projections = Vec::with_capacity(column);
        for basis_column in &q_columns {
            let projection = dot(basis_column, &values);
            projections.push(projection);
            for (value, basis) in values.iter_mut().zip(basis_column) {
                *value -= projection * basis;
            }
        }
        let norm = dot(&values, &values).sqrt();
        if !norm.is_finite() || norm <= f64::EPSILON * (points.len() as f64).sqrt() {
            return Err(ProcessingError::new(
                operation,
                "degenerate_x",
                "拟合 X 值无法确定多项式",
            ));
        }
        let (previous_rows, current_rows) = upper.split_at_mut(column);
        for (row, projection) in previous_rows.iter_mut().zip(projections) {
            row[column] = projection;
        }
        current_rows[0][column] = norm;
        for value in &mut values {
            *value /= norm;
        }
        q_columns.push(values);
    }

    let targets: Vec<f64> = points.iter().map(|point| point.1).collect();
    let projected: Vec<f64> = q_columns
        .iter()
        .map(|column| dot(column, &targets))
        .collect();
    let mut coefficients = vec![0.0; column_count];
    for row in (0..column_count).rev() {
        let remainder = projected[row]
            - upper[row][row + 1..]
                .iter()
                .zip(&coefficients[row + 1..])
                .map(|(factor, coefficient)| factor * coefficient)
                .sum::<f64>();
        coefficients[row] = remainder / upper[row][row];
        if !coefficients[row].is_finite() {
            return Err(numeric_failure(operation));
        }
    }
    Ok(PolynomialModel {
        coefficients,
        center,
        scale,
    })
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn numeric_failure(operation: &'static str) -> ProcessingError {
    ProcessingError::new(operation, "numeric_failure", "数值计算失败")
}

#[cfg(test)]
mod tests {
    use super::{
        Anchor, ProcessingError, ProcessingMetadata, ProcessingOperation, apply_to_dataset,
        center_values, denoise_values, fit_polynomial, local_flatten_values, normalize_values,
        remove_polynomial_background,
    };
    use crate::data::{DataSet, DataSetKind, NumericColumn};
    use std::path::PathBuf;

    fn dataset(x: &[f64], y: &[f64]) -> DataSet {
        DataSet {
            source: PathBuf::from("processing.csv"),
            label: None,
            kind: DataSetKind::Source,
            plot_id: "test-processing".to_owned(),
            fit_link: None,
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "x".to_owned(),
                    values: x.to_vec(),
                },
                NumericColumn {
                    name: "y".to_owned(),
                    values: y.to_vec(),
                },
            ],
            row_count: x.len(),
            alive: vec![true; x.len()],
        }
    }

    fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            if expected.is_nan() {
                assert!(actual.is_nan());
            } else if expected.is_infinite() {
                assert_eq!(actual, expected);
            } else {
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{actual} != {expected}"
                );
            }
        }
    }

    fn error_code(result: Result<super::ProcessingResult, ProcessingError>) -> &'static str {
        result.unwrap_err().code
    }

    #[test]
    fn center_matches_legacy_midrange_and_preserves_nonfinite_values() {
        let result = center_values(&[f64::NAN, -2.0, 2.0, f64::INFINITY, 6.0]).unwrap();
        assert_close(
            &result.values,
            &[f64::NAN, -4.0, 0.0, f64::INFINITY, 4.0],
            0.0,
        );
        assert_eq!(
            result.metadata,
            ProcessingMetadata::Center { midpoint: 2.0 }
        );
    }

    #[test]
    fn normalize_matches_legacy_top_n_and_clipping_rules() {
        let negative = normalize_values(&[-3.0, -2.0, -1.0], 2).unwrap();
        assert_close(&negative.values, &[-1.0, -1.0, 1.0], 0.0);
        assert_eq!(
            negative.metadata,
            ProcessingMetadata::Normalize {
                midpoint: 0.0,
                scale: -1.5,
                top_n: 2
            }
        );
        let mixed = normalize_values(&[1.0, f64::NAN, f64::INFINITY, 3.0], 2).unwrap();
        assert_close(&mixed.values, &[0.5, f64::NAN, f64::INFINITY, 1.0], 1e-12);
    }

    #[test]
    fn dataset_normalize_preserves_legacy_gui_center_then_normalize_order() {
        let dataset = dataset(&[0.0, 1.0, 2.0], &[1.0, 2.0, 5.0]);
        let result = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::CenterNormalize { top_n: 20 },
        )
        .unwrap();
        assert_close(&result.values, &[-1.0, -1.0, 1.0], 1e-12);
        assert_eq!(
            result.metadata,
            ProcessingMetadata::Normalize {
                midpoint: 3.0,
                scale: -1.0 / 3.0,
                top_n: 3,
            }
        );
    }

    #[test]
    fn polynomial_background_removes_exact_quadratic() {
        let x = [-2.0, -1.0, 0.0, 1.0, 2.0];
        let y: Vec<f64> = x.iter().map(|x| 2.0 + 3.0 * x + 4.0 * x * x).collect();
        let result = remove_polynomial_background(&x, &y, -2.0, 2.0, 2).unwrap();
        assert_close(&result.values, &[0.0; 5], 1e-11);
    }

    #[test]
    fn constant_background_does_not_require_distinct_x_values() {
        let result = remove_polynomial_background(&[1.0, 1.0], &[2.0, 4.0], 1.0, 1.0, 0).unwrap();
        assert_close(&result.values, &[-1.0, 1.0], 1e-12);
    }

    #[test]
    fn local_flatten_keeps_left_anchor_and_removes_slope() {
        let result = local_flatten_values(
            &[0.0, 1.0, 2.0],
            &[1.0, 3.0, 5.0],
            0.0,
            2.0,
            0.0,
            Anchor::Left,
            1.0,
        )
        .unwrap();
        assert_close(&result.values, &[1.0, 1.0, 1.0], 1e-12);
    }

    #[test]
    fn denoise_matches_linear_savgol_and_keeps_nonfinite_segments() {
        let result = denoise_values(
            &[0.0, 1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0, f64::INFINITY],
            3,
            1,
            None,
        )
        .unwrap();
        assert_close(
            &result.values,
            &[0.0, 1.0, 2.0, f64::NAN, 4.0, 5.0, 6.0, f64::INFINITY],
            1e-12,
        );
    }

    #[test]
    fn formula_uses_y_and_parameters_without_being_affected_by_missing_x() {
        let dataset = dataset(&[1.0, 2.0, f64::NAN], &[3.0, 4.0, 5.0]);
        let result = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::Formula {
                x_column: 0,
                expression: "a * y + b".to_owned(),
                a: 2.0,
                b: -1.0,
            },
        )
        .unwrap();
        assert_close(&result.values, &[5.0, 7.0, 9.0], 1e-12);
        assert_eq!(
            result.metadata,
            ProcessingMetadata::Formula {
                expression: "a * y + b".to_owned(),
                a: 2.0,
                b: -1.0,
            }
        );
    }

    #[test]
    fn formula_uses_x_without_being_affected_by_missing_y() {
        let dataset = dataset(&[1.0, 2.0, 3.0], &[10.0, f64::NAN, 30.0]);
        let result = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::Formula {
                x_column: 0,
                expression: "2 * x + 1".to_owned(),
                a: 1.0,
                b: 0.0,
            },
        )
        .unwrap();
        assert_close(&result.values, &[3.0, 5.0, 7.0], 1e-12);
    }

    #[test]
    fn formula_uses_the_actual_parameter_values_during_evaluation() {
        let dataset = dataset(&[1.0, 2.0], &[3.0, 4.0]);
        let result = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::Formula {
                x_column: 0,
                expression: "y / b".to_owned(),
                a: 1.0,
                b: 2.0,
            },
        )
        .unwrap();
        assert_close(&result.values, &[1.5, 2.0], 1e-12);
    }

    #[test]
    fn formula_supports_existing_functions_and_reports_invalid_rows() {
        let dataset = dataset(&[0.0, 1.0], &[0.0, 1.0]);
        let result = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::Formula {
                x_column: 0,
                expression: "sin(x) + sqrt(x)".to_owned(),
                a: 1.0,
                b: 0.0,
            },
        )
        .unwrap();
        assert_close(&result.values, &[0.0, 1.0 + 1.0_f64.sin()], 1e-12);

        let error = apply_to_dataset(
            &dataset,
            1,
            &ProcessingOperation::Formula {
                x_column: 0,
                expression: "x / (x - x)".to_owned(),
                a: 1.0,
                b: 0.0,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "formula_error");
        assert!(error.reason.contains("第 1 行"));
    }

    #[test]
    fn optimized_denoise_matches_per_point_polynomial_reference() {
        let input = [0.0, 1.0, 4.0, 9.0, 16.0, 30.0, 36.0, 49.0, 64.0];
        let result = denoise_values(&input, 5, 2, None).unwrap();
        let half = 2;
        let mut reference = Vec::with_capacity(input.len());
        for index in 0..input.len() {
            let start = index.saturating_sub(half).min(input.len() - 5);
            let points: Vec<(f64, f64)> = (start..start + 5)
                .map(|sample| (sample as f64 - index as f64, input[sample]))
                .collect();
            reference.push(fit_polynomial(&points, 2, "denoise").unwrap().evaluate(0.0));
        }
        assert_close(&result.values, &reference, 1e-11);
    }

    #[test]
    fn validation_codes_match_legacy_contract() {
        assert_eq!(error_code(center_values(&[])), "empty_values");
        assert_eq!(error_code(normalize_values(&[1.0], 0)), "invalid_top_n");
        assert_eq!(
            error_code(local_flatten_values(
                &[0.0],
                &[1.0, 2.0],
                0.0,
                1.0,
                0.0,
                Anchor::Left,
                1.0
            )),
            "length_mismatch"
        );
        assert_eq!(
            error_code(denoise_values(&[1.0, 2.0], 3, 3, None)),
            "invalid_window"
        );
        assert_eq!(
            error_code(remove_polynomial_background(&[0.0], &[1.0], 1.0, 0.0, 1)),
            "invalid_interval"
        );
    }
}
