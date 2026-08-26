use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, FontFamily};

const BUNDLED_CJK_FONT: &[u8] = include_bytes!("../assets/InstPlotSansSC-Level1.otf");
const BUNDLED_LATIN_FONT: &[u8] = include_bytes!("../assets/InstPlotSans-Latin.ttf");

pub fn install_interface_fonts(context: &egui::Context) {
    let (bytes, _) = font_bytes();
    context.set_fonts(font_definitions(bytes, BUNDLED_LATIN_FONT));
}

fn font_definitions(cjk_bytes: Vec<u8>, latin_bytes: &'static [u8]) -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.insert(
        "instplot-latin".to_owned(),
        Arc::new(egui::FontData::from_static(latin_bytes)),
    );
    definitions.font_data.insert(
        "instplot-cjk".to_owned(),
        Arc::new(egui::FontData::from_owned(cjk_bytes)),
    );

    // Use the bundled Arial-compatible face for interface text, then fall
    // through to the bundled Chinese subset for glyphs it does not contain.
    let proportional = definitions
        .families
        .entry(FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "instplot-latin".to_owned());
    proportional.push("instplot-cjk".to_owned());

    // Preserve egui's actual monospace face for numeric/function editing.
    definitions
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("instplot-cjk".to_owned());
    definitions
}

fn font_bytes() -> (Vec<u8>, String) {
    if let Some(path) = std::env::var_os("INSTPLOT_LITE_FONT").map(PathBuf::from)
        && let Ok(bytes) = std::fs::read(&path)
        && bytes.len() >= 1024
    {
        return (bytes, path.display().to_string());
    }

    (BUNDLED_CJK_FONT.to_vec(), "内置精简字体".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{BUNDLED_CJK_FONT, BUNDLED_LATIN_FONT, font_bytes, font_definitions};
    use eframe::egui::FontFamily;

    #[test]
    fn bundled_fonts_are_present_and_compact() {
        let (bytes, source) = font_bytes();
        assert_eq!(bytes.len(), BUNDLED_CJK_FONT.len());
        assert!(bytes.len() > 1_000_000);
        assert!(bytes.len() < 1_200_000);
        assert!(BUNDLED_LATIN_FONT.len() > 100_000);
        assert!(BUNDLED_LATIN_FONT.len() < 150_000);
        assert_eq!(source, "内置精简字体");
    }

    #[test]
    fn bundled_latin_is_primary_and_cjk_is_fallback() {
        let definitions = font_definitions(BUNDLED_CJK_FONT.to_vec(), BUNDLED_LATIN_FONT);
        let proportional = &definitions.families[&FontFamily::Proportional];
        assert_eq!(
            proportional.first().map(String::as_str),
            Some("instplot-latin")
        );
        assert_eq!(
            proportional.last().map(String::as_str),
            Some("instplot-cjk")
        );

        let monospace = &definitions.families[&FontFamily::Monospace];
        assert_ne!(
            monospace.first().map(String::as_str),
            Some("instplot-latin")
        );
        assert_eq!(monospace.last().map(String::as_str), Some("instplot-cjk"));
    }
}
