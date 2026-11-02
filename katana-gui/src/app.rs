//! The main window.

use std::path::{Path, PathBuf};

use eframe::egui;
use katana_proto::Patch;

use crate::editor::{Editor, Origin};
use crate::library::{self, Kind, Library};
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
    /// A patch of a downloaded set; its file is created only after Discard.
    Pick(PathBuf, usize),
    Close,
}

/// Seconds between auto-saves while the editor keeps changing.
pub const AUTOSAVE_SECS: f64 = 300.0;

/// Smallest window the layout fits (the signal chain needs about 1165 px).
pub const MIN_WINDOW: [f32; 2] = [1200.0, 720.0];

pub struct KatanaApp {
    pub editor: Editor,
    pub library: Library,
    pub tab: Tab,
    pub theme: ThemeId,
    /// `layout::BlockDef::id` of the block shown in the detail panel.
    pub selected_block: &'static str,
    /// Last failed action, shown in the status line.
    pub error: Option<String>,
    applied_theme: Option<ThemeId>,
    pending: Option<Pending>,
    /// Library tab: search words and sort order.
    pub search: String,
    pub sort: library::Sort,
    /// A library file waiting for the user to confirm its deletion, with its name.
    pending_delete: Option<(PathBuf, String)>,
    /// The editor's patch and origin and the channel tab as last auto-saved (or as at start).
    autosaved: (Patch, Origin, Option<u8>),
    /// `egui` time of the last timed auto-save.
    autosaved_at: Option<f64>,
    /// Local time of the last auto-save, for the status line.
    pub last_autosave: Option<chrono::NaiveDateTime>,
    /// The Save as… prompt's name while it is open.
    save_as_name: Option<String>,
    /// The user chose to discard changes and close.
    close_confirmed: bool,
    /// The channel tab shown: 0 = PANEL, 1–8 = A1–B4; `None` when none is.
    pub channel_tab: Option<u8>,
}

impl Default for KatanaApp {
    fn default() -> Self {
        Self::new()
    }
}

impl KatanaApp {
    /// An app without a library folder (every library action fails).
    pub fn new() -> KatanaApp {
        KatanaApp::with_library(Library::detached())
    }

