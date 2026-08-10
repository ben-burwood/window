//! The Drawing Paner application: window chrome, input handling (zoom / pan /
//! fit), a light/dark background toggle, and direct vector rendering of a DXF's
//! primitives with an `egui::Painter`.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2, ViewportCommand};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::dxf::{self, Bounds, Primitive};

/// Screen pixels per world unit. Fit picks a value inside this band; the zoom
/// controls keep it there.
const MIN_ZOOM: f32 = 1e-6;
const MAX_ZOOM: f32 = 1e7;
/// Multiplicative step for the discrete zoom controls (buttons / keys).
const ZOOM_STEP: f32 = 1.25;
/// Text below this on-screen height is skipped (unreadable), above it clamped.
const MIN_TEXT_PX: f32 = 4.0;
const MAX_TEXT_PX: f32 = 4000.0;

pub struct DrawingPanerApp {
    drawing: Option<dxf::Drawing>,
    file_name: String,
    error: Option<String>,
    /// Screen pixels per world unit.
    zoom: f32,
    /// Pan offset (screen points) relative to a drawing centered in the panel.
    offset: Vec2,
    /// Fit-to-window is deferred to the first frame after a load, when the
    /// panel size is known.
    fit_pending: bool,
    /// Set after a load so the window title is refreshed once next frame.
    title_dirty: bool,
    /// When true, paint a dark background (CAD-style); otherwise light.
    dark_bg: bool,
    /// Path of the loaded file, used to reload.
    current_path: Option<PathBuf>,
    /// Set by the file watcher (on its own thread) when the file changes on disk.
    outdated: Arc<AtomicBool>,
    /// Live file watcher; replaced on each load, dropped on exit.
    _watch: Option<window_core::FileWatch>,
    /// Wakes the UI from the watcher thread.
    egui_ctx: egui::Context,
}

impl DrawingPanerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            drawing: None,
            file_name: String::new(),
            error: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_pending: false,
            title_dirty: true,
            dark_bg: false,
            current_path: None,
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
        self.file_name = file_name(path);
        self.title_dirty = true;
        match dxf::load(path) {
            Ok(drawing) => {
                self.drawing = Some(drawing);
                self.error = None;
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
                self.fit_pending = true;
                self.current_path = Some(path.to_path_buf());
                self.start_watch(path);
            }
            Err(e) => {
                self.error = Some(e);
                self.drawing = None;
                self.current_path = None;
                self._watch = None;
            }
        }
    }

    fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
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
            .add_filter("DXF drawing", &["dxf"])
            .pick_file()
        {
            self.load(&path);
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl eframe::App for DrawingPanerApp {
    // egui 0.35: the app fills a root `Ui`; we nest our own panels inside it.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.title_dirty {
            let title = if self.file_name.is_empty() {
                "Drawing Paner".to_owned()
            } else {
                format!("{} — Drawing Paner", self.file_name)
            };
            ctx.send_viewport_cmd(ViewportCommand::Title(title));
            self.title_dirty = false;
        }

        self.handle_shortcuts(&ctx);

        egui::Panel::top("toolbar").show(ui, |ui| {
            // Pin the bar to a single row (see image-shutter for the rationale).
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
                    }
                    if self.drawing.is_some() && self.outdated.load(Ordering::Relaxed) {
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

                // Center — zoom controls. Clicking the percentage fits the drawing.
                let has_drawing = self.drawing.is_some();
                cols[1].vertical_centered(|ui| {
                    ui.add_enabled_ui(has_drawing, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Fit").clicked() {
                                self.fit_pending = true;
                            }
                            if ui.button("−").clicked() {
                                self.zoom_by(1.0 / ZOOM_STEP);
                            }
                            if ui
                                .button(format!("{:.0}%", self.zoom * 100.0))
                                .on_hover_text("Fit to window")
                                .clicked()
                            {
                                self.fit_pending = true;
                            }
                            if ui.button("+").clicked() {
                                self.zoom_by(ZOOM_STEP);
                            }
                        });
                    });
                });

                // Right — open a file and toggle the background.
                cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📂 Open").clicked() {
                        self.open_dialog();
                    }
                    ui.toggle_value(&mut self.dark_bg, "🌙 Dark")
                        .on_hover_text("Use a dark background");
                });
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(err) = &self.error {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::LIGHT_RED, format!("⚠  {err}"));
                });
                return;
            }
            if self.drawing.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a DXF (Ctrl/Cmd+O) to get started.");
                });
                return;
            }
            self.show_canvas(ui, &ctx);
        });
    }
}

