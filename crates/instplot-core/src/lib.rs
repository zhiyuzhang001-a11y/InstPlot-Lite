use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct NumericColumn {
    pub name: String,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FitLink {
    pub parent_dataset_id: Option<String>,
    pub source_x_column: String,
    pub source_y_column: String,
    pub equation: Option<String>,
    pub display_equation: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataSetKind {
    #[default]
    Source,
    Fit,
}

impl DataSetKind {
    pub fn metadata_value(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Fit => "fit",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataSet {
    pub source: PathBuf,
    pub label: Option<String>,
    pub kind: DataSetKind,
    pub plot_id: String,
    pub fit_link: Option<FitLink>,
    pub encoding: String,
    pub separator: String,
    pub columns: Vec<NumericColumn>,
    pub row_count: usize,
    pub alive: Vec<bool>,
}

pub fn generated_plot_id(path: &Path, columns: &[NumericColumn]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;
    let mut hash = OFFSET_BASIS;
    let mut update = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    update(path.to_string_lossy().as_bytes());
    for column in columns {
        update(column.name.as_bytes());
        for value in &column.values {
            update(&value.to_bits().to_le_bytes());
        }
    }
    format!("instplot-{hash:016x}")
}

impl DataSet {
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            return label.clone();
        }
        self.source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未命名数据")
            .to_owned()
    }

    pub fn plot_points(
        &self,
        x_column: usize,
        y_column: usize,
        limit: usize,
        x_range: Option<[f64; 2]>,
    ) -> Vec<[f64; 2]> {
        let Some(x_values) = self.columns.get(x_column).map(|column| &column.values) else {
            return Vec::new();
        };
        let Some(y_values) = self.columns.get(y_column).map(|column| &column.values) else {
            return Vec::new();
        };
        let in_range = |x: f64| {
            x_range.is_none_or(|[left, right]| x >= left.min(right) && x <= left.max(right))
        };
        let eligible_count = x_values
            .iter()
            .zip(y_values)
            .zip(&self.alive)
            .filter(|(values, alive)| {
                let (x, y) = values;
                **alive && x.is_finite() && y.is_finite() && in_range(**x)
            })
            .count();
        let limit = limit.max(2);
        if eligible_count <= limit {
            return x_values
                .iter()
                .zip(y_values)
                .zip(&self.alive)
                .filter_map(|((x, y), alive)| {
                    (*alive && x.is_finite() && y.is_finite() && in_range(*x)).then_some([*x, *y])
                })
                .collect();
        }

        // Keep both y extrema from each source-order bucket. This preserves
        // narrow peaks that ordinary stride sampling can silently skip.
        let bucket_size = eligible_count.div_ceil(limit / 2).max(1);
        let mut output = Vec::with_capacity(limit + 2);
        let mut bucket: Vec<(usize, [f64; 2])> = Vec::with_capacity(bucket_size);
        for (row_index, ((x, y), alive)) in
            x_values.iter().zip(y_values).zip(&self.alive).enumerate()
        {
            if !*alive || !x.is_finite() || !y.is_finite() || !in_range(*x) {
                continue;
            }
            bucket.push((row_index, [*x, *y]));
            if bucket.len() == bucket_size {
                append_bucket_extrema(&mut output, &bucket);
                bucket.clear();
            }
        }
        append_bucket_extrema(&mut output, &bucket);
        output
    }

    pub fn row_points(
        &self,
        x_column: usize,
        y_column: usize,
    ) -> impl Iterator<Item = (usize, [f64; 2])> + '_ {
        let x_values = self
            .columns
            .get(x_column)
            .map_or(&[][..], |column| column.values.as_slice());
        let y_values = self
            .columns
            .get(y_column)
            .map_or(&[][..], |column| column.values.as_slice());
        x_values
            .iter()
            .zip(y_values)
            .zip(&self.alive)
            .enumerate()
            .filter_map(|(row_index, ((x, y), alive))| {
                (*alive && x.is_finite() && y.is_finite()).then_some((row_index, [*x, *y]))
            })
    }

    pub fn rows_in_bounds(
        &self,
        x_column: usize,
        y_column: usize,
        x_bounds: [f64; 2],
        y_bounds: [f64; 2],
    ) -> Vec<usize> {
        let x_min = x_bounds[0].min(x_bounds[1]);
        let x_max = x_bounds[0].max(x_bounds[1]);
        let y_min = y_bounds[0].min(y_bounds[1]);
        let y_max = y_bounds[0].max(y_bounds[1]);
        self.row_points(x_column, y_column)
            .filter_map(|(row_index, [x, y])| {
                (x >= x_min && x <= x_max && y >= y_min && y <= y_max).then_some(row_index)
            })
            .collect()
    }

    pub fn delete_rows(&mut self, rows: &[usize]) -> Vec<usize> {
        let mut changed = Vec::with_capacity(rows.len());
        for &row_index in rows {
            if let Some(alive) = self.alive.get_mut(row_index)
                && *alive
            {
                *alive = false;
                changed.push(row_index);
            }
        }
        changed
    }

    pub fn restore_rows(&mut self, rows: &[usize]) {
        for &row_index in rows {
            if let Some(alive) = self.alive.get_mut(row_index) {
                *alive = true;
            }
        }
    }
}

fn append_bucket_extrema(output: &mut Vec<[f64; 2]>, bucket: &[(usize, [f64; 2])]) {
    let Some(minimum) = bucket
        .iter()
        .min_by(|left, right| left.1[1].total_cmp(&right.1[1]))
    else {
        return;
    };
    let Some(maximum) = bucket
        .iter()
        .max_by(|left, right| left.1[1].total_cmp(&right.1[1]))
    else {
        return;
    };
    if minimum.0 <= maximum.0 {
        output.push(minimum.1);
        if minimum.0 != maximum.0 {
            output.push(maximum.1);
        }
    } else {
        output.push(maximum.1);
        output.push(minimum.1);
    }
}

#[cfg(test)]
mod tests {
    use super::{DataSet, DataSetKind, NumericColumn, generated_plot_id};
    use std::path::{Path, PathBuf};

    fn dataset(x: Vec<f64>, y: Vec<f64>) -> DataSet {
        let row_count = x.len();
        DataSet {
            source: PathBuf::from("sample.csv"),
            label: None,
            kind: DataSetKind::Source,
            plot_id: "sample-id".to_owned(),
            fit_link: None,
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "x".to_owned(),
                    values: x,
                },
                NumericColumn {
                    name: "y".to_owned(),
                    values: y,
                },
            ],
            row_count,
            alive: vec![true; row_count],
        }
    }

    #[test]
    fn generated_identity_keeps_the_existing_byte_contract() {
        let columns = vec![
            NumericColumn {
                name: "x".to_owned(),
                values: vec![1.0, 2.0],
            },
            NumericColumn {
                name: "y".to_owned(),
                values: vec![3.0, 4.0],
            },
        ];
        assert_eq!(
            generated_plot_id(Path::new("sample.csv"), &columns),
            "instplot-c2bb893aa33ffbd5"
        );
    }

    #[test]
    fn plot_points_are_bounded_and_finite() {
        let data = dataset(vec![1.0, 3.0, 5.0, 7.0], vec![2.0, f64::NAN, 6.0, 8.0]);
        let points = data.plot_points(0, 1, 2, None);
        assert!(points.len() <= 2);
        assert!(points.iter().flatten().all(|value| value.is_finite()));
    }

    #[test]
    fn min_max_decimation_preserves_a_narrow_peak() {
        let x = (0..100).map(f64::from).collect();
        let y = (0..100)
            .map(|index| if index == 51 { 500.0 } else { 0.0 })
            .collect();
        let data = dataset(x, y);
        let points = data.plot_points(0, 1, 10, Some([0.0, 99.0]));
        assert!(points.len() <= 10);
        assert!(points.iter().any(|point| point[1] == 500.0));
    }

    #[test]
    fn deletion_and_restoration_only_change_requested_rows() {
        let mut data = dataset(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 2.0]);
        assert_eq!(data.delete_rows(&[1, 99]), [1]);
        assert_eq!(
            data.row_points(0, 1)
                .map(|(row, _)| row)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        data.restore_rows(&[1]);
        assert!(data.alive.iter().all(|alive| *alive));
    }

    #[test]
    fn rectangle_selection_uses_complete_alive_data() {
        let mut data = dataset(vec![0.0, 1.0, 2.0], vec![0.0, 10.0, 2.0]);
        data.delete_rows(&[0]);
        assert_eq!(data.rows_in_bounds(0, 1, [0.0, 2.0], [1.0, 11.0]), [1, 2]);
    }
}
