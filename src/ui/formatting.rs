#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisDisplay {
    offset: f64,
    exponent: i32,
}

impl AxisDisplay {
    pub(crate) fn from_range(range: Option<[f64; 2]>) -> Self {
        let Some([minimum, maximum]) = range else {
            return Self {
                offset: 0.0,
                exponent: 0,
            };
        };
        if !minimum.is_finite() || !maximum.is_finite() {
            return Self {
                offset: 0.0,
                exponent: 0,
            };
        }

        let span = (maximum - minimum).abs();
        let center = minimum + (maximum - minimum) * 0.5;
        let offset = if span > 0.0 && center.abs() >= 10_000.0 && center.abs() / span >= 10_000.0 {
            // Keep the reference coarser than the visible variation so that
            // floating-point noise around exact powers of ten cannot make the
            // displayed baseline jump by one tick.
            let quantum = 10.0_f64.powf(span.log10().ceil() + 1.0);
            if quantum.is_finite() && quantum > 0.0 {
                (center / quantum).round() * quantum
            } else {
                0.0
            }
        } else {
            0.0
        };

        let magnitude = if offset == 0.0 {
            minimum.abs().max(maximum.abs())
        } else {
            (minimum - offset).abs().max((maximum - offset).abs())
        };
        let engineering_exponent = if magnitude > 0.0 {
            (magnitude.log10().floor() as i32).div_euclid(3) * 3
        } else {
            0
        };
        // Keep ordinary labels to at most five integer digits or four
        // fractional leading places. Beyond that, a shared engineering scale
        // is easier to scan and avoids wide tick labels.
        let exponent = if (offset != 0.0 && engineering_exponent != 0)
            || magnitude >= 100_000.0
            || (magnitude > 0.0 && magnitude < 0.001)
        {
            engineering_exponent
        } else {
            0
        };

        Self { offset, exponent }
    }

    fn scale(self) -> f64 {
        10.0_f64.powi(self.exponent)
    }

    pub(crate) fn format_tick(self, value: f64, step_size: f64) -> String {
        let scale = self.scale();
        let displayed = (value - self.offset) / scale;
        let displayed_step = step_size.abs() / scale.abs();
        format_axis_decimal(displayed, displayed_step)
    }

    pub(crate) fn label(self, name: &str) -> String {
        let mut notes = Vec::with_capacity(2);
        if self.offset != 0.0 {
            notes.push(format!("基准 {}", format_axis_reference(self.offset)));
        }
        if self.exponent != 0 {
            notes.push(format!("×10^({})", self.exponent));
        }
        if notes.is_empty() {
            name.to_owned()
        } else {
            format!("{name}（{}）", notes.join("；"))
        }
    }
}

