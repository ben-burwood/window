use egui::{Color32, RichText, ViewportCommand};
use egui_extras::syntax_highlighting::CodeTheme;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::config::{self, Format, Kind, Node};
use crate::highlight;

/// Which part of a tree row is being edited inline.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Key,
    Value,
}

/// An inline edit in progress in the tree.
struct Editing {
    path: Vec<usize>,
    field: Field,
    buffer: String,
    /// Focus the text field on its first frame only.
    focus: bool,
}

/// Something a tree row asked for, applied once the tree has finished drawing.
enum Action {
    StartEdit(Vec<usize>, Field, String),
    Commit,
    Cancel,
    SetBool(Vec<usize>, bool),
    ChangeType(Vec<usize>, Kind),
    AddChild(Vec<usize>),
    Delete(Vec<usize>),
    Copy(String),
}

/// What to do once the user has dealt with unsaved changes.
#[derive(Clone, Copy)]
enum Pending {
    Open,
    Close,
}

/// The tree filter's matches, recomputed only when the filter text or the tree changes.
struct FilterCache {
    needle: String,
    generation: u64,
    hits: HashSet<Vec<usize>>,
}

pub struct ConfigLintelApp {
    /// The editor's text — the source of truth.
    text: String,
    /// The text as last loaded / saved, to tell whether there are unsaved edits.
    saved_text: String,
    format: Format,
    /// Parse result of `text`, refreshed whenever it changes.
    parse_error: Option<String>,
    /// The last successful parse, still shown (read-only) while the text has an error.
    tree: Vec<Node>,
    /// Bumped whenever `tree` is replaced, to invalidate `filter_cache`.
    generation: u64,
    filter: String,
    filter_cache: Option<FilterCache>,
    editing: Option<Editing>,
    /// An open / close waiting on the "discard unsaved changes?" prompt.
    confirm: Option<Pending>,
    /// Set once the user has agreed to close with unsaved changes.
    allow_close: bool,
    file_name: String,
    /// Load error, shown in place of the editor.
    error: Option<String>,
    /// Status of the last save / tree edit, shown in the toolbar.
    status: Option<String>,
    /// The window title last sent, so it is only re-sent when it changes.
    title: String,
    /// Path of the loaded file, used to save and reload.
    current_path: Option<PathBuf>,
    /// Set by the file watcher (on its own thread) when the file changes on disk.
    outdated: Arc<AtomicBool>,
    /// Live file watcher; replaced on each load, dropped on exit.
    _watch: Option<window_core::FileWatch>,
    /// Wakes the UI from the watcher thread.
    egui_ctx: egui::Context,
}

