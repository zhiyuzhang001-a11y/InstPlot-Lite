use super::{ProcessingError, ProcessingMetadata, ProcessingResult};

fn error(reason: &str) -> ProcessingError {
    ProcessingError::new("auto_linear_background", "uncertain_background", reason)
}

fn median(values: &[f64]) -> f64 {
    let mut v = values.to_vec();
    v.sort_unstable_by(f64::total_cmp);
    let n = v.len();
    v[(n - 1) / 2] / 2.0 + v[n / 2] / 2.0
}

// Weighted QR avoids normal equations for the trend/sinusoid joint fit.
fn fit(t: &[f64], y: &[f64], cycles: Option<f64>, robust: bool) -> Option<Vec<f64>> {
    let mut columns = vec![vec![1.0; t.len()], t.to_vec()];
    if let Some(c) = cycles {
        columns.push(
            t.iter()
                .map(|x| (std::f64::consts::TAU * c * x).sin())
                .collect(),
        );
        columns.push(
            t.iter()
                .map(|x| (std::f64::consts::TAU * c * x).cos())
                .collect(),
        );
    }
    let n = columns.len();
    let mut weights = vec![1.0_f64; t.len()];
    let mut coefficients = vec![0.0; n];
    for _ in 0..if robust { 7 } else { 1 } {
        let mut q: Vec<Vec<f64>> = Vec::new();
        let mut r = vec![vec![0.0; n]; n];
        for j in 0..n {
            let mut v: Vec<_> = columns[j]
                .iter()
                .zip(&weights)
                .map(|(v, w)| v * w.sqrt())
                .collect();
            for i in 0..j {
                r[i][j] = q[i].iter().zip(&v).map(|(a, b)| a * b).sum();
                for (v, q) in v.iter_mut().zip(&q[i]) {
                    *v -= r[i][j] * q;
                }
            }
            r[j][j] = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if r[j][j] < 1e-8 || !r[j][j].is_finite() {
                return None;
            }
            q.push(v.iter().map(|x| x / r[j][j]).collect());
        }
        let weighted: Vec<_> = y.iter().zip(&weights).map(|(v, w)| v * w.sqrt()).collect();
        let rhs: Vec<f64> = q
            .iter()
            .map(|v| v.iter().zip(&weighted).map(|(a, b)| a * b).sum())
            .collect();
        for i in (0..n).rev() {
            coefficients[i] =
                (rhs[i] - (i + 1..n).map(|j| r[i][j] * coefficients[j]).sum::<f64>()) / r[i][i];
        }
        if coefficients.iter().any(|v| !v.is_finite()) {
            return None;
        }
        if !robust {
            break;
        }
        let residual: Vec<_> = (0..y.len())
            .map(|i| y[i] - (0..n).map(|j| coefficients[j] * columns[j][i]).sum::<f64>())
            .collect();
        let center = median(&residual);
        let scale = 1.4826
            * median(
                &residual
                    .iter()
                    .map(|v| (v - center).abs())
                    .collect::<Vec<_>>(),
            );
        if scale < 1e-12 {
            break;
        }
        for (w, v) in weights.iter_mut().zip(residual) {
            *w = (1.5 * scale / (v - center).abs()).min(1.0);
        }
    }
    Some(coefficients)
}

fn loss(t: &[f64], y: &[f64], c: &[f64], cycles: Option<f64>) -> f64 {
    let mut squares: Vec<_> = t
        .iter()
        .zip(y)
        .map(|(t, y)| {
            let periodic = cycles.map_or(0.0, |f| {
                c[2] * (std::f64::consts::TAU * f * t).sin()
                    + c[3] * (std::f64::consts::TAU * f * t).cos()
            });
            (y - c[0] - c[1] * t - periodic).powi(2)
        })
        .collect();
    squares.sort_unstable_by(f64::total_cmp);
    let count = (squares.len() * 9 / 10).max(1);
    squares[..count].iter().sum::<f64>() / count as f64
}

