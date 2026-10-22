use super::*;
use crate::studio_edit_protocol::{self, Column, Transfer};

pub(super) struct StudioEdit {
    request_path: PathBuf,
    request: Transfer,
    sent: bool,
}

impl InstPlotLiteApp {
    pub(crate) fn open_studio_edit(&mut self, path: PathBuf) -> Result<(), String> {
        if path.file_name().and_then(|n| n.to_str()) != Some("request.json") {
            return Err("Studio edit requires request.json".to_owned());
        }
        let request = studio_edit_protocol::read_transfer(&path)?;
        self.x_column = request
            .columns
            .iter()
            .position(|c| c.name == request.x_column)
            .unwrap();
        self.y_column = request
            .columns
            .iter()
            .position(|c| c.name == request.y_column)
            .unwrap();
        self.datasets = vec![data::DataSet {
            source: path.clone(),
            label: Some(request.label.clone()),
            kind: data::DataSetKind::Source,
            plot_id: request.target_id.clone(),
            fit_link: None,
            encoding: "Studio edit v1".to_owned(),
            separator: "embedded".to_owned(),
            columns: request
                .columns
                .iter()
                .map(|c| data::NumericColumn {
                    name: c.name.clone(),
                    values: c.values.iter().map(|v| v.unwrap_or(f64::NAN)).collect(),
                })
                .collect(),
            row_count: request.alive.len(),
            alive: request.alive.clone(),
        }];
        self.active_dataset = 0;
        self.history = EditHistory::default();
        self.reset_view = true;
        self.status = "处理完成后点击“发回 Studio”；原始文件不会被覆盖。".to_owned();
        self.studio_edit = Some(StudioEdit {
            request_path: path,
            request,
            sent: false,
        });
        Ok(())
    }

    fn send_studio_edit(&mut self) -> Result<(), String> {
        let edit = self.studio_edit.as_mut().ok_or("没有 Studio 编辑会话")?;
        if edit.sent {
            return Err("本次结果已发回；请从 Studio 重新发起下一次编辑".to_owned());
        }
        let current_request = studio_edit_protocol::read_transfer(&edit.request_path)?;
        if !current_request.matches_session(&edit.request) || current_request != edit.request {
            return Err("Studio 编辑请求已变化，未发送".to_owned());
        }
        let dataset = self
            .datasets
            .get(self.active_dataset)
            .ok_or("没有选中数据")?;
        if dataset.plot_id != edit.request.target_id {
            return Err("请选择从 Studio 接收的原始数据组，不能发回其他文件或拟合曲线".to_owned());
        }
        let mut result = edit.request.clone();
        result.x_column = dataset
            .columns
            .get(self.x_column)
            .ok_or("X 列缺失")?
            .name
            .clone();
        result.y_column = dataset
            .columns
            .get(self.y_column)
            .ok_or("Y 列缺失")?
            .name
            .clone();
        result.columns = dataset
            .columns
            .iter()
            .map(|c| Column {
                name: c.name.clone(),
                values: c
                    .values
                    .iter()
                    .map(|v| v.is_finite().then_some(*v))
                    .collect(),
            })
            .collect();
        result.alive = dataset.alive.clone();
        studio_edit_protocol::write_transfer(
            &edit.request_path.with_file_name("result.json"),
            &result,
        )?;
        edit.sent = true;
        Ok(())
    }

    pub(super) fn studio_edit_bar(&mut self, ui: &mut egui::Ui) {
        let Some(edit) = &self.studio_edit else {
            return;
        };
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "InstPlot Lite — Studio 编辑：{}",
                edit.request.label
            )));
        let mut send = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Studio 编辑：{}", edit.request.label));
            send = ui
                .add_enabled(
                    !edit.sent && self.pending_deletion.is_none(),
                    egui::Button::new("发回 Studio"),
                )
                .clicked();
            ui.weak(if edit.sent {
                "已发回，请在 Studio 确认更新。"
            } else {
                "只更新这一组；原文件和 Studio 排版不变。"
            });
        });
        if send {
            self.status = match self.send_studio_edit() {
                Ok(()) => "已发回 Studio，请在 Studio 确认更新；本窗口数据仍保留。".to_owned(),
                Err(error) => format!("发回失败：{error}"),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn studio_session_keeps_exact_columns_masks_and_identity_and_returns_only_its_dataset_once() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("instplot-lite-test-{id}"));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root).unwrap();
        let request = Transfer {
            schema_version: 1,
            session_id: "a".repeat(64),
            target_id: "3nm".into(),
            base_sha256: "b".repeat(64),
            label: "3 nm 电阻".into(),
            x_column: "x".into(),
            y_column: "change".into(),
            columns: vec![
                Column {
                    name: "x".into(),
                    values: vec![Some(0.0), Some(1.0), Some(2.0), None],
                },
                Column {
                    name: "change".into(),
                    values: vec![Some(1.0), Some(2.0), Some(3.0), None],
                },
            ],
            alive: vec![true, true, true, false],
        };
        let path = root.join("request.json");
        studio_edit_protocol::write_transfer(&path, &request).unwrap();
        let creation = eframe::CreationContext::_new_kittest(egui::Context::default());
        let mut app = InstPlotLiteApp::new(&creation, Vec::new(), None);
        app.open_studio_edit(path.clone()).unwrap();
        assert_eq!(app.datasets.len(), 1);
        assert_eq!(app.datasets[0].plot_id, "3nm");
        assert_eq!((app.x_column, app.y_column), (0, 1));
        assert!(app.datasets[0].columns[1].values[3].is_nan());
        let mut unrelated = app.datasets[0].clone();
        unrelated.plot_id = "unrelated".into();
        app.datasets.push(unrelated);
        app.active_dataset = 1;
        assert!(app.send_studio_edit().is_err());
        assert!(!root.join("result.json").exists());
        app.active_dataset = 0;
        app.datasets[0].columns[1].values[1] = 8.5;
        app.datasets[0].alive[0] = false;
        app.send_studio_edit().unwrap();
        let result = studio_edit_protocol::read_transfer(&root.join("result.json")).unwrap();
        assert!(result.matches_session(&request));
        assert_eq!(
            result.columns[1].values,
            vec![Some(1.0), Some(8.5), Some(3.0), None]
        );
        assert_eq!(result.alive, vec![false, true, true, false]);
        assert!(app.send_studio_edit().is_err());
        assert_eq!(studio_edit_protocol::read_transfer(&path).unwrap(), request);
        std::fs::remove_file(root.join("result.json")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
