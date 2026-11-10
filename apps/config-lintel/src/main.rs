// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod highlight;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // File to open is passed as the first CLI argument.
    // OS file association launches us on Windows and Linux (and via `open --args` on macOS).
    let initial_file: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Config Lintel")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([520.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Config Lintel",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::ConfigLintelApp::new(
                cc,
                initial_file.clone(),
            )))
        }),
    )
}