/// Estimates a periodic signal and linear drift together. Only the drift is
/// removed. This is a model-based preview, not identification of physical background.
/// Ambiguous/non-periodic data fail closed rather than flattening the signal.
pub fn remove_auto_linear_background(
    x: &[f64],
    y: &[f64],
    alive: &[bool],
    zero: bool,
) -> Result<ProcessingResult, ProcessingError> {
    if x.len() != y.len() || (!alive.is_empty() && alive.len() != x.len()) {
        return Err(error("数据长度不一致"));
    }
    let mut pairs: Vec<_> = x
        .iter()
        .zip(y)
        .enumerate()
        .filter(|(i, (x, y))| (alive.is_empty() || alive[*i]) && x.is_finite() && y.is_finite())
        .map(|(_, (&x, &y))| (x, y))
        .collect();
    if pairs.len() < 24 {
        return Err(error("有效点不足，无法可靠识别周期背景；未修改数据"));
    }
    pairs.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let min = pairs[0].0;
    let span = pairs[pairs.len() - 1].0 - min;
    if !span.is_finite() || span <= 0.0 {
        return Err(error("X 范围无效；未修改数据"));
    }
    let points = pairs.len();
    let sampled: Vec<_> = pairs.iter().step_by(pairs.len().div_ceil(1500)).collect();
    let origin = median(&sampled.iter().map(|p| p.1).collect::<Vec<_>>());
    let scale = sampled
        .iter()
        .map(|p| (p.1 - origin).abs())
        .fold(0.0_f64, f64::max);
    if !scale.is_finite() || scale <= f64::MIN_POSITIVE {
        return Err(error("没有可识别的周期信号；建议保持原数据"));
    }
    let t: Vec<_> = sampled.iter().map(|p| (p.0 - min) / span - 0.5).collect();
    let ys: Vec<_> = sampled.iter().map(|p| (p.1 - origin) / scale).collect();
    let linear = fit(&t, &ys, None, true).ok_or_else(|| error("拟合不稳定；未修改数据"))?;
    let linear_loss = loss(&t, &ys, &linear, None);
    let upper = 6.0_f64.min(sampled.len() as f64 / 12.0);
    let mut best = (f64::INFINITY, 0.0);
    for i in 0..=90 {
        let cycles = 1.5 + i as f64 * 0.05;
        if cycles > upper {
            break;
        }
        if let Some(c) = fit(&t, &ys, Some(cycles), false) {
            let l = loss(&t, &ys, &c, Some(cycles));
            if l < best.0 {
                best = (l, cycles);
            }
        }
    }
    let coarse = best.1;
    for i in -20..=20 {
        let cycles = coarse + i as f64 * 0.0025;
        if cycles < 1.5 || cycles > upper {
            continue;
        }
        if let Some(c) = fit(&t, &ys, Some(cycles), true) {
            let l = loss(&t, &ys, &c, Some(cycles));
            if l < best.0 {
                best = (l, cycles);
            }
        }
    }
    let c = fit(&t, &ys, Some(best.1), true).ok_or_else(|| error("周期识别失败；未修改数据"))?;
    let residual = loss(&t, &ys, &c, Some(best.1));
    if best.1 <= 1.5025
        || best.1 >= upper - 0.0025
        || residual >= linear_loss * 0.35
        || c[2].hypot(c[3]) <= 3.0 * residual.sqrt()
        || c[2].hypot(c[3]) < 1e-3
    {
        return Err(error(
            "未发现足够可靠的周期特征，不能自动区分真实信号和线性背景；未修改数据",
        ));
    }
    let slope = c[1] * scale / span;
    let baseline = origin + scale * (c[0] - c[1] * 0.5);
    let values: Vec<_> = x
        .iter()
        .zip(y)
        .map(|(&x, &y)| {
            if x.is_finite() && y.is_finite() {
                y - slope * (x - min) - if zero { baseline } else { 0.0 }
            } else {
                y
            }
        })
        .collect();
    if !slope.is_finite()
        || !baseline.is_finite()
        || values
            .iter()
            .zip(y)
            .any(|(a, b)| b.is_finite() && !a.is_finite())
    {
        return Err(error("结果超出数值范围；未修改数据"));
    }
    Ok(ProcessingResult {
        values,
        metadata: ProcessingMetadata::AutoLinearBackground {
            slope,
            anchor_x: min,
            baseline,
            period: span / best.1,
            noise: residual.sqrt() * scale,
            span,
            points,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_periodic_signal_and_removes_only_drift_with_outliers_and_mask() {
        let x: Vec<_> = (0..721).map(|i| i as f64 / 2.0).collect();
        let signal = |x: f64| 7.0 + 2.0 * (std::f64::consts::TAU * x / 180.0 + 0.7).cos();
        let mut y: Vec<_> = x.iter().map(|&x| signal(x) + 0.008 * x).collect();
        y[70] += 50.0;
        y[100] = 10000.0;
        let mut alive = vec![true; x.len()];
        alive[100] = false;
        let result = remove_auto_linear_background(&x, &y, &alive, false).unwrap();
        let ProcessingMetadata::AutoLinearBackground { slope, period, .. } = result.metadata else {
            panic!()
        };
        assert!((slope - 0.008).abs() < 0.0001, "{slope}");
        assert!((period - 180.0).abs() < 0.1, "{period}");
        for i in [0, 180, 360, 540, 720] {
            assert!((result.values[i] - signal(x[i])).abs() < 0.04);
        }
        assert_eq!(y[70], signal(x[70]) + 0.008 * x[70] + 50.0);
    }
    #[test]
    fn units_are_not_assumed_and_nonperiodic_data_are_rejected() {
        let x: Vec<_> = (0..400)
            .map(|i| (i as f64 / 399.0).powf(1.1) * 20.0)
            .collect();
        let y: Vec<_> = x
            .iter()
            .map(|x| 4.0 + 0.1 * x + (std::f64::consts::TAU * x / 8.0).sin())
            .collect();
        let r = remove_auto_linear_background(&x, &y, &[], true).unwrap();
        let ProcessingMetadata::AutoLinearBackground { period, slope, .. } = r.metadata else {
            panic!()
        };
        assert!((period - 8.0).abs() < 0.03);
        assert!((slope - 0.1).abs() < 0.005);
        let y: Vec<_> = x.iter().map(|x| 3.0 * x).collect();
        assert!(remove_auto_linear_background(&x, &y, &[], false).is_err());
        assert!(remove_auto_linear_background(&x, &vec![1.0; x.len()], &[], false).is_err());
        assert!(remove_auto_linear_background(&[1.0; 30], &[2.0; 30], &[], false).is_err());
        assert!(remove_auto_linear_background(&x, &y, &[true], false).is_err());
    }
}
