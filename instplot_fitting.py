"""GUI-independent curve fitting with stable results and errors."""

from __future__ import annotations

import ast
from dataclasses import dataclass
import warnings
from typing import Callable, Sequence

import numpy as np
from scipy.optimize import curve_fit


class FitError(RuntimeError):
    def __init__(self, code: str, reason: str):
        self.code = code
        self.reason = reason
        super().__init__(f"{code}: {reason}")


@dataclass(frozen=True)
class FitResult:
    equation: str
    r2: float
    x_fit: np.ndarray
    y_fit: np.ndarray
    parameters: tuple[float, ...]


_FUNCTIONS = {
    "sin": np.sin,
    "cos": np.cos,
    "tan": np.tan,
    "sinh": np.sinh,
    "cosh": np.cosh,
    "tanh": np.tanh,
    "exp": np.exp,
    "log": np.log,
    "log10": np.log10,
    "sqrt": np.sqrt,
    "abs": np.abs,
    "arctan": np.arctan,
    "arctan2": np.arctan2,
}
_CONSTANTS = {"pi": np.pi, "e": np.e}
_PARAMETER_NAMES = ("a", "b", "c", "d", "e_param", "f", "g", "h")
_ALLOWED_NODES = (
    ast.Expression,
    ast.BinOp,
    ast.UnaryOp,
    ast.Call,
    ast.Load,
    ast.Add,
    ast.Sub,
    ast.Mult,
    ast.Div,
    ast.Pow,
    ast.Mod,
    ast.USub,
    ast.UAdd,
    ast.Constant,
    ast.Name,
)


def _values(values, name: str) -> np.ndarray:
    try:
        raw = np.asarray(values)
    except Exception as error:
        raise FitError("invalid_values", f"{name}: {error}") from error
    if raw.ndim != 1:
        raise FitError("invalid_values", f"{name} must be one-dimensional")
    if np.iscomplexobj(raw):
        raise FitError("invalid_values", f"{name} must be real")
    try:
        result = raw.astype(float, copy=True)
    except (TypeError, ValueError) as error:
        raise FitError("invalid_values", f"{name}: {error}") from error
    if not np.all(np.isfinite(result)):
        raise FitError("invalid_values", f"{name} contains NaN or infinity")
    return result


def _custom_function(expression: str, parameter_count: int):
    expression = (expression or "").strip().replace("^", "**")
    if not expression:
        raise FitError("invalid_expression", "custom expression is empty")
    if parameter_count < 1 or parameter_count > len(_PARAMETER_NAMES):
        raise FitError("invalid_parameters", "custom fit requires 1 to 8 parameters")
    try:
        tree = ast.parse(expression, mode="eval")
        allowed_names = set(_FUNCTIONS) | set(_CONSTANTS) | {"x"} | set(_PARAMETER_NAMES)
        for node in ast.walk(tree):
            if not isinstance(node, _ALLOWED_NODES):
                raise ValueError("contains unsupported syntax")
            if isinstance(node, ast.Name) and node.id not in allowed_names:
                raise ValueError(f"unsupported symbol: {node.id}")
            if isinstance(node, ast.Call):
                if not isinstance(node.func, ast.Name) or node.func.id not in _FUNCTIONS:
                    raise ValueError("unsupported function call")
        code = compile(tree, "<fit-expression>", "eval")
    except (SyntaxError, ValueError) as error:
        raise FitError("invalid_expression", str(error)) from error

    def function(x, *parameters):
        environment = dict(_FUNCTIONS)
        environment.update(_CONSTANTS)
        environment["x"] = x
        environment.update(zip(_PARAMETER_NAMES, parameters))
        return np.asarray(eval(code, {"__builtins__": {}}, environment), dtype=float)

    return expression, function


