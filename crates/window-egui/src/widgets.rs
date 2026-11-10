//! Rust counterparts of the `@window/ui` Vue components: `Toolbar`, `Badge`, `EmptyState`, and
//! the primary `.vw-btn` variant.

use egui::{
    Align, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Layout, Margin, Pos2, Rect,
    Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, Vec2, ViewportCommand,
    pos2, vec2,
};

use crate::tokens::{self, palette};

const BADGE_H: f32 = 18.0;

/// egui can't centre content it hasn't measured yet, so widgets that centre remember last pass's
/// size under `id` and re-run the frame when the newly measured `used` differs from `last`.
fn remember_size(ui: &Ui, id: Id, used: f32, last: Option<f32>) {
    if last.is_none_or(|l| (l - used).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, used));
        ui.ctx().request_discard("re-centre after measuring");
    }
}

// ---- Toolbar ---------------------------------------------------------------

/// The top bar (`Toolbar.vue`): a 48px `surface` strip with a bottom border and three slots.
///
/// ```ignore
/// window_egui::Toolbar::show(ui, |bar| {
///     bar.start(|ui| { window_egui::file_label(ui, &self.file_name); });
///     bar.center(|ui| { /* zoom controls */ });
///     bar.end(|ui| if ui.button("Open").clicked() { self.open_dialog() });
/// });
/// ```
pub struct Toolbar<'a> {
    ui: &'a mut Ui,
    rect: Rect,
}

impl Toolbar<'_> {
    pub fn show(ui: &mut Ui, add_contents: impl FnOnce(&mut Toolbar<'_>)) {
        let p = palette(ui);
        egui::Panel::top("vw_toolbar")
            .exact_size(tokens::TOOLBAR_H)
            .frame(
                Frame::new()
                    .fill(p.surface)
                    .inner_margin(Margin::symmetric(tokens::SPACE_3 as i8, 0)),
            )
            .show(ui, |ui| {
                let rect = ui.max_rect();
                add_contents(&mut Toolbar { ui, rect });
            });
    }

    /// Left third, laid out left to right.
    pub fn start(&mut self, add_contents: impl FnOnce(&mut Ui)) {
        let r = self.third(0);
        self.slot(r, Layout::left_to_right(Align::Center), add_contents);
    }

    /// Centred in the middle third.
    pub fn center(&mut self, add_contents: impl FnOnce(&mut Ui)) {
        let id = Id::unique("vw_toolbar_center");
        let third = self.third(1);
        let last: Option<f32> = self.ui.data(|d| d.get_temp(id));
        let width = last.unwrap_or(third.width()).min(third.width());
        let r = Rect::from_center_size(third.center(), vec2(width, third.height()));
        let used = self.slot(r, Layout::left_to_right(Align::Center), add_contents);
        remember_size(self.ui, id, used, last);
    }

    /// Right third, laid out right to left (the first widget added is rightmost).
    pub fn end(&mut self, add_contents: impl FnOnce(&mut Ui)) {
        let r = self.third(2);
        self.slot(r, Layout::right_to_left(Align::Center), add_contents);
    }

    fn third(&self, i: usize) -> Rect {
        let w = self.rect.width() / 3.0;
        let x = self.rect.left() + w * i as f32;
        Rect::from_x_y_ranges(x..=x + w, self.rect.y_range())
    }

    /// Lay out a slot in `rect` and return the width it used.
    fn slot(&mut self, rect: Rect, layout: Layout, add_contents: impl FnOnce(&mut Ui)) -> f32 {
        let mut child = self
            .ui
            .new_child(UiBuilder::new().max_rect(rect).layout(layout));
        child.spacing_mut().interact_size.y = tokens::CONTROL_H;
        add_contents(&mut child);
        child.min_rect().width()
    }
}

/// The current file name, muted and truncated like `.file-name` in the Tauri toolbars.
pub fn file_label(ui: &mut Ui, name: &str) -> Response {
    let color = palette(ui).fg_muted;
    ui.scope(|ui| {
        ui.set_max_width(260.0);
        ui.add(egui::Label::new(RichText::new(name).color(color)).truncate())
    })
    .inner
}

