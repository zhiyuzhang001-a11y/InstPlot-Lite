import numpy as np
import pandas as pd
import pytest
import warnings
from PySide6.QtWidgets import QDialog, QPushButton, QTableWidget, QTableWidgetItem

import InstPlot
from InstPlot import center_data, denoise_data, local_flatten_keep_anchor, normalize_data
from instplot_processing import (
    ProcessingError,
    center_values,
    denoise_values,
    local_flatten_values,
    normalize_values,
    remove_polynomial_background,
)


def test_processing_core_has_no_gui_or_plot_imports():
    source = __import__("inspect").getsource(__import__("instplot_processing"))
    assert "PySide6" not in source
    assert "matplotlib" not in source
    assert "InstPlot" not in source


def test_core_center_preserves_nonfinite_positions_and_input():
    values = np.array([np.nan, -2.0, 2.0, np.inf, 6.0])
    original = values.copy()
    result = center_values(values)
    np.testing.assert_allclose(result.values, [np.nan, -4.0, 0.0, np.inf, 4.0], equal_nan=True)
    np.testing.assert_allclose(values, original, equal_nan=True)


def test_core_normalize_preserves_legacy_finite_and_nonfinite_values():
    finite = normalize_values([-3.0, -2.0, -1.0], top_n=2)
    mixed = normalize_values([1.0, np.nan, np.inf, 3.0], top_n=2)
    np.testing.assert_allclose(finite.values, [-1.0, -1.0, 1.0])
    assert finite.metadata["scale"] == pytest.approx(-1.5)
    np.testing.assert_allclose(mixed.values, [0.5, np.nan, np.inf, 1.0], equal_nan=True)


def test_core_denoise_processes_finite_segments_without_spreading_nan_inf():
    result = denoise_values([0.0, 1.0, 2.0, np.nan, 4.0, 5.0, 6.0, np.inf], 3, 1)
    np.testing.assert_allclose(
        result.values,
        [0.0, 1.0, 2.0, np.nan, 4.0, 5.0, 6.0, np.inf],
        equal_nan=True,
        atol=1e-12,
    )


def test_core_local_and_background_validate_degenerate_inputs():
    with pytest.raises(ProcessingError) as repeated:
        local_flatten_values([1.0, 1.0], [1.0, 2.0], 1.0, 1.0)
    assert repeated.value.code == "degenerate_x"
    with pytest.raises(ProcessingError) as insufficient:
        remove_polynomial_background([0.0, 1.0], [1.0, 2.0], 0.0, 0.0, 1)
    assert insufficient.value.code == "insufficient_fit_points"


@pytest.mark.parametrize(
    ("call", "code"),
    [
        (lambda: center_values([]), "empty_values"),
        (lambda: normalize_values([1.0], top_n=0), "invalid_top_n"),
        (lambda: local_flatten_values([0.0], [1.0, 2.0], 0.0, 1.0), "length_mismatch"),
        (lambda: denoise_values([1.0, 2.0], 3, 3), "invalid_window"),
        (lambda: remove_polynomial_background([0.0], [1.0], 1.0, 0.0, 1), "invalid_interval"),
    ],
)
def test_core_invalid_inputs_have_stable_codes(call, code):
    with pytest.raises(ProcessingError) as raised:
        call()
    assert raised.value.code == code


@pytest.mark.parametrize(
    "call",
    [
        lambda: center_values(np.ones((2, 2))),
        lambda: normalize_values(np.ones((2, 2))),
        lambda: local_flatten_values(np.ones((2, 2)), [1.0, 2.0], 0.0, 1.0),
        lambda: local_flatten_values([0.0, 1.0], np.ones((2, 2)), 0.0, 1.0),
        lambda: denoise_values(np.ones((2, 2)), 3, 1),
        lambda: denoise_values([1.0, 2.0], 3, 1, x=np.ones((2, 2)), x1=0.0, x2=1.0),
        lambda: remove_polynomial_background(np.ones((2, 2)), [1.0, 2.0], 0.0, 1.0, 1),
        lambda: remove_polynomial_background([0.0, 1.0], np.ones((2, 2)), 0.0, 1.0, 1),
    ],
)
def test_core_rejects_multidimensional_inputs_with_stable_error(call):
    with pytest.raises(ProcessingError) as raised:
        call()
    assert raised.value.code == "invalid_dimensions"


