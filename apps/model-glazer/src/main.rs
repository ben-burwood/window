// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod mesh;
mod render;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // The file to open is passed as the first CLI argument. This is how the OS
    // file association launches us on Windows and Linux (and via `open --args`
    // on macOS). See README for the macOS double-click caveat.
    let initial_file: Option<PathBuf> = std::env::args_os().nth(1).map(PathBuf::from);

    let options = eframe::NativeOptions {
        // Render with the glow (OpenGL) backend; our 3D paint callback needs the
        // shared `glow::Context` that this backend exposes via `cc.gl`.
        renderer: eframe::Renderer::Glow,
        // A depth buffer is required for correct 3D rendering; eframe requests
        // none by default (egui itself is 2D).
        depth_buffer: 24,
        viewport: egui::ViewportBuilder::default()
            .with_title("Model Glazer")
            .with_inner_size([1000.0, 720.0])
            .with_min_inner_size([360.0, 260.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Model Glazer",
        options,
        Box::new(move |cc| Ok(Box::new(app::ModelGlazerApp::new(cc, initial_file.clone())))),
    )
}
