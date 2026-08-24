"""Qt-independent numerical processing core for PlotApp."""

from __future__ import annotations

from dataclasses import dataclass, field
from numbers import Integral, Real
from typing import Any
import warnings

import numpy as np
import pandas as pd
from scipy.signal import savgol_filter


class ProcessingError(ValueError):
    def __init__(self, operation: str, code: str, reason: str):
        self.operation = operation
        self.code = code
        self.reason = reason
        super().__init__(f"{operation}:{code}: {reason}")


@dataclass(frozen=True)
class ProcessingResult:
    values: np.ndarray
    changed_mask: np.ndarray
    metadata: dict[str, Any] = field(default_factory=dict)


def _values(values, operation: str) -> np.ndarray:
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            raw = np.asarray(values)
    except Exception as error:
        raise ProcessingError(
            operation, "invalid_dimensions", "values must form a one-dimensional array"
        ) from error
    if raw.ndim != 1:
        raise ProcessingError(operation, "invalid_dimensions", "values must be one-dimensional")
    if raw.size == 0:
        raise ProcessingError(operation, "empty_values", "values must not be empty")

    if raw.dtype == object and any(
        isinstance(item, (list, tuple, np.ndarray, pd.Series, pd.Index, pd.DataFrame))
        for item in raw
    ):
        raise ProcessingError(
            operation, "invalid_dimensions", "values must not contain nested arrays"
        )
    if np.iscomplexobj(raw) or (
        raw.dtype == object
        and any(isinstance(item, (complex, np.complexfloating)) for item in raw)
    ):
        raise ProcessingError(operation, "invalid_values", "complex values are not supported")

    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            numeric = np.asarray(pd.to_numeric(raw, errors="coerce"))
            if np.iscomplexobj(numeric):
                raise ProcessingError(
                    operation, "invalid_values", "complex values are not supported"
                )
            array = np.asarray(numeric, dtype=float)
    except ProcessingError:
        raise
    except Exception as error:
        raise ProcessingError(operation, "invalid_values", "values cannot be converted to numbers") from error
    return array.copy()


def _integer(value, operation: str, code: str, name: str) -> int:
    if isinstance(value, (bool, np.bool_)) or not isinstance(value, (Integral, np.integer)):
        raise ProcessingError(operation, code, f"{name} must be an integer")
    return int(value)


def _finite_real(value, operation: str, code: str, name: str) -> float:
    if isinstance(value, (bool, np.bool_)) or not isinstance(value, Real):
        raise ProcessingError(operation, code, f"{name} must be a finite number")
    converted = float(value)
    if not np.isfinite(converted):
        raise ProcessingError(operation, code, f"{name} must be a finite number")
    return converted


def _guard_numeric(operation: str, calculation):
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error")
            with np.errstate(over="raise", divide="raise", invalid="raise"):
                return calculation()
    except (Warning, FloatingPointError, OverflowError, ValueError, np.linalg.LinAlgError) as error:
        raise ProcessingError(operation, "numeric_failure", "numerical calculation failed") from error


def _result(original: np.ndarray, values: np.ndarray, **metadata) -> ProcessingResult:
    changed = ~np.isclose(original, values, equal_nan=True)
    return ProcessingResult(values=values, changed_mask=changed, metadata=metadata)


def center_values(values) -> ProcessingResult:
    original = _values(values, "center")
    finite = np.isfinite(original)
    if not finite.any():
        raise ProcessingError("center", "no_finite_values", "no finite values to center")
    def calculate():
        midpoint = original[finite].max() / 2.0 + original[finite].min() / 2.0
        result = original.copy()
        result[finite] -= midpoint
        return midpoint, result

    midpoint, result = _guard_numeric("center", calculate)
    return _result(original, result, center=midpoint)


def normalize_values(values, top_n: int = 20) -> ProcessingResult:
    original = _values(values, "normalize")
    top_n = _integer(top_n, "normalize", "invalid_top_n", "top_n")
    if top_n <= 0:
        raise ProcessingError("normalize", "invalid_top_n", "top_n must be positive")
    finite = np.isfinite(original)
    if not finite.any():
        raise ProcessingError("normalize", "no_finite_values", "no finite values to normalize")
    valid = original[finite]
    count = min(int(top_n), len(valid))
    def calculate():
        selected = np.partition(valid, -count)[-count:]
        magnitude = float(np.max(np.abs(selected)))
        scale = 0.0 if magnitude == 0.0 else float(magnitude * np.mean(selected / magnitude))
        result = original.copy()
        if scale != 0.0:
            finite_values = valid.copy()
            above = valid > scale
            below = (~above) & (valid < -scale)
            middle = ~(above | below)
            finite_values[above] = 1.0
            finite_values[below] = -1.0
            finite_values[middle] = valid[middle] / scale
            result[finite] = finite_values
        return scale, result

    scale, result = _guard_numeric("normalize", calculate)
    return _result(original, result, scale=scale, top_n=count)