@pytest.mark.parametrize(
    "values",
    [
        [[1.0], [2.0, 3.0]],
        (np.array([1.0]), np.array([2.0, 3.0])),
        [1.0, np.array([2.0, 3.0])],
        np.array([[1.0], [2.0, 3.0]], dtype=object),
    ],
)
def test_core_ragged_inputs_have_stable_dimension_error(values):
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        with pytest.raises(ProcessingError) as raised:
            center_values(values)
    assert raised.value.code == "invalid_dimensions"


@pytest.mark.parametrize(
    "values",
    [
        [1.0 + 2.0j, 3.0 + 4.0j],
        np.array([1.0 + 0.0j, 2.0 + 0.0j]),
        np.array([1.0, np.complex128(2.0 + 3.0j)], dtype=object),
        pd.Series([1.0 + 2.0j, 3.0 + 4.0j], dtype="complex128"),
    ],
)
def test_core_complex_arrays_are_rejected_without_discarding_imaginary_values(values):
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        with pytest.raises(ProcessingError) as raised:
            normalize_values(values)
    assert raised.value.code == "invalid_values"


@pytest.mark.parametrize(
    ("values", "expected"),
    [
        ([1.0, 2.0, 3.0], [-1.0, 0.0, 1.0]),
        ((1.0, 2.0, 3.0), [-1.0, 0.0, 1.0]),
        (np.array([1, 2, 3], dtype=np.int64), [-1.0, 0.0, 1.0]),
        (pd.Series([1.0, pd.NA, 3.0], dtype="Float64"), [-1.0, np.nan, 1.0]),
        (pd.Series([1, pd.NA, 3], dtype="Int64"), [-1.0, np.nan, 1.0]),
        (pd.Series(["1", "bad", "3"], dtype="string"), [-1.0, np.nan, 1.0]),
        (np.array(["1", "bad", 3], dtype=object), [-1.0, np.nan, 1.0]),
    ],
)
def test_core_accepts_supported_real_and_coercible_one_dimensional_inputs(values, expected):
    original = values.copy() if hasattr(values, "copy") else tuple(values)

    result = center_values(values)

    np.testing.assert_allclose(result.values, expected, equal_nan=True)
    if isinstance(values, np.ndarray):
        np.testing.assert_array_equal(values, original)
    elif isinstance(values, pd.Series):
        pd.testing.assert_series_equal(values, original)
    else:
        assert values == original


@pytest.mark.parametrize(
    ("call", "code"),
    [
        (lambda: center_values([[1.0], [2.0, 3.0]]), "invalid_dimensions"),
        (lambda: normalize_values(np.array([1.0 + 2.0j, 3.0 + 4.0j])), "invalid_values"),
        (lambda: local_flatten_values([[0.0], [1.0, 2.0]], [1.0, 2.0], 0.0, 1.0), "invalid_dimensions"),
        (lambda: local_flatten_values([0.0, 1.0], np.array([1.0 + 2.0j, 2.0]), 0.0, 1.0), "invalid_values"),
        (lambda: denoise_values([[1.0], [2.0, 3.0]], 3, 1), "invalid_dimensions"),
        (lambda: denoise_values([1.0, 2.0, 3.0], 3, 1, x=np.array([0.0, 1.0 + 2.0j, 2.0]), x1=0.0, x2=2.0), "invalid_values"),
        (lambda: remove_polynomial_background(np.array([0.0, 1.0 + 2.0j]), [1.0, 2.0], 0.0, 1.0, 0), "invalid_values"),
        (lambda: remove_polynomial_background([0.0, 1.0], [[1.0], [2.0, 3.0]], 0.0, 1.0, 0), "invalid_dimensions"),
    ],
)
def test_shared_conversion_contract_covers_every_public_value_position(call, code):
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        with pytest.raises(ProcessingError) as raised:
            call()
    assert raised.value.code == code


