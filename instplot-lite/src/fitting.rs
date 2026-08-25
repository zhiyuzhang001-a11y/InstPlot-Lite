use std::fmt;

use fasteval2::{Evaler, ExpressionI, Parser, Slab};
use nalgebra::{DMatrix, DVector};

const PARAMETER_NAMES: [&str; 8] = ["a", "b", "c", "d", "e_param", "f", "g", "h"];
const FIT_POINT_COUNT: usize = 500;

#[derive(Clone, Debug, PartialEq)]
pub enum FitMethod {
    Polynomial {
        degree: usize,
    },
    Exponential,
    Logarithmic,
    Power,
    Custom {
        expression: String,
        initial_parameters: Vec<f64>,
    },
}

#[derive(Clone, Debug)]
pub struct FitResult {
    pub equation: String,
    pub r2: f64,
    pub points: Vec<[f64; 2]>,
    #[cfg(test)]
    pub parameters: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitError {
    pub code: &'static str,
    pub reason: String,
}

impl FitError {
    fn new(code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for FitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}：{}", self.code, self.reason)
    }
}

impl std::error::Error for FitError {}

pub fn fit_values(x: &[f64], y: &[f64], method: &FitMethod) -> Result<FitResult, FitError> {
    validate_values(x, y)?;
    let (parameters, equation, evaluator): (Vec<f64>, String, Box<dyn Fn(f64, &[f64]) -> f64>) =
        match method {
            FitMethod::Polynomial { degree } => {
                if !(1..=10).contains(degree) {
                    return Err(FitError::new("invalid_degree", "多项式阶数必须为 1 到 10"));
                }
                if x.len() <= *degree {
                    return Err(FitError::new(
                        "insufficient_points",
                        "数据点数量必须大于多项式阶数",
                    ));
                }
                let solution = polynomial_fit(x, y, *degree)?;
                let equation = polynomial_equation(&solution.original_coefficients);
                let normalized_coefficients = solution.normalized_coefficients;
                let center = solution.center;
                let scale = solution.scale;
                (
                    solution.original_coefficients,
                    equation,
                    Box::new(move |value, _parameters| {
                        let normalized = (value - center) / scale;
                        normalized_coefficients
                            .iter()
                            .fold(0.0, |result, coefficient| result * normalized + coefficient)
                    }),
                )
            }
            FitMethod::Exponential => {
                let initial = exponential_initial(x, y);
                let evaluator =
                    |value: f64, parameters: &[f64]| parameters[0] * (parameters[1] * value).exp();
                let parameters = nonlinear_fit(x, y, initial, &evaluator)?;
                let equation = format!(
                    "y = {} × exp({} × x)",
                    format_number(parameters[0]),
                    format_number(parameters[1])
                );
                (parameters, equation, Box::new(evaluator))
            }
            FitMethod::Logarithmic => {
                if x.iter().any(|value| *value <= 0.0) {
                    return Err(FitError::new("invalid_domain", "对数拟合要求所有 X 大于 0"));
                }
                let transformed: Vec<f64> = x.iter().map(|value| value.ln()).collect();
                let coefficients = linear_fit(&transformed, y)?;
                let parameters = vec![coefficients[0], coefficients[1]];
                let equation = format!(
                    "y = {} × ln(x) + {}",
                    format_number(parameters[0]),
                    format_number(parameters[1])
                );
                (
                    parameters,
                    equation,
                    Box::new(|value, parameters| parameters[0] * value.ln() + parameters[1]),
                )
            }
            FitMethod::Power => {
                if x.iter().any(|value| *value <= 0.0) || y.iter().any(|value| *value <= 0.0) {
                    return Err(FitError::new(
                        "invalid_domain",
                        "幂函数拟合要求所有 X、Y 大于 0",
                    ));
                }
                let log_x: Vec<f64> = x.iter().map(|value| value.ln()).collect();
                let log_y: Vec<f64> = y.iter().map(|value| value.ln()).collect();
                let seed = linear_fit(&log_x, &log_y)?;
                let initial = vec![seed[1].exp(), seed[0]];
                let evaluator =
                    |value: f64, parameters: &[f64]| parameters[0] * value.powf(parameters[1]);
                let parameters = nonlinear_fit(x, y, initial, &evaluator)?;
                let equation = format!(
                    "y = {} × x^{}",
                    format_number(parameters[0]),
                    format_number(parameters[1])
                );
                (parameters, equation, Box::new(evaluator))
            }
            FitMethod::Custom {
                expression,
                initial_parameters,
            } => {
                if initial_parameters.is_empty() || initial_parameters.len() > PARAMETER_NAMES.len()
                {
                    return Err(FitError::new(
                        "invalid_parameters",
                        "自定义拟合需要 1 到 8 个初始参数",
                    ));
                }
                if initial_parameters.iter().any(|value| !value.is_finite()) {
                    return Err(FitError::new(
                        "invalid_parameters",
                        "初始参数必须是有限数值",
                    ));
                }
                let normalized = expression
                    .trim()
                    .replace("**", "^")
                    .replace("log10(", "lg(")
                    .replace("log(", "ln(");
                if normalized.is_empty() {
                    return Err(FitError::new("invalid_expression", "自定义表达式不能为空"));
                }
                let parsed = CustomExpression::parse(&normalized, initial_parameters.len())?;
                let evaluator = move |value: f64, parameters: &[f64]| {
                    parsed.evaluate(value, parameters).unwrap_or(f64::NAN)
                };
                let parameters = nonlinear_fit(x, y, initial_parameters.clone(), &evaluator)?;
                let parameter_text = parameters
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        format!("{}={}", PARAMETER_NAMES[index], format_number(*value))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let equation = format!("y = {normalized}  ({parameter_text})");
                (parameters, equation, Box::new(evaluator))
            }
        };

    let predicted = evaluate_all(x, &parameters, evaluator.as_ref())?;
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let residual = y
        .iter()
        .zip(&predicted)
        .map(|(actual, predicted)| (actual - predicted).powi(2))
        .sum::<f64>();
    let total = y.iter().map(|value| (value - mean).powi(2)).sum::<f64>();
    let r2 = if total > 0.0 {
        1.0 - residual / total
    } else {
        0.0
    };
    if !r2.is_finite() {
        return Err(numeric_failure());
    }

    let minimum = x.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut points = Vec::with_capacity(FIT_POINT_COUNT);
    for index in 0..FIT_POINT_COUNT {
        let fraction = index as f64 / (FIT_POINT_COUNT - 1) as f64;
        let value = minimum + (maximum - minimum) * fraction;
        let fitted = evaluator(value, &parameters);
        if !fitted.is_finite() {
            return Err(numeric_failure());
        }
        points.push([value, fitted]);
    }
    Ok(FitResult {
        equation,
        r2,
        points,
        #[cfg(test)]
        parameters,
    })
}

