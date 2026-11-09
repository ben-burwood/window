use egui::{Color32, ViewportCommand};
use egui_extras::{Column, TableBuilder};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::known_hosts::{self, Entry, KnownHosts, Marker};

const KEY_TYPES: &[&str] = &[
    "ssh-ed25519",
    "ecdsa-sha2-nistp256",
    "ecdsa-sha2-nistp384",
    "ecdsa-sha2-nistp521",
    "ssh-rsa",
    "sk-ssh-ed25519@openssh.com",
    "sk-ecdsa-sha2-nistp256@openssh.com",
];

/// State of the "Add host" dialog.
#[derive(Default)]
struct AddForm {
    /// A full line (e.g. from `ssh-keyscan`) that fills the fields below.
    paste: String,
    hosts: String,
    key_type: String,
    key: String,
    comment: String,
    marker: Option<Marker>,
    error: Option<String>,
}

impl AddForm {
    fn new() -> Self {
        Self {
            key_type: KEY_TYPES[0].to_owned(),
            ..Default::default()
        }
    }

    fn fill_from_paste(&mut self) {
        if let Some(e) = Entry::parse(self.paste.trim()) {
            self.hosts = e.hosts.join(",");
            self.key_type = e.key_type;
            self.key = e.key;
            self.comment = e.comment;
            self.marker = e.marker;
            self.error = None;
        }
    }
}

pub struct SshSillApp {
    hosts: Option<KnownHosts>,
    file_name: String,
    error: Option<String>,
    /// Status of the last save, shown in the toolbar.
    status: Option<String>,
    filter: String,
    /// True when there are unsaved edits.
    dirty: bool,
    add: Option<AddForm>,
    /// Set after a load / edit so the window title is refreshed once next frame.
    title_dirty: bool,
    /// Path of the loaded file, used to save and reload.
    current_path: Option<PathBuf>,
    /// Set by the file watcher (on its own thread) when the file changes on disk.
    outdated: Arc<AtomicBool>,
    /// Live file watcher; replaced on each load, dropped on exit.
    _watch: Option<window_core::FileWatch>,
    /// Wakes the UI from the watcher thread.
    egui_ctx: egui::Context,
}

impl SshSillApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            hosts: None,
            file_name: String::new(),
            error: None,
            status: None,
            filter: String::new(),
            dirty: false,
            add: None,
            title_dirty: true,
            current_path: None,
            outdated: Arc::new(AtomicBool::new(false)),
            _watch: None,
            egui_ctx: cc.egui_ctx.clone(),
        };
        // With no argument, open the user's own known_hosts if it exists.
        let path = initial_file.or_else(|| known_hosts::default_path().filter(|p| p.is_file()));
        if let Some(path) = path {
            app.load(&path);
        }
        app
    }

    fn load(&mut self, path: &Path) {
        self.outdated.store(false, Ordering::Relaxed);
        self.file_name = window_core::file_name(path);
        self.title_dirty = true;
        self.dirty = false;
        self.status = None;
        match KnownHosts::load(path) {
            Ok(hosts) => {
                self.hosts = Some(hosts);
                self.error = None;
                self.current_path = Some(path.to_path_buf());
                self.start_watch(path);
            }
            Err(e) => {
                self.error = Some(e);
                self.hosts = None;
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
        let mut dialog = rfd::FileDialog::new();
        if let Some(dir) =
            known_hosts::default_path().and_then(|p| p.parent().map(Path::to_path_buf))
        {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.load(&path);
        }
    }

    fn save(&mut self) {
        let (Some(hosts), Some(path)) = (&self.hosts, &self.current_path) else {
            return;
        };
        match hosts.save(path) {
            Ok(()) => {
                self.dirty = false;
                self.title_dirty = true;
                self.status = Some("Saved".into());
                // Our own write trips the watcher; that's not an outside change.
                self.start_watch(&path.clone());
                self.outdated.store(false, Ordering::Relaxed);
            }
            Err(e) => self.status = Some(e),
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.title_dirty = true;
        self.status = None;
    }
}

impl eframe::App for SshSillApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.title_dirty {
            let title = if self.file_name.is_empty() {
                "SSH Sill".to_owned()
            } else {
                let mark = if self.dirty { "● " } else { "" };
                format!("{mark}{} — SSH Sill", self.file_name)
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
                // Left — current file name, unsaved / outdated markers.
                cols[0].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if self.file_name.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new("No file open").weak()).truncate(),
                        );
                    } else {
                        ui.add(egui::Label::new(&self.file_name).truncate());
                    }
                    if self.dirty {
                        ui.label(egui::RichText::new("unsaved").small().weak());
                    } else if let Some(status) = &self.status {
                        ui.label(egui::RichText::new(status).small().weak());
                    }
                    if self.hosts.is_some() && self.outdated.load(Ordering::Relaxed) {
                        let pill = egui::Button::new(
                            egui::RichText::new("outdated")
                                .small()
                                .strong()
                                .color(Color32::from_rgb(0xb9, 0x1c, 0x1c)),
                        )
                        .small()
                        .fill(Color32::from_rgb(0xfe, 0xe2, 0xe2));
                        let hint = if self.dirty {
                            "File changed on disk — click to reload and discard your edits"
                        } else {
                            "File changed on disk — click to reload"
                        };
                        if ui.add(pill).on_hover_text(hint).clicked() {
                            self.reload();
                        }
                    }
                });

                // Center — filter.
                cols[1].vertical_centered(|ui| {
                    ui.add_enabled(
                        self.hosts.is_some(),
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("🔍 Filter hosts, types, comments")
                            .desired_width(f32::INFINITY),
                    );
                });

                // Right — open, save, add.
                cols[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📂 Open").clicked() {
                        self.open_dialog();
                    }
                    let loaded = self.hosts.is_some();
                    if ui
                        .add_enabled(loaded && self.dirty, egui::Button::new("💾 Save"))
                        .clicked()
                    {
                        self.save();
                    }
                    if ui
                        .add_enabled(loaded, egui::Button::new("➕ Add"))
                        .clicked()
                    {
                        self.add = Some(AddForm::new());
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
            if self.hosts.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label("Open a known_hosts file (Ctrl/Cmd+O) to get started.");
                });
                return;
            }
            self.show_table(ui);
        });

        self.show_add_dialog(&ctx);
    }
}