impl ConfigLintelApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            text: String::new(),
            saved_text: String::new(),
            format: Format::Yaml,
            parse_error: None,
            tree: Vec::new(),
            generation: 0,
            filter: String::new(),
            filter_cache: None,
            editing: None,
            confirm: None,
            allow_close: false,
            file_name: String::new(),
            error: None,
            status: None,
            title: String::new(),
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

    fn dirty(&self) -> bool {
        self.text != self.saved_text
    }

    fn load(&mut self, path: &Path) {
        self.outdated.store(false, Ordering::Relaxed);
        self.file_name = window_core::file_name(path);
        self.status = None;
        self.editing = None;
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.format = Format::from_path(path).unwrap_or_else(|| Format::sniff(&text));
                self.saved_text = text.clone();
                self.text = text;
                self.tree.clear();
                self.reparse();
                self.error = None;
                self.current_path = Some(path.to_path_buf());
                self.start_watch(path);
            }
            Err(e) => {
                self.error = Some(format!("Could not read {}: {e}", self.file_name));
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

    fn reparse(&mut self) {
        match config::parse(&self.text, self.format) {
            Ok(docs) => {
                self.tree = docs;
                self.generation += 1;
                self.parse_error = None;
            }
            Err(e) => self.parse_error = Some(e),
        }
    }

    /// Open another file, first asking what to do with unsaved edits.
    fn request_open(&mut self) {
        if self.dirty() {
            self.confirm = Some(Pending::Open);
        } else {
            self.open_dialog();
        }
    }

    fn proceed(&mut self, pending: Pending) {
        match pending {
            Pending::Open => self.open_dialog(),
            Pending::Close => {
                self.allow_close = true;
                self.egui_ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn open_dialog(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter("Config files", Format::EXTENSIONS)
            .add_filter("All files", &["*"]);
        if let Some(dir) = self.current_path.as_deref().and_then(Path::parent) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.load(&path);
        }
    }

    /// Save to the current file, unless the text is now in a different format than the file's
    /// extension says (after a conversion) — then ask where to put it instead.
    fn save(&mut self) {
        match self.current_path.clone() {
            Some(path) if Format::from_path(&path).is_none_or(|f| f == self.format) => {
                self.write_to(&path)
            }
            _ => self.save_as(),
        }
    }

    fn save_as(&mut self) {
        let stem = self
            .current_path
            .as_deref()
            .and_then(Path::file_stem)
            .map_or("untitled".into(), |s| s.to_string_lossy().into_owned());
        let ext = self.format.extension();
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(format!("{stem}.{ext}"))
            .add_filter(self.format.name(), &[ext]);
        if let Some(dir) = self.current_path.as_deref().and_then(Path::parent) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.save_file() {
            self.write_to(&path);
        }
    }

    fn write_to(&mut self, path: &Path) {
        match std::fs::write(path, &self.text) {
            Ok(()) => {
                self.saved_text = self.text.clone();
                self.status = Some("Saved".into());
                self.file_name = window_core::file_name(path);
                self.current_path = Some(path.to_path_buf());
                // Our own write trips the watcher; that's not an outside change.
                self.start_watch(path);
                self.outdated.store(false, Ordering::Relaxed);
            }
            Err(e) => self.status = Some(format!("Save failed: {e}")),
        }
    }

    /// Rewrite the text from the (valid) tree in another format.
    fn convert(&mut self, to: Format) {
        match config::serialize(&self.tree, to) {
            Ok(text) => {
                self.text = text;
                self.format = to;
                self.reparse();
                self.status = Some(format!("Converted to {}", to.name()));
            }
            Err(e) => self.status = Some(e),
        }
    }

    /// Apply a change to a copy of the tree and regenerate the text from it. If the current
    /// format can't hold the result (e.g. a null in TOML) nothing changes and the reason is shown.
    fn edit_tree(&mut self, change: impl FnOnce(&mut Vec<Node>) -> Result<(), String>) {
        let mut docs = self.tree.clone();
        if let Err(e) = change(&mut docs) {
            self.status = Some(e);
            return;
        }
        if docs == self.tree {
            return;
        }
        match config::serialize(&docs, self.format) {
            Ok(text) => {
                self.text = text;
                self.reparse();
                self.status = None;
            }
            Err(e) => self.status = Some(e),
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::StartEdit(path, field, buffer) => {
                self.editing = Some(Editing {
                    path,
                    field,
                    buffer,
                    focus: true,
                });
            }
            Action::Cancel => self.editing = None,
            Action::Commit => {
                let Some(Editing {
                    path,
                    field,
                    buffer,
                    ..
                }) = self.editing.take()
                else {
                    return;
                };
                self.edit_tree(|docs| match field {
                    Field::Value => {
                        if let Some(node) = node_at(docs, &path) {
                            *node = node.with_text(&buffer);
                        }
                        Ok(())
                    }
                    Field::Key => {
                        let (parent, index) = path.split_at(path.len() - 1);
                        match node_at(docs, parent) {
                            Some(parent) => parent.rename(index[0], buffer),
                            None => Ok(()),
                        }
                    }
                });
            }
            Action::SetBool(path, value) => self.edit_tree(|docs| {
                if let Some(node) = node_at(docs, &path) {
                    *node = Node::Bool(value);
                }
                Ok(())
            }),
            Action::ChangeType(path, kind) => self.edit_tree(|docs| {
                if let Some(node) = node_at(docs, &path) {
                    *node = node.converted(kind);
                }
                Ok(())
            }),
            Action::AddChild(path) => {
                self.edit_tree(|docs| {
                    if let Some(node) = node_at(docs, &path) {
                        node.push_child();
                    }
                    Ok(())
                });
                // Open the container so the new child is visible.
                let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    &self.egui_ctx,
                    node_id(&path),
                    true,
                );
                state.set_open(true);
                state.store(&self.egui_ctx);
            }
            Action::Delete(path) => self.edit_tree(|docs| {
                let (parent, index) = path.split_at(path.len() - 1);
                if let Some(parent) = node_at(docs, parent) {
                    parent.remove(index[0]);
                }
                Ok(())
            }),
            Action::Copy(text) => {
                self.egui_ctx.copy_text(text);
                self.status = Some("Copied".into());
            }
        }
    }
}

/// The node at `path` (document index, then child indices).
fn node_at<'a>(docs: &'a mut [Node], path: &[usize]) -> Option<&'a mut Node> {
    let (&doc, rest) = path.split_first()?;
    docs.get_mut(doc)?.at_mut(rest)
}