def local_flatten_values(
    x,
    y,
    x1: float,
    x2: float,
    transition: float = 0,
    anchor: str = "left",
    strength: float = 1.0,
) -> ProcessingResult:
    x_values = _values(x, "local_flatten")
    original = _values(y, "local_flatten")
    if len(x_values) != len(original):
        raise ProcessingError("local_flatten", "length_mismatch", "x and y lengths differ")
    if anchor not in {"left", "right", "center"}:
        raise ProcessingError("local_flatten", "invalid_anchor", "anchor must be left, right, or center")
    x1 = _finite_real(x1, "local_flatten", "invalid_interval", "x1")
    x2 = _finite_real(x2, "local_flatten", "invalid_interval", "x2")
    transition = _finite_real(
        transition, "local_flatten", "invalid_transition", "transition"
    )
    strength = _finite_real(strength, "local_flatten", "invalid_strength", "strength")
    if transition < 0:
        raise ProcessingError("local_flatten", "invalid_transition", "transition must be non-negative")
    if not 0 <= strength <= 1:
        raise ProcessingError("local_flatten", "invalid_strength", "strength must be between 0 and 1")
    if x1 > x2:
        x1, x2 = x2, x1
    finite = np.isfinite(x_values) & np.isfinite(original)
    mask = finite & (x_values >= x1) & (x_values <= x2)
    if mask.sum() < 2:
        return _result(original, original.copy(), no_op="insufficient_points")
    if np.unique(x_values[mask]).size < 2:
        raise ProcessingError("local_flatten", "degenerate_x", "fit x values are identical")
    def calculate():
        slope, _intercept = np.polyfit(x_values[mask], original[mask], 1)
        anchor_x = {"left": x1, "right": x2, "center": x1 / 2.0 + x2 / 2.0}[anchor]
        correction = np.zeros_like(original)
        correction[mask] = strength * slope * (x_values[mask] - anchor_x)
        if transition > 0:
            left_edge = x1 - transition
            right_edge = x2 + transition
            if not np.isfinite(left_edge) or not np.isfinite(right_edge):
                raise FloatingPointError("transition bounds overflow")
            left = finite & (x_values >= left_edge) & (x_values < x1)
            right = finite & (x_values > x2) & (x_values <= right_edge)
            correction[left] = (
                0.5 * (1 - np.cos(np.pi * (x_values[left] - left_edge) / transition))
                * strength * slope * (x_values[left] - anchor_x)
            )
            correction[right] = (
                0.5 * (1 + np.cos(np.pi * (x_values[right] - x2) / transition))
                * strength * slope * (x_values[right] - anchor_x)
            )
        result = original.copy()
        result[finite] -= correction[finite]
        return float(slope), result

    slope, result = _guard_numeric("local_flatten", calculate)
    return _result(original, result, slope=float(slope), anchor=anchor)


def _window(length: int, requested: int, polyorder: int) -> int | None:
    if polyorder < 0 or requested < 1:
        raise ProcessingError("denoise", "invalid_window", "window and polyorder must be non-negative")
    if length < 3:
        return None
    candidate = requested + (requested % 2 == 0)
    candidate = min(candidate, length)
    if candidate % 2 == 0:
        candidate -= 1
    return candidate if candidate > polyorder else None


def denoise_values(y, window_length: int, polyorder: int, x=None, x1=None, x2=None) -> ProcessingResult:
    original = _values(y, "denoise")
    requested = _integer(window_length, "denoise", "invalid_window", "window_length")
    order = _integer(polyorder, "denoise", "invalid_window", "polyorder")
    if requested < 1 or requested % 2 == 0 or order < 0 or requested <= order:
        raise ProcessingError("denoise", "invalid_window", "window must be odd and exceed polyorder")
    eligible = np.isfinite(original)
    if x is not None or x1 is not None or x2 is not None:
        if x is None or x1 is None or x2 is None:
            raise ProcessingError("denoise", "incomplete_range", "x, x1, and x2 are required together")
        x_values = _values(x, "denoise")
        if len(x_values) != len(original):
            raise ProcessingError("denoise", "length_mismatch", "x and y lengths differ")
        x1 = _finite_real(x1, "denoise", "invalid_interval", "x1")
        x2 = _finite_real(x2, "denoise", "invalid_interval", "x2")
        if x1 > x2:
            raise ProcessingError("denoise", "invalid_interval", "x1 must not exceed x2")
        eligible &= np.isfinite(x_values) & (x_values >= x1) & (x_values <= x2)
    result = original.copy()
    indices = np.flatnonzero(eligible)
    if indices.size:
        for segment in np.split(indices, np.flatnonzero(np.diff(indices) != 1) + 1):
            window = _window(len(segment), requested, order)
            if window is not None:
                result[segment] = _guard_numeric(
                    "denoise", lambda: savgol_filter(original[segment], window, order)
                )
    if not np.array_equal(result[~eligible], original[~eligible], equal_nan=True):
        raise AssertionError("non-finite or out-of-range values changed")
    return _result(original, result, window_length=requested, polyorder=order)


def remove_polynomial_background(x, y, fit_min: float, fit_max: float, order: int) -> ProcessingResult:
    x_values = _values(x, "background")
    original = _values(y, "background")
    if len(x_values) != len(original):
        raise ProcessingError("background", "length_mismatch", "x and y lengths differ")
    fit_min = _finite_real(fit_min, "background", "invalid_interval", "fit_min")
    fit_max = _finite_real(fit_max, "background", "invalid_interval", "fit_max")
    if fit_min > fit_max:
        raise ProcessingError("background", "invalid_interval", "fit_min must not exceed fit_max")
    order = _integer(order, "background", "invalid_order", "order")
    if order < 0:
        raise ProcessingError("background", "invalid_order", "order must be a non-negative integer")
    finite = np.isfinite(x_values) & np.isfinite(original)
    fit = finite & (x_values >= fit_min) & (x_values <= fit_max)
    if fit.sum() <= order:
        raise ProcessingError("background", "insufficient_fit_points", "not enough finite fit points")
    if np.unique(x_values[fit]).size <= order:
        raise ProcessingError("background", "degenerate_x", "fit x values cannot determine polynomial")
    def calculate():
        coefficients = np.polyfit(x_values[fit], original[fit], order)
        result = original.copy()
        result[finite] -= np.polyval(coefficients, x_values[finite])
        return coefficients, result

    coefficients, result = _guard_numeric("background", calculate)
    return _result(original, result, coefficients=coefficients.tolist(), order=order)