fn validate_values(x: &[f64], y: &[f64]) -> Result<(), FitError> {
    if x.len() != y.len() {
        return Err(FitError::new("shape_mismatch", "X 与 Y 的数据长度不同"));
    }
    if x.len() < 2 {
        return Err(FitError::new("insufficient_points", "至少需要两个数据点"));
    }
    if x.iter().chain(y).any(|value| !value.is_finite()) {
        return Err(FitError::new("invalid_values", "拟合数据包含 NaN 或无穷值"));
    }
    Ok(())
}

struct PolynomialSolution {
    normalized_coefficients: Vec<f64>,
    original_coefficients: Vec<f64>,
    center: f64,
    scale: f64,
}

fn polynomial_fit(x: &[f64], y: &[f64], degree: usize) -> Result<PolynomialSolution, FitError> {
    let minimum = x.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let center = minimum / 2.0 + maximum / 2.0;
    let scale = x
        .iter()
        .map(|value| (value - center).abs())
        .fold(0.0_f64, f64::max);
    if !center.is_finite() || !scale.is_finite() || scale == 0.0 {
        return Err(FitError::new("degenerate_x", "X 值无法确定唯一的多项式"));
    }
    let matrix = DMatrix::from_fn(x.len(), degree + 1, |row, column| {
        ((x[row] - center) / scale).powi((degree - column) as i32)
    });
    let target = DVector::from_column_slice(y);
    let normalized_coefficients: Vec<f64> = matrix
        .svd(true, true)
        .solve(&target, f64::EPSILON.sqrt())
        .ok()
        .map(|solution| solution.iter().copied().collect())
        .filter(|values: &Vec<f64>| values.iter().all(|value| value.is_finite()))
        .ok_or_else(|| FitError::new("solver_failure", "多项式拟合矩阵不可解"))?;
    let original_coefficients = normalized_to_original(&normalized_coefficients, center, scale);
    if original_coefficients.iter().any(|value| !value.is_finite()) {
        return Err(numeric_failure());
    }
    Ok(PolynomialSolution {
        normalized_coefficients,
        original_coefficients,
        center,
        scale,
    })
}