def fit_values(
    x,
    y,
    method: str,
    *,
    degree: int = 2,
    expression: str | None = None,
    initial_parameters: Sequence[float] | None = None,
    cancel_check: Callable[[], None] | None = None,
) -> FitResult:
    check = cancel_check or (lambda: None)
    check()
    x_values = _values(x, "x")
    y_values = _values(y, "y")
    if x_values.shape != y_values.shape:
        raise FitError("shape_mismatch", "x and y lengths differ")
    if len(x_values) < 2:
        raise FitError("insufficient_points", "at least two points are required")

    parameters: tuple[float, ...]
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            with np.errstate(all="raise"):
                if method == "polynomial":
                    if not isinstance(degree, (int, np.integer)) or isinstance(degree, (bool, np.bool_)):
                        raise FitError("invalid_parameters", "degree must be an integer")
                    if degree < 1 or degree > 10:
                        raise FitError("invalid_parameters", "degree must be between 1 and 10")
                    if len(x_values) <= degree:
                        raise FitError("insufficient_points", "point count must exceed polynomial degree")
                    coefficients = np.polyfit(x_values, y_values, int(degree))
                    function = lambda values, *coeffs: np.polyval(coeffs, values)
                    y_pred = function(x_values, *coefficients)
                    terms = []
                    for index, coefficient in enumerate(coefficients):
                        power = degree - index
                        if abs(coefficient) < 1e-10:
                            continue
                        suffix = "" if power == 0 else "*x" if power == 1 else f"*x^{power}"
                        terms.append(f"{coefficient:.4g}{suffix}")
                    equation = "y = " + " + ".join(terms).replace("+ -", "- ")
                    parameters = tuple(float(value) for value in coefficients)
                elif method == "exponential":
                    function = lambda values, a, b: a * np.exp(b * values)
                    fitted, _covariance = curve_fit(function, x_values, y_values, maxfev=5000)
                    y_pred = function(x_values, *fitted)
                    equation = f"y = {fitted[0]:.4g} * exp({fitted[1]:.4g} * x)"
                    parameters = tuple(float(value) for value in fitted)
                elif method == "logarithmic":
                    if np.any(x_values <= 0):
                        raise FitError("invalid_domain", "logarithmic fit requires x > 0")
                    function = lambda values, a, b: a * np.log(values) + b
                    fitted, _covariance = curve_fit(function, x_values, y_values, maxfev=5000)
                    y_pred = function(x_values, *fitted)
                    equation = f"y = {fitted[0]:.4g} * log(x) + {fitted[1]:.4g}"
                    parameters = tuple(float(value) for value in fitted)
                elif method == "power":
                    if np.any(x_values <= 0) or np.any(y_values <= 0):
                        raise FitError("invalid_domain", "power fit requires x and y > 0")
                    function = lambda values, a, b: a * np.power(values, b)
                    fitted, _covariance = curve_fit(function, x_values, y_values, maxfev=5000)
                    y_pred = function(x_values, *fitted)
                    equation = f"y = {fitted[0]:.4g} * x^{fitted[1]:.4g}"
                    parameters = tuple(float(value) for value in fitted)
                elif method == "custom":
                    initial = tuple(float(value) for value in (initial_parameters or (1.0, 1.0, 1.0)))
                    normalized_expression, function = _custom_function(expression or "", len(initial))
                    fitted, _covariance = curve_fit(
                        function, x_values, y_values, p0=initial, maxfev=10000
                    )
                    y_pred = function(x_values, *fitted)
                    parameter_text = ", ".join(
                        f"{_PARAMETER_NAMES[index]}={value:.4g}"
                        for index, value in enumerate(fitted)
                    )
                    equation = f"y = {normalized_expression}  ({parameter_text})"
                    parameters = tuple(float(value) for value in fitted)
                else:
                    raise FitError("invalid_method", f"unsupported fit method: {method}")
    except FitError:
        raise
    except Exception as error:
        raise FitError("solver_failure", str(error)) from error

    check()
    x_min, x_max = float(np.min(x_values)), float(np.max(x_values))
    if method in {"logarithmic", "power"}:
        x_min = max(x_min, 1e-10)
    x_fit = np.linspace(x_min, x_max, 500)
    try:
        with np.errstate(all="raise"):
            y_fit = np.asarray(function(x_fit, *parameters), dtype=float)
            residual = np.sum((y_values - y_pred) ** 2)
            total = np.sum((y_values - np.mean(y_values)) ** 2)
            r2 = float(1 - residual / total) if total > 0 else 0.0
    except Exception as error:
        raise FitError("numeric_failure", str(error)) from error
    if not np.all(np.isfinite(y_fit)) or not np.isfinite(r2):
        raise FitError("numeric_failure", "fit produced non-finite output")
    check()
    return FitResult(equation, r2, x_fit, y_fit, parameters)
