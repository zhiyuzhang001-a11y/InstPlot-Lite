use std::path::PathBuf;

use crate::data;

use super::InstPlotLiteApp;

impl InstPlotLiteApp {
    pub(super) fn open_files(&mut self) {
        let paths = rfd::FileDialog::new()
            .add_filter("数据文件", &["txt", "csv", "dat", "tsv", "xlsx", "xls"])
            .add_filter("文本数据", &["txt", "csv", "dat", "tsv"])
            .add_filter("Excel 工作簿", &["xlsx", "xls"])
            .pick_files();
        if let Some(paths) = paths {
            self.load_paths(paths);
        }
    }

    pub(super) fn load_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let had_datasets = !self.datasets.is_empty();
        let previous_column_names = self.datasets.get(self.active_dataset).and_then(|dataset| {
            Some((
                dataset.columns.get(self.x_column)?.name.clone(),
                dataset.columns.get(self.y_column)?.name.clone(),
            ))
        });
        let first_new_dataset = self.datasets.len();
        let mut loaded_files = 0_usize;
        let mut loaded_datasets = 0_usize;
        let mut last_summary = String::new();
        let mut errors = Vec::new();
        for path in paths {
            match data::read_data_file(&path) {
                Ok(datasets) => {
                    if let Some(dataset) = datasets.iter().find(|candidate| {
                        candidate.kind == data::DataSetKind::Source
                            && self.datasets.iter().any(|existing| {
                                existing.kind == data::DataSetKind::Source
                                    && existing.plot_id == candidate.plot_id
                            })
                    }) {
                        errors.push(format!(
                            "{}：原始数据 Dataset-ID“{}”已在当前会话中使用；为防止拟合关联错误，未重复导入",
                            path.display(),
                            dataset.plot_id
                        ));
                        continue;
                    }
                    loaded_files += 1;
                    loaded_datasets += datasets.len();
                    if let Some(dataset) = datasets.last() {
                        last_summary = format!(
                            "{}：{} 行，{} 个数值列，编码 {}，分隔符 {}",
                            dataset.display_name(),
                            dataset.row_count,
                            dataset.columns.len(),
                            dataset.encoding,
                            dataset.separator
                        );
                    }
                    self.datasets.extend(datasets);
                }
                Err(error) => errors.push(format!("{}：{error}", path.display())),
            }
        }
        if loaded_datasets > 0 {
            self.selected_coordinate = None;
            self.active_dataset = first_new_dataset;
            let column_names = self.column_names();
            (self.x_column, self.y_column) =
                preferred_import_columns(&column_names, previous_column_names.as_ref());
            self.reset_view = true;
            self.visible_x_range = None;
        }
        self.status = match (loaded_files, errors.is_empty()) {
            (0, _) => errors.join("；"),
            (_, true) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；{last_summary}{}",
                if had_datasets && previous_column_names.is_some() {
                    "；新文件已优先匹配已有 X/Y 列"
                } else {
                    ""
                }
            ),
            (_, false) => format!(
                "已导入 {loaded_files} 个文件，共 {loaded_datasets} 个数据集；另有 {} 个失败：{}",
                errors.len(),
                errors.join("；")
            ),
        };
    }
}

fn preferred_import_columns(
    column_names: &[String],
    previous: Option<&(String, String)>,
) -> (usize, usize) {
    let default_y = usize::from(column_names.len() > 1);
    let x_column = previous
        .and_then(|(x_name, _)| column_names.iter().position(|name| name == x_name))
        .unwrap_or(0);
    let y_column = previous
        .and_then(|(_, y_name)| column_names.iter().position(|name| name == y_name))
        .unwrap_or_else(|| {
            if default_y != x_column {
                default_y
            } else {
                (0..column_names.len())
                    .find(|index| *index != x_column)
                    .unwrap_or(x_column)
            }
        });
    (x_column, y_column)
}

#[cfg(test)]
mod tests {
    use super::preferred_import_columns;

    #[test]
    fn imported_dataset_prefers_existing_coordinate_column_names() {
        let columns = ["signal", "temperature", "field"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            preferred_import_columns(&columns, Some(&("field".to_owned(), "signal".to_owned())),),
            (2, 0)
        );
    }

    #[test]
    fn imported_dataset_preserves_a_same_column_x_y_choice() {
        let columns = ["signal", "field"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            preferred_import_columns(&columns, Some(&("field".to_owned(), "field".to_owned())),),
            (1, 1)
        );
    }
}