    /// Problems found while reading the library go to the status line.
    pub fn with_library(mut library: Library) -> KatanaApp {
        let problems = std::mem::take(&mut library.problems);
        let editor = Editor::new();
        KatanaApp {
            autosaved: (editor.patch().clone(), Origin::None, None),
            autosaved_at: None,
            last_autosave: None,
            channel_tab: None,
            editor,
            library,
            tab: Tab::Editor,
            theme: ThemeId::Dark,
            selected_block: "booster",
            error: (!problems.is_empty()).then(|| problems.join("; ")),
            applied_theme: None,
            pending: None,
            search: String::new(),
            sort: library::Sort::default(),
            pending_delete: None,
            save_as_name: None,
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

    /// `with_library`, then back to the state at the last quit: the newest
    /// auto-save, with the file it came from (or that file as it is now, if
    /// there were no unsaved changes). A missing origin file leaves the patch
    /// without one, and says so.
    pub fn start(library: Library) -> KatanaApp {
        let mut app = KatanaApp::with_library(library);
        let Some((patch, origin, dirty, channel)) = app.library.last_autosave() else {
            app.take_library_problems();
            return app;
        };
        app.channel_tab = channel;
        let (baseline, origin) = match origin.path().map(|p| (p, app.library.load(p))) {
            None => (Patch::default_preset(), origin),
            Some((_, Ok(baseline))) => (baseline, origin),
            Some((path, Err(_))) => {
                app.library.problems.push(format!("{} no longer exists", library::file_name(path)));
                (Patch::default_preset(), Origin::None)
            }
        };
        // Clean at quit: the origin file is the truth, even if it changed since.
        let patch = if dirty || origin == Origin::None { patch } else { baseline.clone() };
        app.editor.restore(patch, baseline, origin);
        app.autosaved = (app.editor.patch().clone(), app.editor.origin().clone(), app.channel_tab);
        app.take_library_problems();
        app
    }

    fn take_library_problems(&mut self) {
        let mut all: Vec<String> = self.error.take().into_iter().collect();
        all.append(&mut self.library.problems);
        self.error = (!all.is_empty()).then(|| all.join("; "));
    }

    /// Auto-saves when `AUTOSAVE_SECS` have passed since the last timed one
    /// (`now_secs` in egui time) and the editor changed since the last auto-save.
    pub fn autosave_tick(&mut self, now_secs: f64, now: chrono::NaiveDateTime) {
        if self.autosaved_at.is_some_and(|t| now_secs - t < AUTOSAVE_SECS) {
            return;
        }
        if self.autosave(now) {
            self.autosaved_at = Some(now_secs);
        }
    }

    /// Auto-saves the editor's state if it changed since the last one; true if written.
    fn autosave(&mut self, now: chrono::NaiveDateTime) -> bool {
        let state = (self.editor.patch().clone(), self.editor.origin().clone(), self.channel_tab);
        if state == self.autosaved || self.library.root().is_none() {
            return false;
        }
        match self.library.autosave(&state.0, &state.1, self.editor.needs_restore_as_is(), state.2, now) {
            Ok(()) => {
                self.autosaved = state;
                self.last_autosave = Some(now);
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    /// Loads a file into the editor. A library file becomes the editor's
    /// origin; a file from elsewhere has none (Save then asks for a name).
    fn open(&mut self, path: &Path) {
        let origin = match self.library.kind_of(path) {
            Some(Kind::Patch) => Origin::Patch(path.to_path_buf()),
            Some(Kind::QuickSave) => Origin::QuickSave(path.to_path_buf()),
            None => Origin::None,
        };
        match self.library.load(path) {
            Ok(patch) => {
                self.editor.load(patch, origin);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Picks patch `index` of a downloaded set into My patches and opens it;
    /// asks first if the editor has unsaved changes.
    pub fn request_pick(&mut self, set: PathBuf, index: usize) {
        if self.editor.is_dirty() {
            self.pending = Some(Pending::Pick(set, index));
        } else {
            self.pick(&set, index);
        }
    }

    fn pick(&mut self, set: &Path, index: usize) {
        match self.library.pick(set, index) {
            Ok((path, patch)) => {
                self.editor.load(patch, Origin::Patch(path));
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Copies a `.syx` or `.tsl` into the library; the editor is untouched.
    pub fn import(&mut self, from: PathBuf) {
        self.error = self.library.import(&from).err();
    }

    /// Writes a copy of the editor's patch to `path` (adding `.syx` if missing).
    pub fn export_to(&mut self, path: PathBuf) {
        // A file without the extension would be hidden by the Import dialog's filter.
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("syx")) {
            path
        } else {
            let mut name = path.into_os_string();
            name.push(".syx");
            PathBuf::from(name)
        };
        self.error = library::write_atomic(&path, &self.editor.patch().to_syx())
            .err()
            .map(|e| format!("Couldn't save {}: {e}", file_name(&path)));
    }

    /// Overwrites the quick save the editor came from, unless it is locked;
    /// otherwise creates a new quick save. Never overwrites a named patch.
    pub fn quick_save(&mut self, now: chrono::NaiveDateTime) {
        let patch = self.editor.patch().clone();
        let result = match self.editor.origin().clone() {
            Origin::QuickSave(path) if !self.library.is_read_only(&path) => {
                self.library.overwrite(&path, &patch).map(|()| path)
            }
            _ => self.library.new_quick_save(&patch, now),
        };
        self.finish_save(result.map(|path| (patch, Origin::QuickSave(path))));
    }

    /// Overwrites the file the editor came from; with no origin, asks for a name.
    pub fn save(&mut self) {
        let origin = self.editor.origin().clone();
        let Some(path) = origin.path() else {
            self.open_save_as();
            return;
        };
        let patch = self.editor.patch().clone();
        let result = self.library.overwrite(path, &patch);
        self.finish_save(result.map(|()| (patch, origin.clone())));
    }

    /// Saves a new named patch in My patches; the name is also written into the patch.
    pub fn save_as(&mut self, name: &str) {
        let mut patch = self.editor.patch().clone();
        patch.set_name(name.trim());
        let result = self.library.save_as(name, &patch);
        self.finish_save(result.map(|path| (patch, Origin::Patch(path))));
    }

    fn finish_save(&mut self, result: Result<(Patch, Origin), String>) {
        match result {
            Ok((patch, origin)) => {
                self.editor.saved_as(patch, origin);
                self.error = None;
                // The newest auto-save is never older than the last save.
                self.autosave(chrono::Local::now().naive_local());
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// False while the file the editor came from is read-only (Save as… still works).
    pub fn can_save(&self) -> bool {
        self.editor.path().is_none_or(|p| !self.library.is_read_only(p))
    }

    /// Opens the Save as… prompt, prefilled with the current name.
    pub fn open_save_as(&mut self) {
        self.save_as_name = Some(self.editor.display_name());
    }

    /// Asks before deleting a patch, quick save or set; `name` is what the user sees.
    pub fn request_delete(&mut self, path: PathBuf, name: String) {
        self.pending_delete = Some((path, name));
    }

    fn delete_prompt(&mut self, ctx: &egui::Context) {
        let Some((path, name)) = self.pending_delete.take() else { return };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::NULL.with("delete")).show(ctx, |ui| {
            ui.label(format!("Delete {name}?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    choice = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(false);
                }
            });
        });
        if choice.is_none() && modal.should_close() {
            choice = Some(false);
        }
        match choice {
            Some(true) => match self.library.delete(&path) {
                Ok(()) => {
                    if self.editor.path() == Some(path.as_path()) {
                        self.editor.forget_origin();
                    }
                    self.error = None;
                }
                Err(e) => self.error = Some(e),
            },
            Some(false) => {}
            None => self.pending_delete = Some((path, name)),
        }
    }

    fn save_as_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut name) = self.save_as_name.take() else { return };
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::NULL.with("save_as")).show(ctx, |ui| {
            ui.label("Save as a new patch in My patches.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = ui.label("Name");
                let edit = ui.add(egui::TextEdit::singleline(&mut name).desired_width(240.0)).labelled_by(label.id);
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
                    choice = Some(true);
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!name.trim().is_empty(), egui::Button::new("Save")).clicked() {
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
        match choice {
            Some(true) => self.save_as(&name),
            Some(false) => {}
            None => self.save_as_name = Some(name),
        }
    }

    /// The window asks to close. true = close now; false = asking first.
    pub fn on_close_requested(&mut self) -> bool {
        if self.close_confirmed || !self.editor.is_dirty() {
            self.autosave(chrono::Local::now().naive_local());
            return true;
        }
        self.pending = Some(Pending::Close);
        false
    }

    fn discard_prompt(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let question = format!("Discard unsaved changes to {}?", self.editor.display_name());
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
            (Some(true), Some(Pending::Pick(set, index))) => self.pick(&set, index),
            (Some(true), Some(Pending::Close)) => {
                // The state at quit is the file the editor came from, not the discarded edits.
                let (baseline, origin) = (self.editor.baseline().clone(), self.editor.origin().clone());
                self.editor.load(baseline, origin);
                self.autosave(chrono::Local::now().naive_local());
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
            Tab::Library => views::library(ui, self, &palette),
        });
        self.discard_prompt(&ctx);
        self.save_as_prompt(&ctx);
        self.delete_prompt(&ctx);
        let now_secs = ctx.input(|i| i.time);
        self.autosave_tick(now_secs, chrono::Local::now().naive_local());
        // Wake up for the next auto-save even when nothing else happens.
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(AUTOSAVE_SECS));
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