impl DrawingPanerApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let open = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if open {
            self.open_dialog();
        }
        if self.drawing.is_some() {
            let (fit, zoom_in, zoom_out) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::Num0),
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                )
            });
            if fit {
                self.fit_pending = true;
            }
            if zoom_in {
                self.zoom_by(ZOOM_STEP);
            }
            if zoom_out {
                self.zoom_by(1.0 / ZOOM_STEP);
            }
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let content = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(content, Sense::click_and_drag());

        let bg = if self.dark_bg {
            Color32::from_gray(24)
        } else {
            Color32::from_gray(250)
        };
        let fg = if self.dark_bg {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        let painter = ui.painter_at(content);
        painter.rect_filled(content, 0.0, bg);

        let drawing = self.drawing.as_ref().unwrap();
        let Some(bounds) = drawing.bounds else {
            painter.text(
                content.center(),
                Align2::CENTER_CENTER,
                "No drawable geometry in this file.",
                FontId::proportional(16.0),
                fg,
            );
            return;
        };

        // Fit-to-window once the panel size is known.
        if self.fit_pending {
            self.zoom = fit_zoom(&bounds, content.size());
            self.offset = Vec2::ZERO;
            self.fit_pending = false;
        }

        let pc = content.center();

        // Zoom to cursor. Plain mouse-wheel scroll zooms; Ctrl+scroll and
        // trackpad pinch come through as `zoom_delta`.
        let (scroll, zoom_delta) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (scroll * 0.0015).exp() * zoom_delta;
        if factor != 1.0 {
            if let Some(cursor) = response.hover_pos() {
                let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
                let ratio = new_zoom / self.zoom;
                // Keep the world point under the cursor fixed.
                self.offset -= (cursor - pc - self.offset) * (ratio - 1.0);
                self.zoom = new_zoom;
            }
        }

        // Pan on drag.
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // World (Y-up) -> screen (Y-down), centered on the drawing's bounds.
        let z = self.zoom as f64;
        let wc = bounds.center();
        let ox = (pc.x + self.offset.x) as f64;
        let oy = (pc.y + self.offset.y) as f64;
        let w2s = |p: [f64; 2]| -> Pos2 {
            Pos2::new(
                (ox + (p[0] - wc[0]) * z) as f32,
                (oy - (p[1] - wc[1]) * z) as f32,
            )
        };

        for prim in &drawing.prims {
            match prim {
                Primitive::Polyline { pts, closed, color } => {
                    let mut sp: Vec<Pos2> = pts.iter().map(|&p| w2s(p)).collect();
                    if *closed && sp.len() >= 2 {
                        sp.push(sp[0]);
                    }
                    if polyline_visible(&sp, content) {
                        painter.add(egui::Shape::line(sp, Stroke::new(1.0, pick(*color, fg))));
                    }
                }
                Primitive::Circle { center, r, color } => {
                    let c = w2s(*center);
                    let rr = (*r * z) as f32;
                    if rr >= 0.05
                        && content.intersects(Rect::from_center_size(c, Vec2::splat(rr * 2.0)))
                    {
                        painter.circle_stroke(c, rr, Stroke::new(1.0, pick(*color, fg)));
                    }
                }
                Primitive::Text {
                    pos,
                    height,
                    angle_deg,
                    content: text,
                    color,
                } => {
                    let size = ((*height * z) as f32).min(MAX_TEXT_PX);
                    let p = w2s(*pos);
                    if size >= MIN_TEXT_PX && content.expand(size * 4.0).contains(p) {
                        let col = pick(*color, fg);
                        let galley =
                            painter.layout_no_wrap(text.clone(), FontId::proportional(size), col);
                        // DXF text sits above its baseline; nudge the top-left
                        // anchor up so it reads at the insertion point. World is
                        // Y-up and CCW-positive, which is clockwise on screen —
                        // exactly TextShape's rotation convention.
                        let anchor = Pos2::new(p.x, p.y - galley.size().y);
                        let mut shape = egui::epaint::TextShape::new(anchor, galley, col);
                        shape.angle = (*angle_deg as f32).to_radians();
                        painter.add(shape);
                    }
                }
            }
        }
    }
}

/// Resolve a primitive color, falling back to the foreground color.
fn pick(color: dxf::Color, fg: Color32) -> Color32 {
    match color {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => fg,
    }
}

/// Zoom (px/world-unit) that fits `bounds` inside `panel` with a small margin.
fn fit_zoom(bounds: &Bounds, panel: Vec2) -> f32 {
    let w = bounds.width();
    let h = bounds.height();
    let sx = if w > 1e-9 {
        panel.x as f64 / w
    } else {
        f64::INFINITY
    };
    let sy = if h > 1e-9 {
        panel.y as f64 / h
    } else {
        f64::INFINITY
    };
    let z = sx.min(sy);
    if z.is_finite() && z > 0.0 {
        (z as f32 * 0.98).clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        // Degenerate (single point / empty extent): keep a sane default.
        1.0
    }
}

/// True if the polyline's screen bounding box intersects the visible panel.
fn polyline_visible(pts: &[Pos2], clip: Rect) -> bool {
    pts.len() >= 2 && clip.intersects(Rect::from_points(pts))
}