pub(crate) fn format_axis_decimal(value: f64, step_size: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let zero_threshold = if step_size.is_finite() && step_size > 0.0 {
        step_size * 1.0e-9
    } else {
        f64::EPSILON
    };
    let value = if value.abs() <= zero_threshold {
        0.0
    } else {
        value
    };
    let decimals = decimals_for_step(step_size);
    let formatted = format!("{value:.decimals$}");
    let trimmed = if formatted.contains('.') {
        formatted.trim_end_matches('0').trim_end_matches('.')
    } else {
        &formatted
    };
    if trimmed == "-0" || trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn decimals_for_step(step_size: f64) -> usize {
    if !step_size.is_finite() || step_size <= 0.0 {
        return 3;
    }
    for decimals in 0..=8 {
        let scaled = step_size * 10.0_f64.powi(decimals as i32);
        if (scaled - scaled.round()).abs() <= scaled.abs().max(1.0) * 1.0e-9 {
            return decimals;
        }
    }
    ((-step_size.log10()).ceil() as isize + 2).clamp(0, 8) as usize
}

fn format_axis_reference(value: f64) -> String {
    let scientific = format!("{value:.12e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    format!("{mantissa}×10^({exponent})")
}

pub(crate) fn legend_series_name(name: &str, is_active: bool) -> String {
    let name = compact_label(name, 38);
    if is_active {
        format!("▶ {name}")
    } else {
        name
    }
}

pub(crate) fn compact_label(value: &str, maximum_chars: usize) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= maximum_chars || maximum_chars < 5 {
        return value.to_owned();
    }
    let suffix_count = maximum_chars / 3;
    let prefix_count = maximum_chars - suffix_count - 1;
    let prefix = characters[..prefix_count].iter().collect::<String>();
    let suffix = characters[characters.len() - suffix_count..]
        .iter()
        .collect::<String>();
    format!("{prefix}…{suffix}")
}

pub(crate) fn split_fit_display_equation(equation: &str) -> (&str, Option<&str>) {
    equation.find("  (").map_or((equation, None), |index| {
        let formula = &equation[..index];
        let parameters = equation[index + 2..].trim();
        (formula, (!parameters.is_empty()).then_some(parameters))
    })
}

#[cfg(test)]
mod tests {
    use super::{AxisDisplay, compact_label, format_axis_decimal, legend_series_name};

    #[test]
    fn custom_fit_display_separates_formula_from_parameters() {
        assert_eq!(
            super::split_fit_display_equation("y = a * cos(x) + b  (a=7.01e-7, b=9.69e-8)"),
            ("y = a * cos(x) + b", Some("(a=7.01e-7, b=9.69e-8)"))
        );
        assert_eq!(
            super::split_fit_display_equation("y = 2×x + 1"),
            ("y = 2×x + 1", None)
        );
    }

    #[test]
    fn active_curve_is_explicitly_marked_in_the_legend() {
        assert_eq!(legend_series_name("sample.csv", true), "▶ sample.csv");
        assert_eq!(legend_series_name("other.csv", false), "other.csv");
    }

    #[test]
    fn long_dataset_labels_keep_their_start_and_distinguishing_suffix() {
        assert_eq!(
            compact_label("abcdefghijklmnopqrstuvwxyz", 10),
            "abcdef…xyz"
        );
        assert_eq!(compact_label("short", 10), "short");
    }

    #[test]
    fn tiny_axis_ranges_use_a_shared_engineering_scale() {
        let display = AxisDisplay::from_range(Some([-0.000012, 0.0]));
        assert_eq!(display.exponent, -6);
        assert_eq!(display.offset, 0.0);
        assert_eq!(display.label("2χ"), "2χ（×10^(-6)）");
        assert_eq!(display.format_tick(-0.000001, 0.000001), "-1");
        assert_eq!(display.format_tick(0.0, 0.000001), "0");
    }

    #[test]
    fn five_decimal_axis_values_use_a_shared_engineering_scale() {
        let display = AxisDisplay::from_range(Some([0.00001, 0.00015]));
        assert_eq!(display.exponent, -6);
        assert_eq!(display.label("1-X"), "1-X（×10^(-6)）");
        assert_eq!(display.format_tick(0.00012, 0.00001), "120");
    }

    #[test]
    fn six_digit_axis_values_use_a_shared_engineering_scale() {
        let display = AxisDisplay::from_range(Some([100_000.0, 150_000.0]));
        assert_eq!(display.exponent, 3);
        assert_eq!(display.label("signal"), "signal（×10^(3)）");
        assert_eq!(display.format_tick(120_000.0, 10_000.0), "120");
    }

    #[test]
    fn readable_axis_values_stay_in_ordinary_notation() {
        assert_eq!(AxisDisplay::from_range(Some([0.001, 0.009])).exponent, 0);
        assert_eq!(
            AxisDisplay::from_range(Some([10_000.0, 99_999.0])).exponent,
            0
        );
    }

    #[test]
    fn large_baselines_are_shown_as_small_relative_changes() {
        let display = AxisDisplay::from_range(Some([10_000_000.001, 10_000_000.011]));
        assert!((display.offset - 10_000_000.0).abs() < 1.0e-9);
        assert_eq!(display.exponent, -3);
        assert_eq!(display.label("signal"), "signal（基准 1×10^(7)；×10^(-3)）");
        assert_eq!(display.format_tick(10_000_000.001, 0.001), "1");
    }

    #[test]
    fn ordinary_axis_ticks_keep_required_precision_without_trailing_zeroes() {
        assert_eq!(format_axis_decimal(10.0, 1.0), "10");
        assert_eq!(format_axis_decimal(1.5, 0.25), "1.5");
        assert_eq!(format_axis_decimal(-1.0e-15, 0.1), "0");
    }
}
