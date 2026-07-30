//! The Image Shutter application: window chrome, input handling (zoom / pan /
//! fit), transparency checkerboard and on-demand SVG rasterization.

use egui::{Color32, Rect, Sense, TextureHandle, TextureOptions, Vec2, ViewportCommand};
use std::path::{Path, PathBuf};

use crate::svg::{self, SvgImage};

const MIN_ZOOM: f32 = 0.02;
const MAX_ZOOM: f32 = 64.0;
/// Re-rasterize the texture only when the required scale drifts outside this
/// ratio band relative to the current texture, so we don't re-render every
/// frame while zooming.
const RERENDER_LO: f32 = 0.8;
const RERENDER_HI: f32 = 1.25;
const CHECKER_SIZE: f32 = 12.0;

pub struct ImageShutterApp {
    svg: Option<SvgImage>,
    texture: Option<TextureHandle>,
    /// The scale at which `texture` was last rasterized.
    tex_scale: f32,
    file_name: String,
    error: Option<String>,
    /// Display zoom, where 1.0 == the SVG's intrinsic pixel size.
    zoom: f32,
    /// Pan offset (screen points) relative to a centered image.
    offset: Vec2,
    /// Fit-to-window is deferred to the first frame after a load, when the
    /// panel size is known.
    fit_pending: bool,
    /// Set after a load so the window title is refreshed once next frame.
    title_dirty: bool,
}

impl ImageShutterApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            svg: None,
            texture: None,
            tex_scale: 0.0,
            file_name: String::new(),
            error: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            fit_pending: false,
            title_dirty: true,
        };
        if let Some(path) = initial_file {
            app.load(&path);
        }
        app
    }

    fn load(&mut self, path: &Path) {
        match svg::load(path) {
            Ok(img) => {
                self.file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.svg = Some(img);
                self.texture = None;
                self.tex_scale = 0.0;
                self.error = None;
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
                self.fit_pending = true;
                self.title_dirty = true;
            }
            Err(e) => {
                self.error = Some(e);
                self.svg = None;
                self.texture = None;
                self.file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.title_dirty = true;
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG image", &["svg", "svgz"])
            .pick_file()
        {
            self.load(&path);
        }
    }

    /// Scale at which we want the current texture rasterized: display zoom times
    /// the device pixel ratio, so 1 texel maps to ~1 physical pixel.
    fn desired_scale(&self, ppp: f32) -> f32 {
        self.zoom * ppp
    }

    fn ensure_texture(&mut self, ctx: &egui::Context) {
        let Some(img) = &self.svg else { return };
        let desired = self.desired_scale(ctx.pixels_per_point());
        let needs_render = match &self.texture {
            None => true,
            Some(_) => {
                let ratio = desired / self.tex_scale;
                !(RERENDER_LO..=RERENDER_HI).contains(&ratio)
            }
        };
        if needs_render {
            let color_image = svg::render(img, desired);
            self.tex_scale = desired;
            self.texture = Some(ctx.load_texture(
                "svg",
                color_image,
                TextureOptions::LINEAR,
            ));
        }
    }
}

impl eframe::App for ImageShutterApp {
    // egui 0.35: the app fills a root `Ui`; we nest our own panels inside it.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.title_dirty {
            let title = if self.file_name.is_empty() {
                "Image Shutter".to_owned()
            } else {
                format!("{} — Image Shutter", self.file_name)
            };
            ctx.send_viewport_cmd(ViewportCommand::Title(title));
            self.title_dirty = false;
        }

        self.handle_shortcuts(&ctx);

        egui::Panel::top("toolbar").show(ui, |ui| {
            // Pin the bar to a single row. Without this, `columns` hands each
            // column the panel's full remaining height and the vertically
            // centered content stretches the toolbar down the window.
            let row_h = ui.spacing().interact_size.y + 8.0;
            ui.set_min_height(row_h);
            ui.set_max_height(row_h);
            // Three sections, mirroring doc-viewer: filename (left),
            // zoom controls (center), Open button (right).
            ui.columns(3, |cols| {
                // Left — current file name.
                cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if self.file_name.is_empty() {
                        ui.add(egui::Label::new(egui::RichText::new("No file open").weak()).truncate());
                    } else {
                        ui.add(egui::Label::new(&self.file_name).truncate());
                    }
                });

                // Center — zoom controls. Clicking the percentage resets to 100%.
                let has_image = self.svg.is_some();
                cols[1].vertical_centered(|ui| {
                    ui.add_enabled_ui(has_image, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Fit").clicked() {
                                self.fit_pending = true;
                            }
                            if ui.button("−").clicked() {
                                self.zoom = (self.zoom / 1.25).clamp(MIN_ZOOM, MAX_ZOOM);
                            }
                            if ui
                                .button(format!("{:.0}%", self.zoom * 100.0))
                                .on_hover_text("Reset to 100%")
                                .clicked()
                            {
                                self.zoom = 1.0;
                                self.offset = Vec2::ZERO;
                            }
                            if ui.button("+").clicked() {
                                self.zoom = (self.zoom * 1.25).clamp(MIN_ZOOM, MAX_ZOOM);
                            }
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
            if let Some(err) = &self.error {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::LIGHT_RED, format!("⚠  {err}"));
                });
                return;
            }
            if self.svg.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open an SVG (Ctrl/Cmd+O) to get started.");
                });
                return;
            }
            self.show_canvas(ui, &ctx);
        });
    }
}