fn node_id(path: &[usize]) -> egui::Id {
    egui::Id::unique(("config-node", path))
}

impl eframe::App for ConfigLintelApp {
    // Runs even when the window is hidden / minimized and `ui` is skipped, so a close request
    // (e.g. from the taskbar) can't slip past the unsaved-changes prompt.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() && !self.allow_close {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let title = if self.file_name.is_empty() {
            "Config Lintel".to_owned()
        } else {
            let mark = if self.dirty() { "● " } else { "" };
            format!("{mark}{} — Config Lintel", self.file_name)
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        self.handle_shortcuts(&ctx);
        self.show_confirm(&ctx);
        self.show_toolbar(ui);
        self.show_status_bar(ui);

        if let Some(err) = &self.error {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(Color32::LIGHT_RED, format!("⚠  {err}"));
                });
            });
            return;
        }

        egui::Panel::left("tree")
            .resizable(true)
            .default_size(420.0)
            .size_range(220.0..=1200.0)
            .show(ui, |ui| self.show_tree(ui));

        egui::CentralPanel::default().show(ui, |ui| self.show_editor(ui));
    }
}

impl ConfigLintelApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let (open, save, save_as) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let s = i.key_pressed(egui::Key::S);
            (
                cmd && i.key_pressed(egui::Key::O),
                cmd && !i.modifiers.shift && s,
                cmd && i.modifiers.shift && s,
            )
        });
        if open {
            self.request_open();
        }
        if save && self.dirty() {
            self.save();
        }
        if save_as {
            self.save_as();
        }
    }

    /// The "discard unsaved changes?" prompt before an open / close.
    fn show_confirm(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.confirm else { return };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::unique("confirm_discard")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Unsaved changes");
            ui.add_space(6.0);
            let name = if self.file_name.is_empty() {
                "this document"
            } else {
                &self.file_name
            };
            ui.label(format!("Save your changes to {name} first?"));
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save").clicked() {
                    choice = Some("save");
                }
                if ui.button("Discard").clicked() {
                    choice = Some("discard");
                }
                if ui.button("Cancel").clicked() {
                    choice = Some("cancel");
                }
            });
        });

        match choice {
            Some("save") => {
                self.confirm = None;
                self.save();
                // Only carry on if the save went through (Save As can be cancelled).
                if !self.dirty() {
                    self.proceed(pending);
                }
            }
            Some("discard") => {
                self.confirm = None;
                self.proceed(pending);
            }
            Some(_) => self.confirm = None,
            None if modal.should_close() => self.confirm = None,
            None => {}
        }
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            let row_h = ui.spacing().interact_size.y + 8.0;
            ui.set_min_height(row_h);
            ui.set_max_height(row_h);
            ui.columns(3, |cols| {
                // Left — current file name, unsaved / outdated markers.
                cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if self.file_name.is_empty() {
                        ui.add(egui::Label::new(RichText::new("Untitled").weak()).truncate());
                    } else {
                        ui.add(egui::Label::new(&self.file_name).truncate());
                    }
                    if self.dirty() {
                        ui.label(RichText::new("unsaved").small().weak());
                    }
                    if self.current_path.is_some() && self.outdated.load(Ordering::Relaxed) {
                        let pill = egui::Button::new(
                            RichText::new("outdated")
                                .small()
                                .strong()
                                .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                        )
                        .small()
                        .fill(Color32::from_rgb(0xfe, 0xe2, 0xe2));
                        let hint = if self.dirty() {
                            "File changed on disk — click to reload and discard your edits"
                        } else {
                            "File changed on disk — click to reload"
                        };
                        if ui.add(pill).on_hover_text(hint).clicked() {
                            self.reload();
                        }
                    }
                });

                // Center — tree filter.
                cols[1].vertical_centered(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("🔍 Filter keys and values")
                            .desired_width(f32::INFINITY),
                    );
                });

                // Right — open, save, convert, format.
                cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📂 Open").clicked() {
                        self.request_open();
                    }
                    if ui
                        .add_enabled(self.dirty(), egui::Button::new("💾 Save"))
                        .on_hover_text("Ctrl/Cmd+S · Shift for Save As")
                        .clicked()
                    {
                        self.save();
                    }

                    let valid = self.parse_error.is_none();
                    ui.add_enabled_ui(valid, |ui| {
                        ui.menu_button("⇄ Convert", |ui| {
                            for f in Format::ALL {
                                if f != self.format && ui.button(f.name()).clicked() {
                                    self.convert(f);
                                }
                            }
                        })
                        .response
                        .on_hover_text("Rewrite the document in another format")
                        .on_disabled_hover_text("Fix the parse error first");
                    });

                    // Which parser reads the text (doesn't change the text).
                    let before = self.format;
                    egui::ComboBox::from_id_salt("format")
                        .selected_text(self.format.name())
                        .width(64.0)
                        .show_ui(ui, |ui| {
                            for f in Format::ALL {
                                ui.selectable_value(&mut self.format, f, f.name());
                            }
                        })
                        .response
                        .on_hover_text("Parse the text as…");
                    if self.format != before {
                        self.reparse();
                    }
                });
            });
        });
    }

    fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                match &self.parse_error {
                    Some(e) => {
                        // Parser messages can span lines (TOML quotes the source); show the first.
                        let first = e.lines().next().unwrap_or_default();
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("⚠ {first}")).color(Color32::LIGHT_RED),
                            )
                            .truncate(),
                        )
                        .on_hover_text(RichText::new(e).monospace());
                    }
                    None => {
                        let docs = match self.tree.len() {
                            1 => String::new(),
                            n => format!(" · {n} documents"),
                        };
                        ui.label(
                            RichText::new(format!("✔ Valid {}{docs}", self.format.name())).weak(),
                        );
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(status) = &self.status {
                        ui.label(RichText::new(status).small().weak());
                    }
                });
            });
        });
    }

    fn show_editor(&mut self, ui: &mut egui::Ui) {
        let theme = CodeTheme::from_memory(ui.ctx(), ui.style());
        let format = self.format;
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let mut job = highlight::highlight(ui.ctx(), ui.style(), &theme, text.as_str(), format);
            job.wrap.max_width = wrap_width;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };

        let was_empty = self.text.is_empty();
        let changed = egui::ScrollArea::both()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .hint_text(
                            "Paste or type YAML, TOML, JSON or XML — or open a file (Ctrl/Cmd+O)",
                        )
                        .layouter(&mut layouter),
                )
                .changed()
            })
            .inner;

        if changed {
            // Pasting into an empty, unsaved buffer: guess what it is.
            if was_empty && self.current_path.is_none() {
                self.format = Format::sniff(&self.text);
            }
            self.editing = None;
            self.status = None;
            self.reparse();
        }
    }

    fn show_tree(&mut self, ui: &mut egui::Ui) {
        let editable = self.parse_error.is_none();
        ui.horizontal(|ui| {
            ui.strong("Tree");
            let note = if editable {
                "double-click to edit · right-click for more"
            } else {
                "showing last valid parse"
            };
            ui.label(RichText::new(note).small().weak()).on_hover_text(
                "Edits here regenerate the source text, so comments and formatting are not kept.",
            );
        });
        ui.separator();

        if self.tree.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Nothing to show yet.").weak());
            });
            return;
        }

        let needle = self.filter.trim().to_lowercase();
        if needle.is_empty() {
            self.filter_cache = None;
        } else if self
            .filter_cache
            .as_ref()
            .is_none_or(|c| c.needle != needle || c.generation != self.generation)
        {
            self.filter_cache = Some(FilterCache {
                hits: config::matching_paths(&self.tree, &needle),
                needle,
                generation: self.generation,
            });
        }

        let docs = &self.tree;
        let mut view = TreeView {
            docs,
            editing: &mut self.editing,
            hits: self.filter_cache.as_ref().map(|c| &c.hits),
            editable,
            format: self.format,
            actions: Vec::new(),
        };
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            if !editable {
                ui.disable();
            }
            if docs.len() == 1 {
                view.children(ui, &docs[0], &mut vec![0]);
            } else {
                for (i, doc) in docs.iter().enumerate() {
                    view.node(ui, Label::Document(i), doc, &mut vec![i]);
                }
            }
        });

        for action in std::mem::take(&mut view.actions) {
            self.apply(action);
        }
    }
}

