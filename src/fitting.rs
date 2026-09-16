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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormulaAxis {
    X,
    Y,
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
                let normalized = normalize_expression(expression);
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

/// Evaluates a row-by-row data-processing formula using the same expression
/// syntax as custom fitting. `x` and `y` refer to the selected plot columns;
/// `a` and `b` are user-provided scalar parameters.
pub fn evaluate_formula_values(
    source: &str,
    x: &[f64],
    y: &[f64],
    a: f64,
    b: f64,
) -> Result<Vec<f64>, FitError> {
    if x.len() != y.len() {
        return Err(FitError::new("invalid_data", "X 列和 Y 列长度不一致"));
    }
    if !a.is_finite() || !b.is_finite() {
        return Err(FitError::new("invalid_parameters", "a 和 b 必须是有限数值"));
    }
    let normalized = normalize_expression(source);
    if normalized.is_empty() {
        return Err(FitError::new("invalid_expression", "公式不能为空"));
    }
    let axis = formula_output_axis(&normalized)?;
    let parsed = FormulaExpression::parse(&normalized)?;
    x.iter()
        .zip(y)
        .enumerate()
        .map(|(row, (x, y))| {
            let source_value = match axis {
                FormulaAxis::X => x,
                FormulaAxis::Y => y,
            };
            if !source_value.is_finite() {
                return Ok(f64::NAN);
            }
            let value = parsed.evaluate(*x, *y, a, b).map_err(|error| {
                FitError::new(
                    "formula_error",
                    format!("第 {} 行无法计算：{error}", row + 1),
                )
            })?;
            value.is_finite().then_some(value).ok_or_else(|| {
                FitError::new(
                    "formula_error",
                    format!("第 {} 行结果不是有限数值", row + 1),
                )
            })
        })
        .collect()
}

pub fn formula_output_axis(source: &str) -> Result<FormulaAxis, FitError> {
    let normalized = normalize_expression(source);
    if normalized.is_empty() {
        return Err(FitError::new("invalid_expression", "公式不能为空"));
    }
    if let Some(hint) = expression_syntax_hint(&normalized) {
        return Err(FitError::new("invalid_expression", hint));
    }
    validate_expression_identifiers(&normalized, &["x", "y", "a", "b"])?;
    let characters: Vec<char> = normalized.chars().collect();
    let mut index = 0;
    let mut uses_x = false;
    let mut uses_y = false;
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
        match characters[start..index].iter().collect::<String>().as_str() {
            "x" => uses_x = true,
            "y" => uses_y = true,
            _ => {}
        }
    }
    match (uses_x, uses_y) {
        (true, false) => Ok(FormulaAxis::X),
        (false, true) => Ok(FormulaAxis::Y),
        (false, false) => Err(FitError::new(
            "invalid_expression",
            "公式必须引用 x 或 y，才能确定要生成的列",
        )),
        (true, true) => Err(FitError::new(
            "ambiguous_formula",
            "公式同时引用 x 和 y；请拆成一次 X 处理或一次 Y 处理",
        )),
    }
}

pub fn evaluate_constant_expression(source: &str) -> Result<f64, FitError> {
    let normalized = normalize_expression(source);
    if normalized.is_empty() {
        return Err(FitError::new("invalid_expression", "数值表达式不能为空"));
    }
    validate_expression_identifiers(&normalized, &[])?;
    let mut slab = Slab::new();
    let expression = Parser::new()
        .parse(&normalized, &mut slab.ps)
        .map_err(|error| {
            FitError::new(
                "invalid_expression",
                friendly_parse_error("无法解析数值表达式", &normalized, error),
            )
        })?;
    let mut namespace = |name: &str, arguments: Vec<f64>| -> Option<f64> {
        match name {
            "pi" if arguments.is_empty() => Some(std::f64::consts::PI),
            "e" if arguments.is_empty() => Some(std::f64::consts::E),
            "exp" if arguments.len() == 1 => Some(arguments[0].exp()),
            "ln" if arguments.len() == 1 => Some(arguments[0].ln()),
            "lg" if arguments.len() == 1 => Some(arguments[0].log10()),
            "sqrt" if arguments.len() == 1 => Some(arguments[0].sqrt()),
            "arctan" if arguments.len() == 1 => Some(arguments[0].atan()),
            "arctan2" if arguments.len() == 2 => Some(arguments[0].atan2(arguments[1])),
            _ => None,
        }
    };
    let value = expression
        .from(&slab.ps)
        .eval(&slab, &mut namespace)
        .map_err(|error| {
            FitError::new("invalid_expression", format!("无法计算数值表达式：{error}"))
        })?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| FitError::new("invalid_expression", "数值表达式结果必须是有限数值"))
}

