use crate::{data::DataSet, fitting};
use instplot_processing::{
    ProcessingError, ProcessingResult, center_values, denoise_values, local_flatten_values,
    normalize_values, remove_polynomial_background,
};

pub use instplot_processing::{Anchor, ProcessingMetadata, ProcessingOperation};

pub fn apply_to_dataset(
    dataset: &DataSet,
    y_column: usize,
    operation: &ProcessingOperation,
) -> Result<ProcessingResult, ProcessingError> {
    let y = dataset
        .columns
        .get(y_column)
        .ok_or_else(|| processing_error("processing", "missing_column", "Y 列不存在"))?
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
        .map_err(|error| processing_error("formula", error.code, error.reason))?;
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
        .ok_or_else(|| processing_error(operation, "missing_column", "X 列不存在"))
}

fn processing_error(
    operation: &'static str,
    code: &'static str,
    reason: impl Into<String>,
) -> ProcessingError {
    ProcessingError {
        operation,
        code,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessingMetadata, ProcessingOperation, apply_to_dataset};
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
}