impl SshSillApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // rfd opens a native modal, so run it outside the input closure.
        let (open, save) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(egui::Key::O),
                i.modifiers.command && i.key_pressed(egui::Key::S),
            )
        });
        if open {
            self.open_dialog();
        }
        if save && self.dirty {
            self.save();
        }
    }

    fn show_table(&mut self, ui: &mut egui::Ui) {
        let Some(hosts) = &self.hosts else { return };
        let needle = self.filter.to_lowercase();
        let rows: Vec<(usize, &Entry)> = hosts
            .entries()
            .filter(|(_, e)| {
                needle.is_empty()
                    || e.hosts.iter().any(|h| h.to_lowercase().contains(&needle))
                    || e.key_type.to_lowercase().contains(&needle)
                    || e.comment.to_lowercase().contains(&needle)
            })
            .collect();

        if rows.is_empty() {
            ui.centered_and_justified(|ui| {
                let msg = if hosts.entries().next().is_none() {
                    "No hosts yet — click ➕ Add."
                } else {
                    "No hosts match the filter."
                };
                ui.label(egui::RichText::new(msg).weak());
            });
            return;
        }

        let mut remove: Option<usize> = None;
        let row_h = ui.spacing().interact_size.y + 4.0;
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(280.0).at_least(120.0).clip(true))
            .column(Column::initial(170.0).at_least(80.0).clip(true))
            .column(Column::initial(260.0).at_least(120.0).clip(true))
            .column(Column::remainder().at_least(60.0).clip(true))
            .column(Column::exact(32.0))
            .header(row_h, |mut header| {
                for title in ["Host", "Key type", "Fingerprint", "Comment", ""] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let (index, e) = rows[row.index()];
                    row.col(|ui| {
                        if let Some(m) = e.marker {
                            ui.label(egui::RichText::new(m.as_str()).small().strong());
                        }
                        let hosts = e.hosts_display();
                        let label = if e.is_hashed() {
                            egui::RichText::new(hosts).weak()
                        } else {
                            egui::RichText::new(hosts)
                        };
                        ui.add(egui::Label::new(label).truncate())
                            .on_hover_text(e.hosts.join("\n"));
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(&e.key_type).truncate());
                    });
                    row.col(|ui| {
                        let fp = e.fingerprint().unwrap_or_else(|| "(invalid key)".into());
                        ui.add(egui::Label::new(egui::RichText::new(fp).monospace()).truncate())
                            .on_hover_text("SHA256 fingerprint (as shown by ssh-keygen -lf)");
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(&e.comment).truncate());
                    });
                    row.col(|ui| {
                        if ui
                            .small_button("🗑")
                            .on_hover_text("Remove this host")
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                });
            });

        if let Some(index) = remove {
            if let Some(hosts) = &mut self.hosts {
                hosts.remove(index);
            }
            self.mark_dirty();
        }
    }

    fn show_add_dialog(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.add else { return };
        let mut submit = false;
        let mut cancel = false;

        let modal = egui::Modal::new(egui::Id::unique("add_host")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading("Add host");
            ui.add_space(6.0);

            ui.label(
                egui::RichText::new("Paste a line (e.g. from ssh-keyscan) to fill the fields")
                    .small()
                    .weak(),
            );
            if ui
                .add(
                    egui::TextEdit::singleline(&mut form.paste)
                        .hint_text("host ssh-ed25519 AAAA… comment")
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                form.fill_from_paste();
            }
            ui.separator();

            egui::Grid::new("add_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Hosts");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.hosts)
                            .hint_text("example.com, [10.0.0.1]:2222")
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();

                    ui.label("Key type");
                    egui::ComboBox::from_id_salt("key_type")
                        .selected_text(&form.key_type)
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for t in KEY_TYPES {
                                ui.selectable_value(&mut form.key_type, (*t).to_owned(), *t);
                            }
                        });
                    ui.end_row();

                    ui.label("Key");
                    ui.add(
                        egui::TextEdit::multiline(&mut form.key)
                            .hint_text("AAAAC3NzaC1lZDI1NTE5…")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();

                    ui.label("Comment");
                    ui.add(
                        egui::TextEdit::singleline(&mut form.comment)
                            .hint_text("optional")
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();

                    ui.label("Marker");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut form.marker, None, "None");
                        ui.selectable_value(
                            &mut form.marker,
                            Some(Marker::CertAuthority),
                            "@cert-authority",
                        );
                        ui.selectable_value(&mut form.marker, Some(Marker::Revoked), "@revoked");
                    });
                    ui.end_row();
                });

            if let Some(err) = &form.error {
                ui.add_space(4.0);
                ui.colored_label(Color32::LIGHT_RED, format!("⚠  {err}"));
            }

            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Add").clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

        if submit {
            match Entry::new(
                &form.hosts,
                &form.key_type,
                &form.key,
                &form.comment,
                form.marker,
            ) {
                Ok(entry) => {
                    if let Some(hosts) = &mut self.hosts {
                        hosts.push(entry);
                    }
                    self.add = None;
                    self.mark_dirty();
                }
                Err(e) => form.error = Some(e),
            }
        } else if cancel || modal.should_close() {
            self.add = None;
        }
    }
}