/// How a tree row is labelled.
#[derive(Clone, Copy)]
enum Label<'a> {
    Key(&'a str),
    Index(usize),
    Document(usize),
}

/// Draws the tree, collecting what the user did into `actions`.
struct TreeView<'a> {
    docs: &'a [Node],
    editing: &'a mut Option<Editing>,
    /// Paths the filter keeps, or `None` when there is no filter.
    hits: Option<&'a HashSet<Vec<usize>>>,
    editable: bool,
    format: Format,
    actions: Vec<Action>,
}

impl TreeView<'_> {
    fn visible(&self, path: &[usize]) -> bool {
        self.hits.is_none_or(|hits| hits.contains(path))
    }

    fn children(&mut self, ui: &mut egui::Ui, node: &Node, path: &mut Vec<usize>) {
        match node.untagged() {
            Node::Seq(items) => {
                for (i, child) in items.iter().enumerate() {
                    path.push(i);
                    self.node(ui, Label::Index(i), child, path);
                    path.pop();
                }
            }
            Node::Map(entries) => {
                for (i, (key, child)) in entries.iter().enumerate() {
                    path.push(i);
                    self.node(ui, Label::Key(key), child, path);
                    path.pop();
                }
            }
            scalar => {
                // A document that is just a scalar.
                self.row(ui, Label::Document(path[0]), scalar, path);
            }
        }
    }

    fn node(&mut self, ui: &mut egui::Ui, label: Label, node: &Node, path: &mut Vec<usize>) {
        if !self.visible(path) {
            return;
        }
        if !node.is_container() {
            self.row(ui, label, node, path);
            return;
        }

        let id = node_id(path);
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            path.len() <= 2,
        );
        if self.hits.is_some() {
            state.set_open(true);
        }
        state
            .show_header(ui, |ui| {
                self.label(ui, label, node, path);
                let summary = match node.untagged() {
                    Node::Seq(_) => format!("[{}]", node.len()),
                    _ => format!("{{{}}}", node.len()),
                };
                let tag = match node {
                    Node::Tagged(tag, _) => format!("{tag} "),
                    _ => String::new(),
                };
                ui.label(RichText::new(format!("{tag}{summary}")).weak().monospace());
            })
            .body(|ui| self.children(ui, node, path));
    }

    /// A scalar: `key  value`.
    fn row(&mut self, ui: &mut egui::Ui, label: Label, node: &Node, path: &[usize]) {
        ui.horizontal(|ui| {
            // Line up with container rows, which have a collapse arrow.
            ui.add_space(ui.spacing().indent);
            self.label(ui, label, node, path);
            self.value(ui, node, path);
        });
    }

    /// The key / index part of a row: renamable for table entries.
    fn label(&mut self, ui: &mut egui::Ui, label: Label, node: &Node, path: &[usize]) {
        if let Label::Key(_) = label
            && self.edit_field(ui, path, Field::Key)
        {
            return;
        }
        let text = match label {
            Label::Key(k) => RichText::new(k).strong(),
            Label::Index(i) => RichText::new(format!("{i}")).weak().monospace(),
            Label::Document(i) => RichText::new(format!("Document {}", i + 1)).strong(),
        };
        let response = ui
            .add(egui::Label::new(text).sense(egui::Sense::click()))
            .on_hover_text(config::path_string(self.docs, path));
        if let Label::Key(k) = label
            && self.editable
            && response.double_clicked()
        {
            self.actions
                .push(Action::StartEdit(path.to_vec(), Field::Key, k.to_owned()));
        }
        self.context_menu(&response, label, node, path);
    }

    fn value(&mut self, ui: &mut egui::Ui, node: &Node, path: &[usize]) {
        if self.edit_field(ui, path, Field::Value) {
            return;
        }
        if let Node::Bool(b) = node {
            let mut v = *b;
            let response = ui.checkbox(&mut v, RichText::new(b.to_string()).monospace());
            if response.changed() {
                self.actions.push(Action::SetBool(path.to_vec(), v));
            }
            self.context_menu(&response, Label::Index(0), node, path);
            return;
        }

        let text = RichText::new(node.display())
            .monospace()
            .color(value_color(ui, node));
        let response = ui
            .add(
                egui::Label::new(text)
                    .truncate()
                    .sense(egui::Sense::click()),
            )
            .on_hover_text(node.kind_name());
        if self.editable && response.double_clicked() {
            self.actions
                .push(Action::StartEdit(path.to_vec(), Field::Value, node.text()));
        }
        self.context_menu(&response, Label::Index(0), node, path);
    }

    /// If `path`'s `field` is being edited, draw the text field and return true.
    fn edit_field(&mut self, ui: &mut egui::Ui, path: &[usize], field: Field) -> bool {
        let Some(edit) = self.editing.as_mut() else {
            return false;
        };
        if edit.path != path || edit.field != field {
            return false;
        }
        let response = ui.add(
            egui::TextEdit::singleline(&mut edit.buffer)
                .desired_width(220.0)
                .font(egui::TextStyle::Monospace),
        );
        if edit.focus {
            response.request_focus();
            edit.focus = false;
        }
        if response.lost_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.actions.push(Action::Cancel);
            } else {
                self.actions.push(Action::Commit);
            }
        }
        true
    }

    fn context_menu(
        &mut self,
        response: &egui::Response,
        label: Label,
        node: &Node,
        path: &[usize],
    ) {
        let is_doc_root = path.len() == 1;
        response.context_menu(|ui| {
            if self.editable {
                if let Label::Key(k) = label
                    && ui.button("✏ Rename").clicked()
                {
                    self.actions
                        .push(Action::StartEdit(path.to_vec(), Field::Key, k.to_owned()));
                }
                if !node.is_container() && ui.button("✏ Edit value").clicked() {
                    self.actions
                        .push(Action::StartEdit(path.to_vec(), Field::Value, node.text()));
                }
                if node.is_container() && ui.button("➕ Add child").clicked() {
                    self.actions.push(Action::AddChild(path.to_vec()));
                }
                ui.menu_button("Change type", |ui| {
                    for kind in Kind::ALL {
                        let current = node.kind() == kind;
                        if ui
                            .add_enabled(!current, egui::Button::new(kind.name()))
                            .clicked()
                        {
                            self.actions.push(Action::ChangeType(path.to_vec(), kind));
                        }
                    }
                });
                ui.separator();
            }
            if !is_doc_root && ui.button("📋 Copy path").clicked() {
                self.actions
                    .push(Action::Copy(config::path_string(self.docs, path)));
            }
            if ui.button("📋 Copy value").clicked() {
                self.actions
                    .push(Action::Copy(copy_value(node, self.format)));
            }
            if !is_doc_root && self.editable {
                ui.separator();
                if ui.button("🗑 Delete").clicked() {
                    self.actions.push(Action::Delete(path.to_vec()));
                }
            }
        });
    }
}

/// Scalars copy as their plain text; containers as a snippet in the current format, falling back
/// to JSON where the format can't hold a bare fragment (e.g. an array in TOML).
fn copy_value(node: &Node, format: Format) -> String {
    if !node.is_container() {
        return node.text();
    }
    let docs = std::slice::from_ref(node);
    config::serialize(docs, format)
        .or_else(|_| config::serialize(docs, Format::Json))
        .unwrap_or_default()
}

fn value_color(ui: &egui::Ui, node: &Node) -> Color32 {
    let dark = ui.visuals().dark_mode;
    match node.untagged() {
        Node::String(_) => {
            if dark {
                Color32::from_rgb(0x9c, 0xd6, 0x8a)
            } else {
                Color32::from_rgb(0x2e, 0x7d, 0x32)
            }
        }
        Node::Number(_) | Node::Datetime(_) => {
            if dark {
                Color32::from_rgb(0x8a, 0xb4, 0xf8)
            } else {
                Color32::from_rgb(0x15, 0x65, 0xc0)
            }
        }
        _ => {
            if dark {
                Color32::from_rgb(0xd0, 0x9c, 0xf0)
            } else {
                Color32::from_rgb(0x7b, 0x1f, 0xa2)
            }
        }
    }
}
