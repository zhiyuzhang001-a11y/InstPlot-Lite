use std::path::PathBuf;

use eframe::egui;

use super::InstPlotLiteApp;
use crate::image_export;

impl InstPlotLiteApp {
    pub(super) fn request_plot_png(&mut self, context: &egui::Context) {
        if self.last_export_rect.is_none() {
            self.status = "绘图区尚未准备好，暂时无法导出图片".to_owned();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG 图片", &["png"])
            .set_file_name("instplot-plot.png")
            .save_file()
        else {
            return;
        };
        self.pending_screenshot = Some(with_png_extension(path));
        request_screenshot(context);
        self.status = "正在生成绘图区 PNG…".to_owned();
    }

    pub(super) fn begin_startup_screenshot_if_requested(&mut self, context: &egui::Context) {
        let Some(path) = self.startup_screenshot.take() else {
            return;
        };
        self.pending_screenshot = Some(with_png_extension(path));
        self.close_after_screenshot = true;
        request_screenshot(context);
    }

    pub(super) fn handle_screenshot_result(&mut self, context: &egui::Context) {
        let image = context.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            if self.pending_screenshot.is_some() {
                context.request_repaint_after(std::time::Duration::from_millis(50));
            }
            return;
        };
        let (Some(path), Some(plot_rect)) = (self.pending_screenshot.take(), self.last_export_rect)
        else {
            return;
        };
        match image_export::save_plot_png(&path, &image, plot_rect, context.pixels_per_point()) {
            Ok(()) => self.status = format!("图片已导出：{}", path.display()),
            Err(error) => self.status = format!("图片导出失败：{error}"),
        }
        if self.close_after_screenshot {
            self.close_after_screenshot = false;
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn request_screenshot(context: &egui::Context) {
    context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    context.request_repaint();
}

fn with_png_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        path
    } else {
        path.with_extension("png")
    }
}

#[cfg(test)]
mod tests {
    use super::with_png_extension;
    use std::path::PathBuf;

    #[test]
    fn screenshot_paths_are_normalized_to_png() {
        assert_eq!(
            with_png_extension(PathBuf::from("plot")),
            PathBuf::from("plot.png")
        );
        assert_eq!(
            with_png_extension(PathBuf::from("plot.PNG")),
            PathBuf::from("plot.PNG")
        );
    }
}