@pytest.mark.parametrize(
    ("window_length", "polyorder"),
    [
        (4, 2),
        (3.9, 2),
        ("3", 1),
        (True, 0),
        (np.nan, 1),
        (np.inf, 1),
        (3, 1.5),
        (3, "1"),
        (3, False),
        (3, np.nan),
        (3, np.inf),
    ],
)
def test_core_denoise_rejects_non_integer_or_even_parameters(window_length, polyorder):
    with pytest.raises(ProcessingError) as raised:
        denoise_values(np.arange(7.0), window_length, polyorder)
    assert raised.value.code == "invalid_window"


@pytest.mark.parametrize(
    ("call", "code"),
    [
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], np.nan, 1.0), "invalid_interval"),
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], 0.0, np.inf), "invalid_interval"),
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], "bad", 1.0), "invalid_interval"),
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], np.complex128(1.0 + 2.0j), 1.0), "invalid_interval"),
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], 0.0, 1.0, transition=np.nan), "invalid_transition"),
        (lambda: local_flatten_values([0.0, 1.0], [1.0, 2.0], 0.0, 1.0, strength=np.inf), "invalid_strength"),
        (lambda: denoise_values([0.0, 1.0, 2.0], 3, 1, x=[0.0, 1.0, 2.0], x1=np.nan, x2=2.0), "invalid_interval"),
        (lambda: denoise_values([0.0, 1.0, 2.0], 3, 1, x=[0.0, 1.0, 2.0], x1=0.0, x2=np.inf), "invalid_interval"),
        (lambda: denoise_values([0.0, 1.0, 2.0], 3, 1, x=[0.0, 1.0, 2.0], x1=0.0, x2=1.0 + 2.0j), "invalid_interval"),
        (lambda: remove_polynomial_background([0.0, 1.0], [1.0, 2.0], np.nan, 1.0, 0), "invalid_interval"),
        (lambda: remove_polynomial_background([0.0, 1.0], [1.0, 2.0], 0.0, "bad", 0), "invalid_interval"),
        (lambda: remove_polynomial_background([0.0, 1.0], [1.0, 2.0], 0.0, 1.0 + 2.0j, 0), "invalid_interval"),
    ],
)
def test_core_rejects_nonfinite_or_nonnumeric_scalar_parameters(call, code):
    with pytest.raises(ProcessingError) as raised:
        call()
    assert raised.value.code == code


def test_core_extreme_finite_center_and_normalize_are_warning_free():
    with np.errstate(all="raise"):
        centered = center_values([1e308, 1e308])
        normalized = normalize_values([-1e308, -5e-324], top_n=1)

    np.testing.assert_allclose(centered.values, [0.0, 0.0])
    assert centered.metadata["center"] == 1e308
    np.testing.assert_allclose(normalized.values, [-1.0, -1.0])
    assert normalized.metadata["scale"] == -5e-324


@pytest.mark.parametrize(
    "call",
    [
        lambda: local_flatten_values(
            [1e308, 1e308 - 1e292, 1e308 - 2e292], [1.0, 2.0, 3.0], 0.0, 1e308
        ),
        lambda: remove_polynomial_background(
            [1e308, 1e308 - 1e292, 1e308 - 2e292], [1.0, 2.0, 3.0], 0.0, 1e308, 1
        ),
    ],
)
def test_core_extreme_fits_return_named_error_instead_of_warning(call):
    with np.errstate(all="raise"):
        with pytest.raises(ProcessingError) as raised:
            call()
    assert raised.value.code == "numeric_failure"


def test_center_preserves_input_and_uses_nan_aware_midrange():
    values = np.array([np.nan, -2.0, 2.0, 6.0])
    original = values.copy()

    result = center_data(values)

    np.testing.assert_allclose(result, [np.nan, -4.0, 0.0, 4.0], equal_nan=True)
    np.testing.assert_allclose(values, original, equal_nan=True)


def test_center_empty_input_has_stable_processing_error():
    with pytest.raises(ProcessingError) as raised:
        center_data([])
    assert raised.value.code == "empty_values"