fn normalize_expression(source: &str) -> String {
    source
        .trim()
        .replace("**", "^")
        .replace("log10(", "lg(")
        .replace("log(", "ln(")
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
        let mut variables = vec!["x"];
        variables.extend_from_slice(&PARAMETER_NAMES[..parameter_count]);
        validate_expression_identifiers(source, &variables)?;
        let mut slab = Slab::new();
        let expression = Parser::new().parse(source, &mut slab.ps).map_err(|error| {
            FitError::new(
                "invalid_expression",
                friendly_parse_error("无法解析表达式", source, error),
            )
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

struct FormulaExpression {
    slab: Slab,
    expression: ExpressionI,
}

impl FormulaExpression {
    fn parse(source: &str) -> Result<Self, FitError> {
        validate_expression_identifiers(source, &["x", "y", "a", "b"])?;
        let mut slab = Slab::new();
        let expression = Parser::new().parse(source, &mut slab.ps).map_err(|error| {
            FitError::new(
                "invalid_expression",
                friendly_parse_error("无法解析公式", source, error),
            )
        })?;
        Ok(Self { slab, expression })
    }

    fn evaluate(&self, x: f64, y: f64, a: f64, b: f64) -> Result<f64, fasteval2::Error> {
        let mut namespace = |name: &str, arguments: Vec<f64>| -> Option<f64> {
            match name {
                "x" if arguments.is_empty() => Some(x),
                "y" if arguments.is_empty() => Some(y),
                "a" if arguments.is_empty() => Some(a),
                "b" if arguments.is_empty() => Some(b),
                "pi" if arguments.is_empty() => Some(std::f64::consts::PI),
                "e" if arguments.is_empty() => Some(std::f64::consts::E),
                "exp" if arguments.len() == 1 => Some(arguments[0].exp()),
                "ln" if arguments.len() == 1 => Some(arguments[0].ln()),
                "lg" if arguments.len() == 1 => Some(arguments[0].log10()),
                "sqrt" if arguments.len() == 1 => Some(arguments[0].sqrt()),
                "arctan" if arguments.len() == 1 => Some(arguments[0].atan()),
                "arctan2" if arguments.len() == 2 => Some(arguments[0].atan2(arguments[1])),
                _ => None,
            }
        };
        self.expression
            .from(&self.slab.ps)
            .eval(&self.slab, &mut namespace)
    }
}

fn validate_expression_identifiers(source: &str, variables: &[&str]) -> Result<(), FitError> {
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
        if !variables.contains(&identifier.as_str()) && !FUNCTIONS.contains(&identifier.as_str()) {
            return Err(FitError::new(
                "invalid_expression",
                format!("不支持的符号：{identifier}"),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SyntaxTokenKind {
    Number,
    Identifier(String),
    LeftParen,
    RightParen,
    Operator(char),
    Comma,
}

#[derive(Clone, Debug)]
struct SyntaxToken {
    kind: SyntaxTokenKind,
    position: usize,
}

fn friendly_parse_error(context: &str, source: &str, error: impl fmt::Display) -> String {
    expression_syntax_hint(source)
        .map(|hint| format!("{context}：{hint}"))
        .unwrap_or_else(|| format!("{context}：{error}"))
}

fn expression_syntax_hint(source: &str) -> Option<String> {
    const FUNCTIONS: [&str; 13] = [
        "sin", "cos", "tan", "sinh", "cosh", "tanh", "exp", "ln", "lg", "sqrt", "abs", "arctan",
        "arctan2",
    ];
    let characters = source.char_indices().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0_usize;
    let mut parentheses = Vec::new();
    while index < characters.len() {
        let (byte_position, character) = characters[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }
        let position = source[..byte_position].chars().count() + 1;
        if character == '(' {
            parentheses.push(position);
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::LeftParen,
                position,
            });
            index += 1;
            continue;
        }
        if character == ')' {
            if parentheses.pop().is_none() {
                return Some(format!("第 {position} 个字符的 ) 没有对应的 ("));
            }
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::RightParen,
                position,
            });
            index += 1;
            continue;
        }
        if matches!(character, '+' | '-' | '*' | '/' | '^') {
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Operator(character),
                position,
            });
            index += 1;
            continue;
        }
        if character == ',' {
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Comma,
                position,
            });
            index += 1;
            continue;
        }
        if character.is_ascii_digit()
            || (character == '.'
                && characters
                    .get(index + 1)
                    .is_some_and(|(_, next)| next.is_ascii_digit()))
        {
            index += 1;
            while index < characters.len() && characters[index].1.is_ascii_digit() {
                index += 1;
            }
            if index < characters.len() && characters[index].1 == '.' {
                index += 1;
                while index < characters.len() && characters[index].1.is_ascii_digit() {
                    index += 1;
                }
            }
            if index < characters.len() && matches!(characters[index].1, 'e' | 'E') {
                let exponent = index;
                index += 1;
                if index < characters.len() && matches!(characters[index].1, '+' | '-') {
                    index += 1;
                }
                let digits = index;
                while index < characters.len() && characters[index].1.is_ascii_digit() {
                    index += 1;
                }
                if index == digits {
                    index = exponent;
                }
            }
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Number,
                position,
            });
            continue;
        }
        if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < characters.len()
                && (characters[index].1.is_ascii_alphanumeric() || characters[index].1 == '_')
            {
                index += 1;
            }
            let start_byte = characters[start].0;
            let end_byte = characters
                .get(index)
                .map_or(source.len(), |(byte_position, _)| *byte_position);
            tokens.push(SyntaxToken {
                kind: SyntaxTokenKind::Identifier(source[start_byte..end_byte].to_owned()),
                position,
            });
            continue;
        }
        return Some(format!("第 {position} 个字符“{character}”不受支持"));
    }
    if let Some(position) = parentheses.last() {
        return Some(format!("第 {position} 个字符的 ( 缺少对应的 )"));
    }
    for (token_index, token) in tokens.iter().enumerate() {
        let next = tokens.get(token_index + 1);
        if let SyntaxTokenKind::Identifier(name) = &token.kind
            && FUNCTIONS.contains(&name.as_str())
            && !next.is_some_and(|next| next.kind == SyntaxTokenKind::LeftParen)
        {
            return Some(format!("函数 {name} 后缺少 (，例如 {name}(x)"));
        }
        let Some(next) = next else {
            if let SyntaxTokenKind::Operator(operator) = token.kind {
                return Some(format!("公式不能以运算符 {operator} 结尾"));
            }
            continue;
        };
        let left_is_value = matches!(
            token.kind,
            SyntaxTokenKind::Number | SyntaxTokenKind::Identifier(_) | SyntaxTokenKind::RightParen
        );
        let right_is_value = matches!(
            next.kind,
            SyntaxTokenKind::Number | SyntaxTokenKind::Identifier(_) | SyntaxTokenKind::LeftParen
        );
        let function_call = matches!(&token.kind, SyntaxTokenKind::Identifier(name) if FUNCTIONS.contains(&name.as_str()))
            && next.kind == SyntaxTokenKind::LeftParen;
        if left_is_value && right_is_value && !function_call {
            return Some(format!(
                "第 {} 个字符附近缺少乘号 *，例如写成 … * …",
                next.position
            ));
        }
    }
    None
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
    use super::{
        FitMethod, FormulaAxis, evaluate_constant_expression, fit_values, formula_output_axis,
    };

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
    fn formula_axis_follows_the_referenced_coordinate() {
        assert_eq!(formula_output_axis("2 * x + 1").unwrap(), FormulaAxis::X);
        assert_eq!(formula_output_axis("sqrt(y)").unwrap(), FormulaAxis::Y);
        assert_eq!(
            formula_output_axis("x + y").unwrap_err().code,
            "ambiguous_formula"
        );
        assert_eq!(
            formula_output_axis("a + b").unwrap_err().code,
            "invalid_expression"
        );
    }

    #[test]
    fn constant_expressions_support_fractions_parentheses_and_constants() {
        assert_close(
            evaluate_constant_expression("10 / 11").unwrap(),
            10.0 / 11.0,
            1e-12,
        );
        assert_close(
            evaluate_constant_expression("(2 + 3) / 7").unwrap(),
            5.0 / 7.0,
            1e-12,
        );
        assert_close(
            evaluate_constant_expression("pi / 2").unwrap(),
            std::f64::consts::FRAC_PI_2,
            1e-12,
        );
        assert!(evaluate_constant_expression("1 / 0").is_err());
    }

    #[test]
    fn syntax_errors_explain_common_missing_characters() {
        let missing_multiply = formula_output_axis("2x + 1").unwrap_err();
        assert!(missing_multiply.reason.contains("缺少乘号 *"));

        let missing_function_parenthesis = formula_output_axis("sin x").unwrap_err();
        assert!(
            missing_function_parenthesis
                .reason
                .contains("函数 sin 后缺少 (")
        );

        let missing_closing_parenthesis = formula_output_axis("sin(x").unwrap_err();
        assert!(missing_closing_parenthesis.reason.contains("缺少对应的 )"));

        let extra_closing_parenthesis = formula_output_axis("x + 1)").unwrap_err();
        assert!(extra_closing_parenthesis.reason.contains("没有对应的 ("));
    }

    #[test]
    fn syntax_hints_do_not_reject_valid_functions_or_scientific_notation() {
        assert_eq!(
            formula_output_axis("1e-3 * sin(x)").unwrap(),
            FormulaAxis::X
        );
        assert_close(
            evaluate_constant_expression("1e-3 + sin(pi / 2)").unwrap(),
            1.001,
            1e-12,
        );
    }

    #[test]
    fn custom_fit_reports_a_missing_multiplication_sign() {
        let error = fit_values(
            &[0.0, 1.0, 2.0],
            &[0.0, 1.0, 2.0],
            &FitMethod::Custom {
                expression: "a x".to_owned(),
                initial_parameters: vec![1.0],
            },
        )
        .unwrap_err();
        assert!(error.reason.contains("缺少乘号 *"));
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
