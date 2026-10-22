use super::*;
use crate::studio_edit_protocol::{self, Column, Transfer};

pub(super) struct StudioEdit {
    request_path: PathBuf,
    request: Transfer,
    sent: bool,
    last_poll: std::time::Instant,
}

impl InstPlotLiteApp {
    pub(crate) fn open_studio_edit(&mut self, path: PathBuf) -> Result<(), String> {
        studio_edit_protocol::result_path(&path)?;
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
            last_poll: std::time::Instant::now(),
        });
        Ok(())
    }

    fn send_studio_edit(&mut self) -> Result<(), String> {
        let edit = self.studio_edit.as_mut().ok_or("没有 Studio 编辑会话")?;
        if edit.sent {
            return Err("等待 Studio 确认本次结果".to_owned());
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
            &studio_edit_protocol::result_path(&edit.request_path)?,
            &result,
        )?;
        edit.sent = true;
        Ok(())
    }

    pub(super) fn studio_edit_bar(&mut self, ui: &mut egui::Ui) {
        self.poll_studio_edit(ui.ctx());
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
                "已发回，等待 Studio 确认；确认后可继续编辑。"
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

    fn poll_studio_edit(&mut self, context: &egui::Context) {
        let Some(edit) = &mut self.studio_edit else {
            return;
        };
        context.request_repaint_after(std::time::Duration::from_millis(250));
        if edit.last_poll.elapsed() < std::time::Duration::from_millis(250) {
            return;
        }
        edit.last_poll = std::time::Instant::now();
        let Ok(path) = studio_edit_protocol::next_request_path(&edit.request_path) else {
            return;
        };
        if path.symlink_metadata().is_err() {
            return;
        }
        let received = studio_edit_protocol::read_transfer(&path).and_then(|r| {
            if r.session_id != edit.request.session_id {
                Err("会话身份变化，拒绝同步".to_owned())
            } else {
                Ok(r)
            }
        });
        let request = match received {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        // Acknowledging an identical result keeps Lite's processing undo history.
        // Studio undo/redo (or switching dataset) replaces the editor baseline.
        let unchanged = self.datasets.len() == 1
            && self.datasets.first().is_some_and(|d| {
                d.plot_id == request.target_id
                    && d.alive == request.alive
                    && d.columns.len() == request.columns.len()
                    && d.columns.iter().zip(&request.columns).all(|(a, b)| {
                        a.name == b.name
                            && a.values
                                .iter()
                                .map(|v| v.is_finite().then_some(*v))
                                .eq(b.values.iter().copied())
                    })
                    && d.columns
                        .get(self.x_column)
                        .is_some_and(|c| c.name == request.x_column)
                    && d.columns
                        .get(self.y_column)
                        .is_some_and(|c| c.name == request.y_column)
            });
        if unchanged {
            let edit = self.studio_edit.as_mut().unwrap();
            edit.request_path = path;
            edit.request = request;
            edit.sent = false;
            self.status = "Studio 已同步，可继续编辑和撤销，再次发回同一窗口。".to_owned();
        } else if let Err(e) = self.open_studio_edit(path) {
            self.status = format!("同步失败：{e}");
        } else {
            self.status = "已同步 Studio 当前数据（包括撤销/重做）；联动保持连接。".to_owned();
        }
        context.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        context.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        context.request_repaint();
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
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let mut builder = builder;
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
        // Studio acknowledges without disconnecting or starting another app.
        let context = egui::Context::default();
        let mut ack = result.clone();
        ack.base_sha256 = "c".repeat(64);
        let next = studio_edit_protocol::next_request_path(&path).unwrap();
        studio_edit_protocol::write_transfer(&next, &ack).unwrap();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll_studio_edit(&context);
        assert!(!app.studio_edit.as_ref().unwrap().sent);
        assert_eq!(app.datasets.len(), 1);
        app.send_studio_edit().unwrap();
        let second_result =
            studio_edit_protocol::read_transfer(&studio_edit_protocol::result_path(&next).unwrap())
                .unwrap();
        assert_eq!(second_result, ack);
        // Studio undo publishes the old values under a NEW revision. Old results
        // remain immutable and cannot be silently reapplied.
        let undo_path = studio_edit_protocol::next_request_path(&next).unwrap();
        studio_edit_protocol::write_transfer(&undo_path, &request).unwrap();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll_studio_edit(&context);
        assert_eq!(app.datasets[0].columns[1].values[1], 2.0);
        assert_eq!(app.datasets[0].alive, request.alive);
        app.send_studio_edit().unwrap();
        assert_eq!(
            studio_edit_protocol::read_transfer(
                &studio_edit_protocol::result_path(&undo_path).unwrap()
            )
            .unwrap(),
            request
        );
        for extra in [
            studio_edit_protocol::result_path(&next).unwrap(),
            studio_edit_protocol::result_path(&undo_path).unwrap(),
            next,
            undo_path,
        ] {
            std::fs::remove_file(extra).unwrap();
        }
        std::fs::remove_file(root.join("result.json")).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