/// Small muted note beside the file name, e.g. "unsaved" or "1,204 tris".
pub fn meta_label(ui: &mut Ui, text: impl Into<String>) -> Response {
    let color = palette(ui).fg_subtle;
    ui.label(RichText::new(text).small().color(color))
}

/// A toolbar search box that fills its slot, with the text vertically centred in the
/// `CONTROL_H`-tall field.
pub fn filter_field(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .align(egui::Align2::LEFT_CENTER)
            .desired_width(f32::INFINITY),
    )
}

/// Set the window title to `name — app` (with a `●` when `dirty`), or just `app` with no file.
/// Cheap to call every frame: the title is only sent when it changes.
pub fn set_title(ctx: &egui::Context, app: &str, name: &str, dirty: bool) {
    let title = match (name.is_empty(), dirty) {
        (true, _) => app.to_owned(),
        (false, false) => format!("{name} — {app}"),
        (false, true) => format!("● {name} — {app}"),
    };
    let id = Id::unique("vw_title");
    if ctx.data(|d| d.get_temp::<String>(id)).as_deref() != Some(title.as_str()) {
        ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
        ctx.data_mut(|d| d.insert_temp(id, title));
    }
}

// ---- Buttons ---------------------------------------------------------------

/// `.vw-btn--primary`: an accent-filled button.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let p = palette(ui);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, p.accent_fg);
    let size = vec2(
        galley.size().x + 2.0 * tokens::SPACE_3,
        tokens::CONTROL_H.max(galley.size().y + 8.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if response.is_pointer_button_down_on() {
            p.accent_active
        } else if response.hovered() {
            p.accent_hover
        } else {
            p.accent
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(tokens::RADIUS_SM), fill);
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, p.accent_fg);
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A bordered button that stays highlighted while `on` (`ToolbarButton.vue` with `active`).
/// Flips `on` when clicked.
pub fn toggle_button(ui: &mut Ui, on: &mut bool, text: &str) -> Response {
    let response = ui.add(egui::Button::new(text).selected(*on));
    if response.clicked() {
        *on = !*on;
    }
    response
}

// ---- Badge -----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeVariant {
    Default,
    Warning,
    Danger,
}

/// A small uppercase status pill (`Badge.vue`).
pub fn badge(ui: &mut Ui, text: &str, variant: BadgeVariant) -> Response {
    paint_badge(ui, text, variant, Sense::hover())
}

/// The "outdated" warning badge shown when the open file changes on disk. Returns true when
/// clicked, i.e. the user asked to reload.
pub fn outdated_badge(ui: &mut Ui, discards_edits: bool) -> bool {
    let hint = if discards_edits {
        "File changed on disk — click to reload and discard your edits"
    } else {
        "File changed on disk — click to reload"
    };
    paint_badge(ui, "outdated", BadgeVariant::Warning, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(hint)
        .clicked()
}

fn paint_badge(ui: &mut Ui, text: &str, variant: BadgeVariant, sense: Sense) -> Response {
    let p = palette(ui);
    let (fill, fg, stroke) = match variant {
        BadgeVariant::Default => (p.surface_2, p.fg_muted, p.border),
        BadgeVariant::Warning => (
            p.surface.lerp_to_gamma(p.danger, 0.12),
            p.danger,
            p.surface.lerp_to_gamma(p.danger, 0.35),
        ),
        BadgeVariant::Danger => (p.danger, Color32::WHITE, p.danger),
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_uppercase(), FontId::proportional(tokens::FS_XS), fg);
    let size = vec2(galley.size().x + 2.0 * tokens::SPACE_2, BADGE_H);
    let (rect, response) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let stroke = if sense.senses_click() && response.hovered() {
            fg
        } else {
            stroke
        };
        ui.painter().rect(
            rect,
            CornerRadius::same((BADGE_H / 2.0) as u8),
            fill,
            Stroke::new(1.0, stroke),
            StrokeKind::Inside,
        );
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, fg);
    }
    response
}

