use super::*;
use crate::studio_edit_protocol::{self, Column, Transfer};

pub(super) struct StudioEdit {
    request_path: PathBuf,
    request: Transfer,
    sent: bool,
    last_poll: std::time::Instant,
    drafts: std::collections::BTreeMap<(String, String), PathBuf>,
    draft_files: Vec<PathBuf>,
    activation_sequence: u64,
}

impl InstPlotLiteApp {
    pub(crate) fn open_studio_edit(&mut self, path: PathBuf) -> Result<(), String> {
        studio_edit_protocol::result_path(&path)?;
        let request = studio_edit_protocol::read_transfer(&path)?;
        let activation_sequence = self
            .studio_edit
            .as_ref()
            .filter(|e| e.request.session_id == request.session_id)
            .map_or(0, |e| e.activation_sequence);
        let switching = self
            .studio_edit
            .as_ref()
            .is_some_and(|e| e.request.target_id != request.target_id);
        let drafts = self
            .studio_edit
            .as_ref()
            .map(|e| e.drafts.clone())
            .unwrap_or_default();
        let draft_files = self
            .studio_edit
            .as_ref()
            .map(|e| e.draft_files.clone())
            .unwrap_or_default();
        let restored = if switching {
            drafts
                .get(&(request.target_id.clone(), request.base_sha256.clone()))
                .map(|p| studio_edit_protocol::read_transfer(p))
                .transpose()?
                .filter(|d| d.matches_session(&request))
        } else {
            None
        };
        let data = restored.as_ref().unwrap_or(&request);
        self.x_column = data
            .columns
            .iter()
            .position(|c| c.name == data.x_column)
            .unwrap();
        self.y_column = data
            .columns
            .iter()
            .position(|c| c.name == data.y_column)
            .unwrap();
        self.datasets = vec![data::DataSet {
            source: path.clone(),
            label: Some(request.label.clone()),
            kind: data::DataSetKind::Source,
            plot_id: request.target_id.clone(),
            fit_link: None,
            encoding: "Studio edit v1".to_owned(),
            separator: "embedded".to_owned(),
            columns: data
                .columns
                .iter()
                .map(|c| data::NumericColumn {
                    name: c.name.clone(),
                    values: c.values.iter().map(|v| v.unwrap_or(f64::NAN)).collect(),
                })
                .collect(),
            row_count: data.alive.len(),
            alive: data.alive.clone(),
        }];
        self.active_dataset = 0;
        self.history = EditHistory::default();
        self.pending_deletion = None;
        self.selection_start = None;
        self.selection_current = None;
        self.selected_coordinate = None;
        self.visible_x_range = None;
        self.fit_results = FitResultState::default();
        self.fit_settings.message.clear();
        self.processing_settings.scope = ProcessingScope::Current;
        self.processing_settings.selected_datasets = vec![true];
        if let Some([min, max]) = finite_range(&self.datasets[0].columns[self.x_column].values) {
            self.processing_settings.fit_min = min;
            self.processing_settings.fit_max = max;
            self.processing_settings.local_min = min;
            self.processing_settings.local_max = max;
            self.processing_settings.denoise_min = min;
            self.processing_settings.denoise_max = max;
            self.processing_settings.set_linear_defaults(min, max);
        }
        self.processing_settings.linear_preview = None;
        self.fit_settings.scope = FitScope::Current;
        self.fit_settings.selected_datasets = vec![true];
        self.refresh_fit_defaults_from_active_dataset();
        self.reset_view = true;
        self.status = "处理完成后点击“发回 Studio”；原始文件不会被覆盖。".to_owned();
        self.studio_edit = Some(StudioEdit {
            request_path: path,
            request,
            sent: false,
            last_poll: std::time::Instant::now(),
            drafts,
            draft_files,
            activation_sequence,
        });
        if restored.is_some() {
            self.status = "已恢复这条曲线未发回的草稿；原文件未覆盖。".to_owned();
        }
        Ok(())
    }

    fn send_studio_edit(&mut self) -> Result<(), String> {
        let edit = self.studio_edit.as_ref().ok_or("没有 Studio 编辑会话")?;
        if edit.sent {
            return Err("等待 Studio 确认本次结果".to_owned());
        }
        let current_request = studio_edit_protocol::read_transfer(&edit.request_path)?;
        if !current_request.matches_session(&edit.request) || current_request != edit.request {
            return Err("Studio 编辑请求已变化，未发送".to_owned());
        }
        let result = self.studio_edit_snapshot()?;
        studio_edit_protocol::write_transfer(
            &studio_edit_protocol::result_path(&edit.request_path)?,
            &result,
        )?;
        self.studio_edit.as_mut().unwrap().sent = true;
        Ok(())
    }

