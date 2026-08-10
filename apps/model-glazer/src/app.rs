//! The Model Glazer application: window chrome, file loading + live reload, an
//! orbit/pan/zoom canvas, and a solid/wireframe toggle. 3D geometry is drawn by
//! [`crate::render::Renderer`] through an egui GPU paint callback.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Color32, ViewportCommand};
use egui_glow::glow;

use crate::camera::Camera;
use crate::mesh::{self, Aabb};
use crate::render::{Renderer, Shading, Viewport};

pub struct ModelGlazerApp {
    /// Shared GL context (from eframe's glow backend); `None` if unavailable.
    gl: Option<Arc<glow::Context>>,
    /// GPU renderer, shared with the paint callback. `Err` carries why it is
    /// unavailable (no GL context, or a shader compile/link failure) — a fatal,
    /// whole-session condition shown in place of the canvas.
    renderer: Result<Arc<Mutex<Renderer>>, String>,

    camera: Camera,
    shading: Shading,
    /// Bounds of the loaded model, kept so "Fit" can re-frame it.
    model_aabb: Option<Aabb>,
    /// Triangle count of the loaded model, shown in the toolbar.
    tri_count: usize,

    file_name: String,
    /// File-load error (parse failure / unsupported type).
    error: Option<String>,
    /// Path of the loaded file, used to reload.
    current_path: Option<PathBuf>,
    /// Set after a load so the window title is refreshed once next frame.
    title_dirty: bool,

    /// Set by the file watcher (on its own thread) when the file changes on disk.
    outdated: Arc<AtomicBool>,
    /// Live file watcher; replaced on each load, dropped on exit.
    _watch: Option<window_core::FileWatch>,
    /// Wakes the UI from the watcher thread.
    egui_ctx: egui::Context,
}

impl ModelGlazerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let gl = cc.gl.clone();
        let renderer = match &gl {
            Some(ctx) => Renderer::new(ctx).map(|r| Arc::new(Mutex::new(r))),
            None => Err("No OpenGL context available.".to_string()),
        };

        let mut app = Self {
            gl,
            renderer,
            camera: Camera::default(),
            shading: Shading::Solid,
            model_aabb: None,
            tri_count: 0,
            file_name: String::new(),
            error: None,
            current_path: None,
            title_dirty: true,
            outdated: Arc::new(AtomicBool::new(false)),
            _watch: None,
            egui_ctx: cc.egui_ctx.clone(),
        };
        if let Some(path) = initial_file {
            app.load(&path);
        }
        app
    }

    fn load(&mut self, path: &Path) {
        self.outdated.store(false, Ordering::Relaxed);
        match mesh::load(path) {
            Ok(mesh) => {
                self.file_name = window_core::file_name(path);
                self.tri_count = mesh.triangle_count();
                self.camera.fit(&mesh.aabb);
                self.model_aabb = Some(mesh.aabb);
                if let (Some(gl), Ok(renderer)) = (&self.gl, &self.renderer) {
                    renderer.lock().unwrap().upload(gl, &mesh);
                }
                self.error = None;
                self.current_path = Some(path.to_path_buf());
                self.title_dirty = true;
                self.start_watch(path);
            }
            Err(e) => {
                self.error = Some(e);
                self.model_aabb = None;
                self.tri_count = 0;
                self.file_name = window_core::file_name(path);
                self.title_dirty = true;
                self.current_path = None;
                self._watch = None;
            }
        }
    }

    fn start_watch(&mut self, path: &Path) {
        let outdated = self.outdated.clone();
        let ctx = self.egui_ctx.clone();
        self._watch = window_core::watch_file(path, Duration::from_millis(300), move || {
            outdated.store(true, Ordering::Relaxed);
            ctx.request_repaint();
        })
        .ok();
    }

    fn reload(&mut self) {
        if let Some(path) = self.current_path.clone() {
            self.load(&path);
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("3D model", &["stl", "obj", "3mf"])
            .pick_file()
        {
            self.load(&path);
        }
    }

    fn fit(&mut self) {
        if let Some(aabb) = self.model_aabb {
            self.camera.fit(&aabb);
        }
    }
}

