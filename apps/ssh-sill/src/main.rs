// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod known_hosts;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // optional file to open is passed as the first CLI argument; without one the app opens ~/.ssh/known_hosts.
    let initial_file: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SSH Sill")
            .with_inner_size([1100.0, 640.0])
            .with_min_inner_size([480.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SSH Sill",
        options,
        Box::new(move |cc| Ok(Box::new(app::SshSillApp::new(cc, initial_file.clone())))),
    )
}