def test_center_all_nan_errors_and_infinity_keeps_its_position_without_warning():
    with pytest.raises(ProcessingError) as raised:
        center_data([np.nan, np.nan])
    with_infinity = center_data([1.0, np.inf])

    assert raised.value.code == "no_finite_values"
    np.testing.assert_allclose(with_infinity, [0.0, np.inf], equal_nan=True)


def test_normalize_freezes_current_top_average_and_clamping():
    values = pd.Series([-4.0, -2.0, 0.0, 2.0, 8.0, np.nan])

    result, scale = normalize_data(values, top_n=2)

    assert scale == pytest.approx(5.0)
    np.testing.assert_allclose(result, [-0.8, -0.4, 0.0, 0.4, 1.0, np.nan], equal_nan=True)


def test_normalize_all_negative_freezes_legacy_signed_scale_behavior():
    result, scale = normalize_data([-3.0, -2.0, -1.0], top_n=2)

    assert scale == pytest.approx(-1.5)
    np.testing.assert_allclose(result, [-1.0, -1.0, 1.0])


@pytest.mark.parametrize("values", [[], [np.nan, np.nan]])
def test_normalize_empty_or_all_nan_returns_nan_scale(values):
    result, scale = normalize_data(values)

    assert np.isnan(scale)
    np.testing.assert_allclose(result, values, equal_nan=True)


def test_normalize_zero_and_infinite_scales_freeze_legacy_results():
    zero_result, zero_scale = normalize_data([0.0, 0.0])
    infinite_result, infinite_scale = normalize_data([1.0, np.inf], top_n=1)

    assert zero_scale == 0.0
    np.testing.assert_allclose(zero_result, [0.0, 0.0])
    assert infinite_scale == 1.0
    np.testing.assert_allclose(infinite_result, [1.0, np.inf], equal_nan=True)


def test_local_flatten_preserves_anchor_and_input_arrays():
    x = np.arange(5.0)
    y = 2.0 * x + 5.0
    original = y.copy()

    result = local_flatten_keep_anchor(x, y, 1.0, 3.0, anchor="left")

    np.testing.assert_allclose(result, [5.0, 7.0, 7.0, 7.0, 13.0])
    np.testing.assert_allclose(y, original)


def test_local_flatten_short_selection_is_noop_and_invalid_anchor_is_named():
    x = np.arange(4.0)
    y = np.array([1.0, 3.0, 5.0, 7.0])

    np.testing.assert_allclose(local_flatten_keep_anchor(x, y, 1.0, 1.0), y)
    with pytest.raises(ValueError, match="anchor must be"):
        local_flatten_keep_anchor(x, y, 0.0, 3.0, anchor="invalid")


def test_local_flatten_unsorted_and_repeated_x_freeze_legacy_behavior():
    unsorted = local_flatten_keep_anchor([2.0, 0.0, 1.0], [5.0, 1.0, 3.0], 0.0, 2.0)
    with pytest.raises(ProcessingError) as repeated_error:
        repeated = local_flatten_keep_anchor([1.0, 1.0, 1.0], [1.0, 2.0, 3.0], 1.0, 1.0)

    np.testing.assert_allclose(unsorted, [1.0, 1.0, 1.0])
    assert repeated_error.value.code == "degenerate_x"


@pytest.mark.parametrize(
    ("values", "window_length", "polyorder"),
    [([1.0], 11, 3), ([1.0, 2.0, 3.0], 3, 3)],
)
def test_denoise_short_or_invalid_window_returns_original_values(
    values, window_length, polyorder
):
    frame = pd.DataFrame({"signal": values})

    result = denoise_data(
        frame, y_col="signal", window_length=window_length, polyorder=polyorder
    )

    np.testing.assert_allclose(result, values)


def test_denoise_preserves_a_linear_signal_with_valid_parameters():
    frame = pd.DataFrame({"signal": [0.0, 1.0, 2.0, 3.0, 4.0]})

    result = denoise_data(frame, y_col="signal", window_length=5, polyorder=2)

    np.testing.assert_allclose(result, frame["signal"], atol=1e-12)