fn normalized_to_original(coefficients: &[f64], center: f64, scale: f64) -> Vec<f64> {
    let alpha = 1.0 / scale;
    let beta = -center / scale;
    let mut coefficients = coefficients.iter();
    let mut ascending = vec![coefficients.next().copied().unwrap_or(0.0)];
    for coefficient in coefficients {
        let mut next = vec![0.0; ascending.len() + 1];
        for (power, value) in ascending.iter().enumerate() {
            next[power] += value * beta;
            next[power + 1] += value * alpha;
        }
        next[0] += coefficient;
        ascending = next;
    }
    ascending.into_iter().rev().collect()
}

fn linear_fit(x: &[f64], y: &[f64]) -> Result<[f64; 2], FitError> {
    let matrix = DMatrix::from_fn(
        x.len(),
        2,
        |row, column| {
            if column == 0 { x[row] } else { 1.0 }
        },
    );
    let target = DVector::from_column_slice(y);
    let solution = matrix
        .svd(true, true)
        .solve(&target, f64::EPSILON.sqrt())
        .ok()
        .ok_or_else(|| FitError::new("solver_failure", "线性拟合矩阵不可解"))?;
    if solution.iter().any(|value| !value.is_finite()) {
        return Err(numeric_failure());
    }
    Ok([solution[0], solution[1]])
}

fn exponential_initial(x: &[f64], y: &[f64]) -> Vec<f64> {
    let sign = if y.iter().all(|value| *value < 0.0) {
        -1.0
    } else {
        1.0
    };
    if y.iter()
        .all(|value| value.signum() == sign && *value != 0.0)
    {
        let log_y: Vec<f64> = y.iter().map(|value| (value * sign).ln()).collect();
        if let Ok([slope, intercept]) = linear_fit(x, &log_y) {
            return vec![sign * intercept.exp(), slope];
        }
    }
    vec![y.iter().sum::<f64>() / y.len() as f64, 0.0]
}

fn nonlinear_fit<F>(
    x: &[f64],
    y: &[f64],
    mut parameters: Vec<f64>,
    evaluator: &F,
) -> Result<Vec<f64>, FitError>
where
    F: Fn(f64, &[f64]) -> f64 + ?Sized,
{
    let parameter_count = parameters.len();
    let mut lambda = 1e-3;
    let mut current_error = squared_error(x, y, &parameters, evaluator)?;
    let mut accepted_any = false;
    for _ in 0..250 {
        let predicted = evaluate_all(x, &parameters, evaluator)?;
        let mut jacobian = DMatrix::zeros(x.len(), parameter_count);
        for column in 0..parameter_count {
            let step = f64::EPSILON.sqrt() * (parameters[column].abs() + 1.0);
            let mut plus = parameters.clone();
            let mut minus = parameters.clone();
            plus[column] += step;
            minus[column] -= step;
            for row in 0..x.len() {
                let high = evaluator(x[row], &plus);
                let low = evaluator(x[row], &minus);
                let derivative = (high - low) / (2.0 * step);
                if !derivative.is_finite() {
                    return Err(numeric_failure());
                }
                jacobian[(row, column)] = derivative;
            }
        }
        let residual = DVector::from_iterator(
            y.len(),
            y.iter()
                .zip(&predicted)
                .map(|(actual, model)| actual - model),
        );
        let transpose = jacobian.transpose();
        let mut normal = &transpose * &jacobian;
        for index in 0..parameter_count {
            normal[(index, index)] += lambda * (normal[(index, index)].abs() + 1.0);
        }
        let gradient = transpose * residual;
        let Some(delta) = normal.lu().solve(&gradient) else {
            lambda *= 10.0;
            if lambda > 1e16 {
                break;
            }
            continue;
        };
        let candidate: Vec<f64> = parameters
            .iter()
            .zip(delta.iter())
            .map(|(parameter, change)| parameter + change)
            .collect();
        let candidate_error = squared_error(x, y, &candidate, evaluator);
        if let Ok(candidate_error) = candidate_error
            && candidate_error < current_error
        {
            let improvement = current_error - candidate_error;
            let change_norm = delta.norm();
            let parameter_norm = DVector::from_column_slice(&candidate).norm();
            parameters = candidate;
            current_error = candidate_error;
            accepted_any = true;
            lambda = (lambda / 3.0).max(1e-12);
            if improvement <= 1e-12 * (1.0 + current_error)
                || change_norm <= 1e-10 * (1.0 + parameter_norm)
            {
                break;
            }
        } else {
            lambda *= 10.0;
            if lambda > 1e16 {
                break;
            }
        }
    }
    if !accepted_any && !current_error.is_finite() {
        return Err(FitError::new("solver_failure", "非线性拟合未能收敛"));
    }
    Ok(parameters)
}