impl ImageShutterApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let open = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
        if open {
            self.open_dialog();
        }
        if self.svg.is_some() {
            let (reset, fit, zoom_in, zoom_out) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Num0),
                    i.key_pressed(egui::Key::F),
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                )
            });
            if reset {
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
            }
            if fit {
                self.fit_pending = true;
            }
            if zoom_in {
                self.zoom = (self.zoom * 1.25).clamp(MIN_ZOOM, MAX_ZOOM);
            }
            if zoom_out {
                self.zoom = (self.zoom / 1.25).clamp(MIN_ZOOM, MAX_ZOOM);
            }
        }
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let img = self.svg.as_ref().unwrap();
        let base = Vec2::new(img.width as f32, img.height as f32);

        let content = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(content, Sense::click_and_drag());

        // Fit-to-window once the panel size is known.
        if self.fit_pending {
            let sx = content.width() / base.x;
            let sy = content.height() / base.y;
            self.zoom = (sx.min(sy) * 0.98).clamp(MIN_ZOOM, MAX_ZOOM);
            self.offset = Vec2::ZERO;
            self.fit_pending = false;
        }

        let center = content.center();
        let size = base * self.zoom;
        let img_min = center - size * 0.5 + self.offset;

        // Zoom to cursor. Plain mouse-wheel scroll zooms; Ctrl+scroll and
        // trackpad pinch come through as `zoom_delta` (and zero the scroll, so
        // the two never double-apply).
        let (scroll, zoom_delta) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (scroll * 0.0015).exp() * zoom_delta;
        if factor != 1.0 {
            if let Some(cursor) = response.hover_pos() {
                let world = (cursor - img_min) / self.zoom; // point in intrinsic px
                self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
                let new_min = cursor - world * self.zoom;
                self.offset = new_min - center + base * self.zoom * 0.5;
            }
        }

        // Pan on drag.
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // Recompute after input, then (re)build the texture at the right scale.
        let size = base * self.zoom;
        let img_min = content.center() - size * 0.5 + self.offset;
        let img_rect = Rect::from_min_size(img_min, size);

        self.ensure_texture(ctx);

        let painter = ui.painter_at(content);
        paint_checkerboard(&painter, content, img_rect);
        if let Some(tex) = &self.texture {
            painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

/// Draw a light/dark checkerboard behind the image so transparent regions are
/// obvious. Only the intersection of the image and the visible panel is drawn.
fn paint_checkerboard(painter: &egui::Painter, clip: Rect, img_rect: Rect) {
    let area = clip.intersect(img_rect);
    if area.width() <= 0.0 || area.height() <= 0.0 {
        return;
    }
    let light = Color32::from_gray(235);
    let dark = Color32::from_gray(200);

    let x0 = (area.min.x / CHECKER_SIZE).floor() as i64;
    let y0 = (area.min.y / CHECKER_SIZE).floor() as i64;
    let x1 = (area.max.x / CHECKER_SIZE).ceil() as i64;
    let y1 = (area.max.y / CHECKER_SIZE).ceil() as i64;

    for ty in y0..y1 {
        for tx in x0..x1 {
            let color = if (tx + ty) % 2 == 0 { light } else { dark };
            let tile = Rect::from_min_size(
                egui::pos2(tx as f32 * CHECKER_SIZE, ty as f32 * CHECKER_SIZE),
                Vec2::splat(CHECKER_SIZE),
            );
            let tile = tile.intersect(area);
            if tile.width() > 0.0 && tile.height() > 0.0 {
                painter.rect_filled(tile, 0.0, color);
            }
        }
    }
}