def test_legacy_denoise_wrapper_keeps_even_window_adjustment():
    frame = pd.DataFrame({"signal": [0.0, 1.0, 5.0, 3.0, 4.0, 5.0, 6.0]})

    result = denoise_data(frame, y_col="signal", window_length=4, polyorder=2)
    expected = denoise_values(frame["signal"], window_length=5, polyorder=2).values

    np.testing.assert_allclose(result, expected)


def test_denoise_range_only_changes_selected_rows_and_preserves_frame():
    frame = pd.DataFrame(
        {"x": np.arange(7.0), "signal": [0.0, 1.0, 5.0, 3.0, 4.0, 5.0, 6.0]}
    )
    original = frame.copy(deep=True)

    result = denoise_data(
        frame,
        y_col="signal",
        window_length=5,
        polyorder=2,
        x_col="x",
        x1=1.0,
        x2=5.0,
    )

    assert result.iloc[0] == 0.0
    assert result.iloc[-1] == 6.0
    assert result.iloc[2] != 5.0
    pd.testing.assert_frame_equal(frame, original)


def test_denoise_nonfinite_values_preserve_positions_without_window_spread():
    nan_result = denoise_data(
        pd.DataFrame({"signal": [0.0, 1.0, np.nan, 3.0, 4.0]}),
        y_col="signal",
        window_length=5,
        polyorder=2,
    )
    inf_result = denoise_data(
        pd.DataFrame({"signal": [0.0, 1.0, np.inf, 3.0, 4.0]}),
        y_col="signal",
        window_length=5,
        polyorder=2,
    )

    np.testing.assert_allclose(nan_result, [0.0, 1.0, np.nan, 3.0, 4.0], equal_nan=True)
    np.testing.assert_allclose(inf_result, [0.0, 1.0, np.inf, 3.0, 4.0], equal_nan=True)


def _background_window(qapp):
    window = InstPlot.PlotApp()
    window.loaded_files.clear()
    window.history.reset(window.loaded_files)
    window.combo_x.clear()
    window.combo_y.clear()
    window.replot_all = lambda *args, **kwargs: None
    return window


def test_background_linear_fit_freezes_current_gui_result(qapp, monkeypatch):
    window = _background_window(qapp)
    frame = pd.DataFrame({"x": np.arange(5.0), "y": 2.0 * np.arange(5.0) + 5.0})
    window.loaded_files = [("linear.txt", frame)]
    window.history.reset(window.loaded_files)
    window.combo_x.addItems(["x", "y"])
    window.combo_y.addItems(["x", "y"])
    window.combo_x.setCurrentText("x")
    window.combo_y.setCurrentText("y")

    def accept_with_full_range(dialog):
        table = dialog.findChild(QTableWidget)
        assert table is not None
        table.setItem(0, 1, QTableWidgetItem("0"))
        table.setItem(0, 2, QTableWidgetItem("4"))
        button = next(item for item in dialog.findChildren(QPushButton) if item.text() == "确定")
        button.click()
        return QDialog.Accepted

    monkeypatch.setattr(InstPlot.QDialog, "exec", accept_with_full_range)
    try:
        window.remove_background()

        np.testing.assert_allclose(window.loaded_files[0][1]["y"], np.zeros(5), atol=1e-12)
        np.testing.assert_allclose(frame["y"], [5.0, 7.0, 9.0, 11.0, 13.0])
        assert window.history.undo_count == 1
    finally:
        window.close()


def test_background_cancel_keeps_data_and_does_not_record_history(qapp, monkeypatch):
    window = _background_window(qapp)
    frame = pd.DataFrame({"x": [0.0, 1.0], "y": [2.0, 3.0]})
    original = frame.copy(deep=True)
    window.loaded_files = [("cancel.txt", frame)]
    window.history.reset(window.loaded_files)
    window.combo_x.addItems(["x", "y"])
    window.combo_y.addItems(["x", "y"])
    window.combo_x.setCurrentText("x")
    window.combo_y.setCurrentText("y")
    monkeypatch.setattr(InstPlot.QDialog, "exec", lambda _dialog: QDialog.Rejected)
    try:
        window.remove_background()

        pd.testing.assert_frame_equal(frame, original)
        assert window.history.undo_count == 0
    finally:
        window.close()
