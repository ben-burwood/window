// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod svg;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // The file to open is passed as the first CLI argument. This is how the OS
    // file association launches us on Windows and Linux (and via `open --args`
    // on macOS). See README for the macOS double-click caveat.
    let initial_file: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Image Shutter")
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([360.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Image Shutter",
        options,
        Box::new(move |cc| Ok(Box::new(app::ImageShutterApp::new(cc, initial_file.clone())))),
    )
}
