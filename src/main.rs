#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod data;
mod data_export;
mod edit_history;
mod fitting;
mod fonts;
mod image_export;
mod processing;
#[cfg(any(target_os = "windows", test))]
mod updater;

use app::InstPlotLiteApp;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let arguments = startup_arguments();
    #[cfg(all(target_os = "windows", feature = "updater-e2e"))]
    if let Some(status_path) = &arguments.updater_e2e_status {
        if let Err(error) = updater::run_e2e(status_path) {
            let _ = std::fs::write(status_path, format!("error\n{error}\n"));
            std::process::exit(1);
        }
        return Ok(());
    }
    if arguments.check_only {
        let mut failed = false;
        for path in &arguments.files {
            match data::read_data_file(path) {
                Ok(datasets) => {
                    for dataset in datasets {
                        println!(
                            "OK\t{}\t{}\t{} rows\t{} columns\t{}\t{}\t{}",
                            path.display(),
                            dataset.display_name(),
                            dataset.row_count,
                            dataset.columns.len(),
                            dataset.encoding,
                            dataset.separator,
                            dataset
                                .columns
                                .iter()
                                .map(|column| column.name.as_str())
                                .collect::<Vec<_>>()
                                .join(" | ")
                        );
                    }
                }
                Err(error) => {
                    failed = true;
                    eprintln!("ERROR\t{}\t{error}", path.display());
                }
            }
        }
        if failed {
            std::process::exit(1);
        }
        return Ok(());
    }
    let app_icon = application_icon();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 780.0])
            .with_min_inner_size([720.0, 580.0])
            .with_icon(app_icon),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "InstPlot Lite",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(InstPlotLiteApp::new(
                creation_context,
                arguments.files,
                arguments.screenshot,
            )))
        }),
    )
}

fn application_icon() -> eframe::egui::IconData {
    const ICON_SIZE: u32 = 128;
    let source = eframe::icon_data::from_png_bytes(include_bytes!("../InP_logo.png"))
        .expect("embedded application icon must be valid PNG");
    if source.width <= ICON_SIZE && source.height <= ICON_SIZE {
        return source;
    }

    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        let source_y = y * source.height / ICON_SIZE;
        for x in 0..ICON_SIZE {
            let source_x = x * source.width / ICON_SIZE;
            let offset = ((source_y * source.width + source_x) * 4) as usize;
            rgba.extend_from_slice(&source.rgba[offset..offset + 4]);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

struct StartupArguments {
    files: Vec<PathBuf>,
    screenshot: Option<PathBuf>,
    check_only: bool,
    #[cfg(all(target_os = "windows", feature = "updater-e2e"))]
    updater_e2e_status: Option<PathBuf>,
}

fn startup_arguments() -> StartupArguments {
    let mut files = Vec::new();
    let mut screenshot = None;
    let mut check_only = false;
    #[cfg(all(target_os = "windows", feature = "updater-e2e"))]
    let mut updater_e2e_status = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--open" {
            if let Some(path) = arguments.next() {
                files.push(PathBuf::from(path));
            }
        } else if argument == "--export-plot"
            && let Some(path) = arguments.next()
        {
            screenshot = Some(PathBuf::from(path));
        } else if argument == "--check" {
            check_only = true;
            if let Some(path) = arguments.next() {
                files.push(PathBuf::from(path));
            }
        } else if argument == "--updater-e2e" {
            #[cfg(all(target_os = "windows", feature = "updater-e2e"))]
            if let Some(path) = arguments.next() {
                updater_e2e_status = Some(PathBuf::from(path));
            }
        } else if !argument.to_string_lossy().starts_with('-') {
            files.push(PathBuf::from(argument));
        }
    }
    StartupArguments {
        files,
        screenshot,
        check_only,
        #[cfg(all(target_os = "windows", feature = "updater-e2e"))]
        updater_e2e_status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_application_icon_is_small_and_valid() {
        let icon = application_icon();
        assert_eq!((icon.width, icon.height), (128, 128));
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);
        assert!(
            icon.rgba
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 0)
        );
    }
}
