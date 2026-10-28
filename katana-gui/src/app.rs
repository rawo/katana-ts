//! The main window.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::editor::Editor;
use crate::theme::{self, ThemeId};
use crate::views;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Editor,
    Library,
}

/// An action waiting for the user to confirm discarding unsaved changes.
enum Pending {
    Open(PathBuf),
    Close,
}

/// Smallest window the layout fits (the signal chain needs about 1165 px).
pub const MIN_WINDOW: [f32; 2] = [1200.0, 720.0];

pub struct KatanaApp {
    pub editor: Editor,
    pub tab: Tab,
    pub theme: ThemeId,
    /// `layout::BlockDef::id` of the block shown in the detail panel.
    pub selected_block: &'static str,
    /// Last failed action, shown in the status line.
    pub error: Option<String>,
    applied_theme: Option<ThemeId>,
    pending: Option<Pending>,
    /// The user chose to discard changes and close.
    close_confirmed: bool,
}

impl Default for KatanaApp {
    fn default() -> Self {
        Self::new()
    }
}

impl KatanaApp {
    pub fn new() -> KatanaApp {
        KatanaApp {
            editor: Editor::new(),
            tab: Tab::Editor,
            theme: ThemeId::Dark,
            selected_block: "booster",
            error: None,
            applied_theme: None,
            pending: None,
            close_confirmed: false,
        }
    }

    /// Opens now if the editor is clean; otherwise asks first.
    pub fn request_open(&mut self, path: PathBuf) {
        if self.editor.is_dirty() {
            self.pending = Some(Pending::Open(path));
        } else {
            self.open(&path);
        }
    }

    fn open(&mut self, path: &Path) {
        let result = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| self.editor.open_bytes(&bytes, path.to_path_buf()).map_err(|e| e.to_string()));
        self.error = result.err().map(|e| format!("Couldn't open {}: {e}", file_name(path)));
    }

    /// Writes the editor to `path` (adding `.syx` if missing); it stays dirty if the write fails.
    pub fn save_to(&mut self, path: PathBuf) {
        // A file without the extension would be hidden by the Open dialog's filter.
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("syx")) {
            path
        } else {
            let mut name = path.into_os_string();
            name.push(".syx");
            PathBuf::from(name)
        };
        match crate::library::write_atomic(&path, &self.editor.patch().to_syx()) {
            Ok(()) => {
                self.editor.saved_to(path);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Couldn't save {}: {e}", file_name(&path))),
        }
    }

    /// The window asks to close. true = close now; false = asking first.
    pub fn on_close_requested(&mut self) -> bool {
        if self.close_confirmed || !self.editor.is_dirty() {
            return true;
        }
        self.pending = Some(Pending::Close);
        false
    }

    fn discard_prompt(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let question = format!("Discard unsaved changes to {}?", self.editor.name());
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::NULL.with("discard")).show(ctx, |ui| {
            ui.label(question);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Discard").clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        // Escape or a click outside the prompt means Cancel.
        if choice.is_none() && modal.should_close() {
            choice = Some(false);
        }
        match (choice, self.pending.take()) {
            (Some(true), Some(Pending::Open(path))) => self.open(&path),
            (Some(true), Some(Pending::Close)) => {
                self.close_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            (Some(_), _) => {}
            (None, pending) => self.pending = pending,
        }
    }

    /// The whole UI, drawn into the window's root `Ui`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.on_close_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if self.applied_theme != Some(self.theme) {
            theme::apply(ui.ctx(), &self.theme.palette());
            self.applied_theme = Some(self.theme);
        }
        let palette = self.theme.palette();
        egui::Panel::top("top_bar").show(ui, |ui| views::top_bar(ui, self, &palette));
        egui::Panel::top("tabs").show(ui, |ui| views::action_bar(ui, self, &palette));
        egui::Panel::bottom("status").show(ui, |ui| views::status(ui, self, &palette));
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Editor => views::editor_tab(ui, self, &palette),
            Tab::Library => views::library(ui),
        });
        self.discard_prompt(&ctx);
        crate::widgets::track_page_wheel(&ctx);
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// `<name>.syx`, with anything but letters, digits, `-` and `_` turned into `_`.
pub fn default_file_name(name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    format!("{safe}.syx")
}

impl eframe::App for KatanaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
