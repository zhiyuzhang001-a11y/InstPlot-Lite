#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod data;
mod data_export;
mod edit_history;
mod fitting;
mod fonts;
mod image_export;
mod native_theme;
mod processing;

use app::InstPlotLiteApp;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let arguments = startup_arguments();
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
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 780.0])
            .with_min_inner_size([720.0, 580.0]),
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

struct StartupArguments {
    files: Vec<PathBuf>,
    screenshot: Option<PathBuf>,
    check_only: bool,
}

fn startup_arguments() -> StartupArguments {
    let mut files = Vec::new();
    let mut screenshot = None;
    let mut check_only = false;
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
        } else if !argument.to_string_lossy().starts_with('-') {
            files.push(PathBuf::from(argument));
        }
    }
    StartupArguments {
        files,
        screenshot,
        check_only,
    }
}