// ---- Empty state -----------------------------------------------------------

/// Error text in the danger colour.
pub fn error_text(ui: &mut Ui, text: impl Into<RichText>) -> Response {
    let color = palette(ui).danger;
    ui.colored_label(color, text)
}

/// The centred "open a file" card (`EmptyState.vue`): a dashed outline with a file icon, a
/// title, a hint (replaced by `error` when set) and an "Open File" button. Fills the remaining
/// space of `ui`. Returns true when the button is clicked.
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str, error: Option<&str>) -> bool {
    let p = palette(ui);
    let pad = tokens::SPACE_6;
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());

    // Centre vertically using last pass's measured height (see `remember_size`).
    let id = ui.scope_id().with("vw_empty_state");
    let last: Option<f32> = ui.data(|d| d.get_temp(id));
    let width = 360.0_f32
        .min(area.width() - 2.0 * tokens::SPACE_5)
        .max(160.0);
    let height = last.unwrap_or(240.0);
    let card = Rect::from_center_size(area.center(), vec2(width, height));

    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(card.shrink(pad))
            .layout(Layout::top_down(Align::Center)),
    );
    child.spacing_mut().item_spacing.y = tokens::SPACE_3;
    let (icon_rect, _) = child.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
    paint_file_icon(child.painter(), icon_rect, p.fg_subtle);
    child.label(RichText::new(title).size(tokens::FS_XL).color(p.fg));
    let (text, color) = match error {
        Some(e) => (e, p.danger),
        None => (hint, p.fg_muted),
    };
    child.add(egui::Label::new(RichText::new(text).color(color)).wrap());
    // On top of `item_spacing`, for a 16px gap above the button.
    child.add_space(tokens::SPACE_1);
    let clicked = primary_button(&mut child, "Open File").clicked();

    let used = child.min_rect().height() + 2.0 * pad;
    remember_size(ui, id, used, last);

    let outline = Rect::from_center_size(area.center(), vec2(width, used));
    ui.painter().extend(Shape::dashed_line(
        &rounded_rect_points(outline, tokens::RADIUS_LG as f32),
        Stroke::new(1.5, p.border_strong),
        6.0,
        4.0,
    ));
    clicked
}

/// A closed polyline around `rect` with rounded corners, for dashed outlines.
fn rounded_rect_points(rect: Rect, r: f32) -> Vec<Pos2> {
    use std::f32::consts::FRAC_PI_2;
    const STEPS: usize = 6;
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -FRAC_PI_2),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), FRAC_PI_2),
        (pos2(rect.left() + r, rect.top() + r), 2.0 * FRAC_PI_2),
    ];
    let mut points: Vec<Pos2> = corners
        .iter()
        .flat_map(|&(c, start)| {
            (0..=STEPS).map(move |i| {
                let a = start + FRAC_PI_2 * i as f32 / STEPS as f32;
                c + r * vec2(a.cos(), a.sin())
            })
        })
        .collect();
    points.push(points[0]);
    points
}

/// The outlined document icon from `EmptyState.vue` (a 24×24 path scaled into `rect`).
fn paint_file_icon(painter: &egui::Painter, rect: Rect, color: Color32) {
    let s = rect.width() / 24.0;
    let at = |x: f32, y: f32| rect.min + vec2(x, y) * s;
    let stroke = Stroke::new(1.125 * s, color);
    let page = [
        at(14.25, 2.25),
        at(6.75, 2.25),
        at(5.7, 2.7),
        at(5.25, 3.75),
        at(5.25, 20.25),
        at(5.7, 21.3),
        at(6.75, 21.75),
        at(17.25, 21.75),
        at(18.3, 21.3),
        at(18.75, 20.25),
        at(18.75, 6.75),
        at(14.25, 2.25),
    ];
    painter.add(Shape::line(page.to_vec(), stroke));
    painter.add(Shape::line(
        vec![at(14.25, 2.25), at(14.25, 6.75), at(18.75, 6.75)],
        stroke,
    ));
}