impl eframe::App for ModelGlazerApp {
    // egui 0.35: the app fills a root `Ui`; we nest our own panels inside it.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.title_dirty {
            let title = if self.file_name.is_empty() {
                "Model Glazer".to_owned()
            } else {
                format!("{} — Model Glazer", self.file_name)
            };
            ctx.send_viewport_cmd(ViewportCommand::Title(title));
            self.title_dirty = false;
        }

        self.handle_shortcuts(&ctx);

        egui::Panel::top("toolbar").show(ui, |ui| {
            let row_h = ui.spacing().interact_size.y + 8.0;
            ui.set_min_height(row_h);
            ui.set_max_height(row_h);
            ui.columns(3, |cols| {
                // Left — current file name + an "outdated" reload pill.
                cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if self.file_name.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new("No file open").weak()).truncate(),
                        );
                    } else {
                        ui.add(egui::Label::new(&self.file_name).truncate());
                        if self.model_aabb.is_some() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{} tris", self.tri_count)).weak(),
                                )
                                .truncate(),
                            );
                        }
                    }
                    if self.model_aabb.is_some() && self.outdated.load(Ordering::Relaxed) {
                        let pill = egui::Button::new(
                            egui::RichText::new("outdated")
                                .small()
                                .strong()
                                .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                        )
                        .small()
                        .fill(Color32::from_rgb(0xfe, 0xe2, 0xe2));
                        if ui
                            .add(pill)
                            .on_hover_text("File changed on disk — click to reload")
                            .clicked()
                        {
                            self.reload();
                        }
                    }
                });

                // Center — fit + shading toggle.
                let has_model = self.model_aabb.is_some();
                cols[1].vertical_centered(|ui| {
                    ui.add_enabled_ui(has_model, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .button("Fit")
                                .on_hover_text("Frame the model (F)")
                                .clicked()
                            {
                                self.fit();
                            }
                            ui.separator();
                            ui.selectable_value(&mut self.shading, Shading::Solid, "Solid");
                            ui.selectable_value(&mut self.shading, Shading::Wireframe, "Wireframe");
                        });
                    });
                });

                // Right — open a file.
                cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📂 Open").clicked() {
                        self.open_dialog();
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            // A broken renderer is fatal for the whole session — surface it first,
            // whether or not a model is loaded, so the user isn't invited to open
            // files that can never render.
            if let Err(err) = &self.renderer {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        format!("⚠  3D rendering unavailable: {err}"),
                    );
                });
                return;
            }
            if let Some(err) = &self.error {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::LIGHT_RED, format!("⚠  {err}"));
                });
                return;
            }
            if self.model_aabb.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a 3D model (Ctrl/Cmd+O) — STL, OBJ, or 3MF.");
                });
                return;
            }
            self.show_canvas(ui);
        });
    }

    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let (Some(gl), Ok(renderer)) = (gl, &self.renderer) {
            renderer.lock().unwrap().destroy(gl);
        }
    }
}

impl ModelGlazerApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let open = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if open {
            self.open_dialog();
        }
        if self.model_aabb.is_some() {
            let fit = ctx.input(|i| i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::Num0));
            if fit {
                self.fit();
            }
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) {
        let Ok(renderer) = &self.renderer else {
            return;
        };
        let renderer = renderer.clone();
        let content = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(content, egui::Sense::click_and_drag());

        // --- Input → camera ---------------------------------------------------
        let modifiers = ui.input(|i| i.modifiers);
        let delta = response.drag_delta();
        let pan = response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && modifiers.shift);
        if pan {
            self.camera.pan(delta.x, delta.y, content.height());
        } else if response.dragged_by(egui::PointerButton::Primary) {
            self.camera.orbit(delta.x, delta.y);
        }
        if response.hovered() {
            let (scroll, zoom_delta) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 {
                self.camera.zoom(scroll);
            }
            if zoom_delta != 1.0 {
                self.camera.pinch(zoom_delta);
            }
        }

        // --- Draw -------------------------------------------------------------
        let aspect = content.width() / content.height().max(1.0);
        let view = self.camera.view();
        let mvp = (self.camera.projection(aspect) * view).to_cols_array();
        let mv = view.to_cols_array();
        let shading = self.shading;

        let cb = egui_glow::CallbackFn::new(move |info, painter| {
            let vp = info.viewport_in_pixels();
            let viewport = Viewport {
                left: vp.left_px,
                bottom: vp.from_bottom_px,
                width: vp.width_px,
                height: vp.height_px,
            };
            renderer
                .lock()
                .unwrap()
                .paint(painter.gl(), &mvp, &mv, shading, viewport);
        });
        ui.painter().add(egui::PaintCallback {
            rect: content,
            callback: Arc::new(cb),
        });
    }
}
