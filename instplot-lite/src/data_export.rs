use std::path::Path;

use crate::data::DataSet;

pub fn save_retained_rows(path: &Path, dataset: &DataSet) -> Result<usize, String> {
    let delimiter = if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
    {
        b','
    } else {
        b'\t'
    };
    let bytes = encode_retained_rows(dataset, delimiter)?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    Ok(dataset.alive.iter().filter(|alive| **alive).count())
}

fn encode_retained_rows(dataset: &DataSet, delimiter: u8) -> Result<Vec<u8>, String> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(Vec::new());
    writer
        .write_record(dataset.columns.iter().map(|column| column.name.as_str()))
        .map_err(|error| error.to_string())?;

    let mut fields = Vec::with_capacity(dataset.columns.len());
    for row_index in 0..dataset.row_count {
        if !dataset.alive.get(row_index).copied().unwrap_or(false) {
            continue;
        }
        fields.clear();
        for column in &dataset.columns {
            let value = column.values.get(row_index).copied().unwrap_or(f64::NAN);
            fields.push(if value.is_finite() {
                value.to_string()
            } else {
                String::new()
            });
        }
        writer
            .write_record(&fields)
            .map_err(|error| error.to_string())?;
    }
    writer.into_inner().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::encode_retained_rows;
    use crate::data::{DataSet, NumericColumn};
    use std::path::PathBuf;

    fn dataset() -> DataSet {
        DataSet {
            source: PathBuf::from("sample.csv"),
            encoding: "UTF-8".to_owned(),
            separator: ",".to_owned(),
            columns: vec![
                NumericColumn {
                    name: "磁场,Oe".to_owned(),
                    values: vec![1.0, 2.0, 3.0],
                },
                NumericColumn {
                    name: "信号".to_owned(),
                    values: vec![4.0, 5.0, f64::NAN],
                },
            ],
            row_count: 3,
            alive: vec![true, false, true],
        }
    }

    #[test]
    fn csv_export_keeps_headers_aligned_and_omits_deleted_rows() {
        let bytes = encode_retained_rows(&dataset(), b',').unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "\"磁场,Oe\",信号\n1,4\n3,\n");
    }
}
