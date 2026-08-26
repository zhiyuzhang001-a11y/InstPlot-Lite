use std::io::Write;
use std::path::Path;

use eframe::egui::{ColorImage, Rect};

pub fn save_plot_png(
    path: &Path,
    image: &ColorImage,
    plot_rect: Rect,
    pixels_per_point: f32,
) -> Result<(), String> {
    let bytes = encode_plot_png(image, plot_rect, pixels_per_point)?;
    let mut file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())
}

fn encode_plot_png(
    image: &ColorImage,
    plot_rect: Rect,
    pixels_per_point: f32,
) -> Result<Vec<u8>, String> {
    let image_width = image.size[0];
    let image_height = image.size[1];
    let left = (plot_rect.left() * pixels_per_point)
        .floor()
        .clamp(0.0, image_width as f32) as usize;
    let right = (plot_rect.right() * pixels_per_point)
        .ceil()
        .clamp(0.0, image_width as f32) as usize;
    let top = (plot_rect.top() * pixels_per_point)
        .floor()
        .clamp(0.0, image_height as f32) as usize;
    let bottom = (plot_rect.bottom() * pixels_per_point)
        .ceil()
        .clamp(0.0, image_height as f32) as usize;
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    if width == 0 || height == 0 {
        return Err("绘图区尺寸无效，无法导出图片".to_owned());
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in top..bottom {
        for x in left..right {
            rgba.extend_from_slice(&light_export_rgba(
                image.pixels[y * image_width + x].to_srgba_unmultiplied(),
            ));
        }
    }

    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(&rgba)
            .map_err(|error| error.to_string())?;
    }
    Ok(output)
}

fn light_export_rgba([red, green, blue, alpha]: [u8; 4]) -> [u8; 4] {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);

    // Plot chrome (background, grid, axes, labels, and legend) is neutral gray.
    // Map bright text to black and darker chrome to white/light gray while
    // leaving colored data series unchanged.
    if maximum - minimum <= 12 {
        let gray = ((u16::from(red) + u16::from(green) + u16::from(blue)) / 3) as u8;
        let export_gray = if gray >= 128 { 0 } else { 255 - gray / 2 };
        [export_gray, export_gray, export_gray, alpha]
    } else {
        [red, green, blue, alpha]
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_plot_png, light_export_rgba};
    use eframe::egui::{Color32, ColorImage, Rect, pos2};

    #[test]
    fn encodes_only_requested_plot_rectangle_as_png() {
        let image = ColorImage::new([4, 4], vec![Color32::WHITE; 16]);
        let bytes = encode_plot_png(
            &image,
            Rect::from_min_max(pos2(1.0, 1.0), pos2(3.0, 3.0)),
            1.0,
        )
        .unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn light_export_inverts_neutral_plot_chrome() {
        assert_eq!(light_export_rgba([0, 0, 0, 255]), [255, 255, 255, 255]);
        assert_eq!(light_export_rgba([255, 255, 255, 255]), [0, 0, 0, 255]);
        assert_eq!(light_export_rgba([64, 64, 64, 128]), [223, 223, 223, 128]);
        assert_eq!(light_export_rgba([180, 180, 180, 255]), [0, 0, 0, 255]);
    }

    #[test]
    fn light_export_preserves_colored_series() {
        assert_eq!(light_export_rgba([214, 79, 79, 255]), [214, 79, 79, 255]);
        assert_eq!(light_export_rgba([76, 145, 222, 192]), [76, 145, 222, 192]);
    }
}