fn squared_error<F>(
    x: &[f64],
    y: &[f64],
    parameters: &[f64],
    evaluator: &F,
) -> Result<f64, FitError>
where
    F: Fn(f64, &[f64]) -> f64 + ?Sized,
{
    let predicted = evaluate_all(x, parameters, evaluator)?;
    let error = y
        .iter()
        .zip(predicted)
        .map(|(actual, predicted)| (actual - predicted).powi(2))
        .sum::<f64>();
    error
        .is_finite()
        .then_some(error)
        .ok_or_else(numeric_failure)
}

fn evaluate_all<F>(x: &[f64], parameters: &[f64], evaluator: &F) -> Result<Vec<f64>, FitError>
where
    F: Fn(f64, &[f64]) -> f64 + ?Sized,
{
    x.iter()
        .map(|value| {
            let result = evaluator(*value, parameters);
            result
                .is_finite()
                .then_some(result)
                .ok_or_else(numeric_failure)
        })
        .collect()
}

struct CustomExpression {
    slab: Slab,
    expression: ExpressionI,
}

impl CustomExpression {
    fn parse(source: &str, parameter_count: usize) -> Result<Self, FitError> {
        validate_custom_identifiers(source, parameter_count)?;
        let mut slab = Slab::new();
        let expression = Parser::new().parse(source, &mut slab.ps).map_err(|error| {
            FitError::new("invalid_expression", format!("无法解析表达式：{error}"))
        })?;
        let parsed = Self { slab, expression };
        parsed
            .evaluate(1.0, &vec![1.0; parameter_count])
            .map_err(|error| {
                FitError::new(
                    "invalid_expression",
                    format!("表达式包含不支持的内容：{error}"),
                )
            })?;
        Ok(parsed)
    }

    fn evaluate(&self, x: f64, parameters: &[f64]) -> Result<f64, fasteval2::Error> {
        let mut namespace = |name: &str, arguments: Vec<f64>| -> Option<f64> {
            match name {
                "x" if arguments.is_empty() => Some(x),
                "pi" if arguments.is_empty() => Some(std::f64::consts::PI),
                "e" if arguments.is_empty() => Some(std::f64::consts::E),
                "exp" if arguments.len() == 1 => Some(arguments[0].exp()),
                "ln" if arguments.len() == 1 => Some(arguments[0].ln()),
                "lg" if arguments.len() == 1 => Some(arguments[0].log10()),
                "sqrt" if arguments.len() == 1 => Some(arguments[0].sqrt()),
                "arctan" if arguments.len() == 1 => Some(arguments[0].atan()),
                "arctan2" if arguments.len() == 2 => Some(arguments[0].atan2(arguments[1])),
                _ => PARAMETER_NAMES
                    .iter()
                    .position(|parameter| *parameter == name)
                    .filter(|_| arguments.is_empty())
                    .and_then(|index| parameters.get(index).copied()),
            }
        };
        self.expression
            .from(&self.slab.ps)
            .eval(&self.slab, &mut namespace)
    }
}

fn validate_custom_identifiers(source: &str, parameter_count: usize) -> Result<(), FitError> {
    const FUNCTIONS: [&str; 15] = [
        "sin", "cos", "tan", "sinh", "cosh", "tanh", "exp", "ln", "lg", "sqrt", "abs", "arctan",
        "arctan2", "pi", "e",
    ];
    let characters: Vec<char> = source.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if !(character.is_ascii_alphabetic() || character == '_') {
            index += 1;
            continue;
        }
        if matches!(character, 'e' | 'E')
            && index > 0
            && characters[index - 1].is_ascii_digit()
            && characters
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit() || matches!(next, '+' | '-'))
        {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < characters.len()
            && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
        {
            index += 1;
        }
        let identifier: String = characters[start..index].iter().collect();
        let is_parameter = PARAMETER_NAMES[..parameter_count].contains(&identifier.as_str());
        if identifier != "x" && !is_parameter && !FUNCTIONS.contains(&identifier.as_str()) {
            return Err(FitError::new(
                "invalid_expression",
                format!("不支持的符号：{identifier}"),
            ));
        }
    }
    Ok(())
}

