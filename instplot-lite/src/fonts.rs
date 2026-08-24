use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, FontFamily};

const BUNDLED_FONT: &[u8] = include_bytes!("../assets/InstPlotSansSC-Level1.otf");

pub fn install_cjk_font(context: &egui::Context) -> String {
    let (bytes, source) = font_bytes();
    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.insert(
        "instplot-cjk".to_owned(),
        Arc::new(egui::FontData::from_owned(bytes)),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, "instplot-cjk".to_owned());
    }
    context.set_fonts(definitions);
    format!("中文字体已加载：{source}")
}

fn font_bytes() -> (Vec<u8>, String) {
    if let Some(path) = std::env::var_os("INSTPLOT_LITE_FONT").map(PathBuf::from)
        && let Ok(bytes) = std::fs::read(&path)
        && bytes.len() >= 1024
    {
        return (bytes, path.display().to_string());
    }

    (BUNDLED_FONT.to_vec(), "内置精简字体".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{BUNDLED_FONT, font_bytes};

    #[test]
    fn bundled_font_is_present_and_compact() {
        let (bytes, source) = font_bytes();
        assert_eq!(bytes.len(), BUNDLED_FONT.len());
        assert!(bytes.len() > 1_000_000);
        assert!(bytes.len() < 1_200_000);
        assert_eq!(source, "内置精简字体");
    }
}