    fn studio_edit_snapshot(&self) -> Result<Transfer, String> {
        let edit = self.studio_edit.as_ref().ok_or("没有 Studio 编辑会话")?;
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
        Ok(result)
    }

    pub(super) fn studio_edit_bar(&mut self, ui: &mut egui::Ui) {
        self.poll_studio_edit(ui.ctx());
        let Some(edit) = &self.studio_edit else {
            return;
        };
        ui.ctx().send_viewport_cmd_to(
            egui::ViewportId::ROOT,
            egui::ViewportCommand::Title(format!(
                "InstPlot Lite — Studio 编辑：{}",
                edit.request.label
            )),
        );
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
            if !edit.drafts.is_empty() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    format!(
                        "已保留 {} 份未发回草稿，切回原曲线可继续（请勿关闭 Lite）。",
                        edit.drafts.len()
                    ),
                );
            }
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
        context.request_repaint_after_for(
            std::time::Duration::from_millis(250),
            egui::ViewportId::ROOT,
        );
        if edit.last_poll.elapsed() < std::time::Duration::from_millis(250) {
            return;
        }
        edit.last_poll = std::time::Instant::now();
        let activation = edit
            .activation_sequence
            .checked_add(1)
            .and_then(|next| {
                match studio_edit_protocol::read_activation(
                    edit.request_path.parent()?,
                    &edit.request.session_id,
                    next,
                ) {
                    Ok(true) => {
                        edit.activation_sequence = next;
                        Some(true)
                    }
                    Ok(false) => Some(false),
                    Err(e) => {
                        self.status = format!("窗口唤起失败：{e}");
                        Some(false)
                    }
                }
            })
            .unwrap_or(false);
        if activation {
            self.focus_studio_editor(context);
        }
        let edit = self.studio_edit.as_ref().unwrap();
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
        let switching = request.target_id != self.studio_edit.as_ref().unwrap().request.target_id;
        if switching {
            let snapshot = match self.studio_edit_snapshot() {
                Ok(s) => s,
                Err(e) => {
                    self.status = format!("切换暂停：{e}；请先导出当前数据");
                    return;
                }
            };
            let edit = self.studio_edit.as_mut().unwrap();
            if snapshot != edit.request {
                // Retain unsent work before changing dataset. Restore only when
                // the Studio source revision still matches, never after undo.
                let bytes: u64 = edit
                    .draft_files
                    .iter()
                    .filter_map(|p| p.metadata().ok())
                    .map(|m| m.len())
                    .sum();
                let incoming_bytes = serde_json::to_vec(&snapshot)
                    .map(|v| v.len() as u64)
                    .unwrap_or(u64::MAX);
                if edit.draft_files.len() >= 32
                    || bytes.saturating_add(incoming_bytes) > 128 * 1024 * 1024
                {
                    self.status = "草稿已达上限，未切换；请先导出当前修改。".to_owned();
                    return;
                }
                let draft_path = edit.request_path.with_file_name(format!(
                    "draft-{}",
                    edit.request_path.file_name().unwrap().to_string_lossy()
                ));
                if let Err(e) = studio_edit_protocol::write_transfer(&draft_path, &snapshot) {
                    self.status = format!("草稿未保存，未切换：{e}");
                    return;
                }
                edit.drafts.insert(
                    (snapshot.target_id.clone(), snapshot.base_sha256.clone()),
                    draft_path.clone(),
                );
                edit.draft_files.push(draft_path);
            }
        }
        // Acknowledging an identical result keeps Lite's processing undo history.
        // Studio undo/redo (or switching dataset) replaces the editor baseline.
        let unchanged = self.datasets.get(self.active_dataset).is_some_and(|d| {
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
        let only_bound_dataset = self.datasets.len() == 1
            && self.datasets[0].plot_id == self.studio_edit.as_ref().unwrap().request.target_id;
        if !unchanged && !only_bound_dataset {
            self.status="此联动窗口还打开了其他数据，已暂停同步以保护编辑；请先导出并关闭额外数据，或使用独立 Lite 窗口。".to_owned();
            return;
        }
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
        // On macOS making the root visible can raise it after immediate child
        // viewport commands. Keep the open editor in front on target switches.
        if switching && !self.processing_open && !self.fit_open {
            context
                .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Visible(true));
            context.send_viewport_cmd_to(
                egui::ViewportId::ROOT,
                egui::ViewportCommand::Minimized(false),
            );
            focus_viewport(context, egui::ViewportId::ROOT);
        }
        context.request_repaint_of(egui::ViewportId::ROOT);
        if self.processing_open {
            context.request_repaint_of(processing_viewport_id());
        }
        if self.fit_open {
            context.request_repaint_of(fitting_viewport_id());
        }
        if switching {
            if self.processing_open {
                focus_viewport(context, processing_viewport_id());
            } else if self.fit_open {
                focus_viewport(context, fitting_viewport_id());
            }
        }
        context.request_repaint();
    }

    fn focus_studio_editor(&self, context: &egui::Context) {
        let target = if self.processing_open {
            processing_viewport_id()
        } else if self.fit_open {
            fitting_viewport_id()
        } else {
            egui::ViewportId::ROOT
        };
        context.send_viewport_cmd_to(target, egui::ViewportCommand::Visible(true));
        focus_viewport(context, target);
        context.request_repaint_of(target);
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
        // Explicit re-open raises only this session, without a data revision:
        // unsent values/masks and its open processing window must survive.
        let context = egui::Context::default();
        context.set_embed_viewports(false);
        studio_edit_protocol::write_activation(&root, &request.session_id, 1).unwrap();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        let mut output = context.run_ui(egui::RawInput::default(), |_| {
            app.poll_studio_edit(&context)
        });
        output.textures_delta.clear();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Focus)
        );
        assert_eq!(app.datasets[0].columns[1].values[1], 8.5);
        assert!(!app.datasets[0].alive[0]);
        assert_eq!(app.studio_edit.as_ref().unwrap().request_path, path);
        assert_eq!(app.studio_edit.as_ref().unwrap().activation_sequence, 1);
        assert!(!root.join("request-1.json").exists());
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
        assert_eq!(app.datasets.len(), 2);
        assert_eq!(app.datasets[1].plot_id, "unrelated");
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
        assert_eq!(app.studio_edit.as_ref().unwrap().request_path, next);
        assert_eq!(app.datasets.len(), 2);
        assert_eq!(app.datasets[0].columns[1].values[1], 8.5);
        assert!(app.status.contains("已暂停同步"));
        // Removing the additional dataset explicitly allows the queued update.
        app.datasets.remove(1);
        app.datasets[0].plot_id = "foreign-only".into();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll_studio_edit(&context);
        assert_eq!(app.datasets[0].plot_id, "foreign-only");
        assert_eq!(app.studio_edit.as_ref().unwrap().request_path, next);
        app.datasets[0].plot_id = request.target_id.clone();
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
        // Switching targets updates already-open tool windows and their ranges,
        // while retaining unreturned work for a later switch back.
        app.datasets[0].columns[1].values[1] = 99.0;
        app.processing_open = true;
        app.fit_open = true;
        app.pending_deletion = Some(PendingDeletion {
            dataset_index: 0,
            rows: vec![0],
            x_column: 0,
            y_column: 1,
        });
        let mut other = request.clone();
        other.target_id = "other".to_owned();
        other.label = "other curve".to_owned();
        other.columns[0].values = vec![Some(10.0), Some(20.0), Some(30.0), None];
        let other_path = studio_edit_protocol::next_request_path(&undo_path).unwrap();
        studio_edit_protocol::write_transfer(&other_path, &other).unwrap();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll_studio_edit(&context);
        assert_eq!(app.datasets[0].plot_id, "other");
        assert!(app.processing_open && app.fit_open);
        assert!(app.pending_deletion.is_none());
        assert_eq!(
            (
                app.processing_settings.fit_min,
                app.processing_settings.fit_max
            ),
            (10.0, 30.0)
        );
        assert_eq!(
            (app.fit_settings.x_min, app.fit_settings.x_max),
            (10.0, 30.0)
        );
        let back_path = studio_edit_protocol::next_request_path(&other_path).unwrap();
        studio_edit_protocol::write_transfer(&back_path, &request).unwrap();
        app.studio_edit.as_mut().unwrap().last_poll =
            std::time::Instant::now() - std::time::Duration::from_secs(1);
        app.poll_studio_edit(&context);
        assert_eq!(app.datasets[0].plot_id, request.target_id);
        assert_eq!(app.datasets[0].columns[1].values[1], 99.0);
        for draft in app.studio_edit.as_ref().unwrap().drafts.values() {
            std::fs::remove_file(draft).unwrap();
        }
        std::fs::remove_file(other_path).unwrap();
        std::fs::remove_file(back_path).unwrap();
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
        std::fs::remove_file(root.join("activate-1")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