fn polynomial_equation(coefficients: &[f64]) -> String {
    let degree = coefficients.len() - 1;
    let terms = coefficients
        .iter()
        .enumerate()
        .filter(|(_, coefficient)| coefficient.abs() >= 1e-10)
        .map(|(index, coefficient)| {
            let power = degree - index;
            match power {
                0 => format_number(*coefficient),
                1 => format!("{}×x", format_number(*coefficient)),
                _ => format!("{}×x^{power}", format_number(*coefficient)),
            }
        })
        .collect::<Vec<_>>();
    let body = if terms.is_empty() {
        "0".to_owned()
    } else {
        terms.join(" + ").replace("+ -", "- ")
    };
    format!("y = {body}")
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if value.abs() >= 10_000.0 || value.abs() < 0.001 {
        format!("{value:.4e}")
    } else {
        format!("{value:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn numeric_failure() -> FitError {
    FitError::new("numeric_failure", "拟合产生了无效数值")
}

#[cfg(test)]
mod tests {
    use super::{FitMethod, fit_values};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn fits_quadratic_and_reports_r_squared() {
        let x = [-2.0, -1.0, 0.0, 1.0, 2.0];
        let y: Vec<f64> = x
            .iter()
            .map(|value| 2.0 * value * value - 3.0 * value + 4.0)
            .collect();
        let result = fit_values(&x, &y, &FitMethod::Polynomial { degree: 2 }).unwrap();
        assert_close(result.parameters[0], 2.0, 1e-10);
        assert_close(result.parameters[1], -3.0, 1e-10);
        assert_close(result.parameters[2], 4.0, 1e-10);
        assert_close(result.r2, 1.0, 1e-12);
        assert_eq!(result.points.len(), 500);
    }

    #[test]
    fn polynomial_fit_stays_stable_for_large_offset_x_values() {
        let x: Vec<f64> = (0..20)
            .map(|index| 1_775_000_000.0 + index as f64)
            .collect();
        let y: Vec<f64> = x
            .iter()
            .map(|value| {
                let local = value - 1_775_000_000.0;
                0.5 * local * local - 2.0 * local + 7.0
            })
            .collect();
        let result = fit_values(&x, &y, &FitMethod::Polynomial { degree: 2 }).unwrap();
        assert!(result.r2 > 0.999_999_999);
        assert!(
            result
                .points
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn fits_exponential_logarithmic_and_power_models() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let exponential: Vec<f64> = x
            .iter()
            .map(|value| 2.5 * (0.3_f64 * value).exp())
            .collect();
        let result = fit_values(&x, &exponential, &FitMethod::Exponential).unwrap();
        assert_close(result.parameters[0], 2.5, 1e-6);
        assert_close(result.parameters[1], 0.3, 1e-6);

        let logarithmic: Vec<f64> = x.iter().map(|value| 3.0 * value.ln() - 2.0).collect();
        let result = fit_values(&x, &logarithmic, &FitMethod::Logarithmic).unwrap();
        assert_close(result.parameters[0], 3.0, 1e-10);
        assert_close(result.parameters[1], -2.0, 1e-10);

        let power: Vec<f64> = x.iter().map(|value| 1.7 * value.powf(2.2)).collect();
        let result = fit_values(&x, &power, &FitMethod::Power).unwrap();
        assert_close(result.parameters[0], 1.7, 1e-5);
        assert_close(result.parameters[1], 2.2, 1e-5);
    }

    #[test]
    fn fits_custom_sine_expression() {
        let x: Vec<f64> = (0..30).map(|index| index as f64 * 0.1).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|value| 2.0 * (1.5 * value + 0.2).sin())
            .collect();
        let result = fit_values(
            &x,
            &y,
            &FitMethod::Custom {
                expression: "a * sin(b * x + c)".to_owned(),
                initial_parameters: vec![1.8, 1.4, 0.1],
            },
        )
        .unwrap();
        assert!(result.r2 > 0.999_999);
    }

    #[test]
    fn rejects_invalid_domains_and_parameters() {
        let result = fit_values(&[0.0, 1.0], &[1.0, 2.0], &FitMethod::Logarithmic);
        assert_eq!(result.unwrap_err().code, "invalid_domain");
        let result = fit_values(
            &[1.0, 2.0],
            &[1.0, 2.0],
            &FitMethod::Custom {
                expression: "a*x".to_owned(),
                initial_parameters: Vec::new(),
            },
        );
        assert_eq!(result.unwrap_err().code, "invalid_parameters");

        let result = fit_values(
            &[1.0, 2.0, 3.0],
            &[1.0, 2.0, 3.0],
            &FitMethod::Custom {
                expression: "print(x) + unknown".to_owned(),
                initial_parameters: vec![1.0],
            },
        );
        assert_eq!(result.unwrap_err().code, "invalid_expression");
    }
}
