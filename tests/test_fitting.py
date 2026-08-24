import numpy as np
import pytest

from instplot_fitting import FitError, fit_values


@pytest.mark.parametrize(
    ("method", "x", "y", "kwargs"),
    [
        ("polynomial", np.linspace(-2, 2, 80), None, {"degree": 3}),
        ("exponential", np.linspace(0, 2, 80), None, {}),
        ("logarithmic", np.linspace(0.2, 4, 80), None, {}),
        ("power", np.linspace(0.2, 4, 80), None, {}),
        (
            "custom",
            np.linspace(-2, 2, 80),
            None,
            {"expression": "a * sin(b * x + c)", "initial_parameters": [2, 1, 0]},
        ),
    ],
)
def test_fit_matrix_roundtrips_known_curves(method, x, y, kwargs):
    if method == "polynomial":
        y = 0.5 * x**3 - 2 * x + 4
    elif method == "exponential":
        y = 1.5 * np.exp(0.7 * x)
    elif method == "logarithmic":
        y = 2.2 * np.log(x) - 0.3
    elif method == "power":
        y = 1.8 * x**1.4
    else:
        y = 2.0 * np.sin(1.0 * x + 0.2)

    original_x, original_y = x.copy(), y.copy()
    result = fit_values(x, y, method, **kwargs)

    assert result.r2 > 0.999999
    assert result.x_fit.shape == (500,)
    assert result.y_fit.shape == (500,)
    assert result.equation.startswith("y =")
    np.testing.assert_array_equal(x, original_x)
    np.testing.assert_array_equal(y, original_y)


@pytest.mark.parametrize(
    ("method", "x", "y", "code"),
    [
        ("logarithmic", [-1.0, 1.0], [1.0, 2.0], "invalid_domain"),
        ("power", [1.0, 2.0], [1.0, -2.0], "invalid_domain"),
        ("polynomial", [1.0], [2.0], "insufficient_points"),
        ("unknown", [1.0, 2.0], [2.0, 3.0], "invalid_method"),
    ],
)
def test_fit_errors_are_named(method, x, y, code):
    with pytest.raises(FitError) as raised:
        fit_values(x, y, method)
    assert raised.value.code == code


def test_custom_expression_rejects_unsafe_syntax():
    with pytest.raises(FitError) as raised:
        fit_values(
            [0.0, 1.0, 2.0],
            [0.0, 1.0, 2.0],
            "custom",
            expression="__import__('os').system('echo unsafe')",
            initial_parameters=[1.0],
        )
    assert raised.value.code == "invalid_expression"


def test_nonfinite_and_shape_mismatch_are_rejected():
    with pytest.raises(FitError, match="invalid_values"):
        fit_values([0.0, 1.0], [1.0, np.nan], "polynomial")
    with pytest.raises(FitError, match="shape_mismatch"):
        fit_values([0.0, 1.0], [1.0], "polynomial")


def test_cancellation_check_runs_before_and_after_solver():
    calls = []

    def check_cancel():
        calls.append(True)

    fit_values([0.0, 1.0, 2.0], [1.0, 3.0, 5.0], "polynomial", cancel_check=check_cancel)
    assert len(calls) >= 2
