//! The main window.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use katana_proto::{device, encode_param, Address, Channel, ParamDef, Patch, ProtoError, PARAMS};

use crate::amp::{AmpLink, Event, Job};
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

/// The Store to channel… prompt: the chosen channel and, once read, what it holds.
struct StoreDialog {
    channel: Option<Channel>,
    held: Option<Patch>,
}

/// Channel names by selector.
pub const CHANNEL_NAMES: [&str; 9] = ["PANEL", "A1", "A2", "A3", "A4", "B1", "B2", "B3", "B4"];

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
    amp: Option<AmpLink>,
    pub connected: bool,
    /// The channel the amp is on (a selector).
    pub amp_on: Option<u8>,
    /// Live edit is on; kept across a disconnect, off at every start.
    pub live: bool,
    /// What the amp worker is busy with.
    pub busy: Option<String>,
    /// The last change made on the amp's panel: what, and when.
    pub last_panel_change: Option<(String, chrono::NaiveDateTime)>,
    /// Set by the signal handler: save and close without asking.
    pub quit_signal: Arc<AtomicBool>,
    store_dialog: Option<StoreDialog>,
    /// The name of the patch being stored, for the message afterwards.
    storing: Option<String>,
    /// What the last store did, for the status line.
    pub store_note: Option<String>,
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
            amp: None,
            connected: false,
            amp_on: None,
            live: false,
            busy: None,
            last_panel_change: None,
            quit_signal: Arc::default(),
            store_dialog: None,
            storing: None,
            store_note: None,
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

    pub fn with_amp(mut self, link: AmpLink) -> KatanaApp {
        self.amp = Some(link);
        self
    }

    /// Ends the amp worker, which turns editor mode off.
    pub fn shutdown_amp(&mut self) {
        if let Some(link) = self.amp.take() {
            link.shutdown();
        }
    }

    fn send(&self, job: Job) {
        if let Some(link) = &self.amp {
            link.send(job);
        }
    }

    /// Handles everything the amp worker reported since the last frame.
    pub fn poll_amp(&mut self, now: chrono::NaiveDateTime) {
        let Some(link) = &self.amp else { return };
        let events: Vec<Event> = std::iter::from_fn(|| link.try_recv()).collect();
        for event in events {
            self.on_amp_event(event, now);
        }
    }

    fn on_amp_event(&mut self, event: Event, now: chrono::NaiveDateTime) {
        match event {
            Event::Connected => {
                self.connected = true;
                if self.live {
                    self.quick_save_if_dirty(now);
                    self.send(Job::LiveOn);
                }
            }
            Event::Disconnected => {
                self.connected = false;
                self.busy = None;
                self.amp_on = None;
                self.store_dialog = None;
                self.storing = None;
            }
            Event::AmpOn(sel) => {
                self.amp_on = Some(sel);
                if self.live {
                    self.channel_tab = Some(sel);
                }
            }
            Event::Slot(sel, p) => {
                self.show_amp_settings(sel, p, now);
                self.channel_tab = Some(sel);
            }
            Event::Current(sel, p) if self.live => {
                self.show_amp_settings(sel, p, now);
                self.channel_tab = Some(sel);
                self.amp_on = Some(sel);
            }
            Event::PanelChange { addr, data } if self.live => {
                self.editor.apply_amp_change(addr, &data);
                let what = PARAMS
                    .iter()
                    .find(|p| p.addr == addr)
                    .map_or_else(|| Address(addr).to_string(), |p| p.name.to_string());
                self.last_panel_change = Some((what, now));
            }
            Event::Peeked(ch, p) => {
                if let Some(d) = self.store_dialog.as_mut().filter(|d| d.channel == Some(ch)) {
                    d.held = Some(p);
                }
            }
            Event::Stored { ch, solo_delay_stored, .. } => {
                let name = self.storing.take().unwrap_or_default();
                let solo = if solo_delay_stored { "" } else { " · solo delay not stored" };
                self.store_note = Some(format!("stored '{name}' in {ch}; the amp is now on {ch}{solo}"));
                self.channel_tab = Some(ch.selector());
                self.amp_on = Some(ch.selector());
                self.error = None;
            }
            Event::StoreFailed(message) => {
                self.storing = None;
                self.error = Some(message);
            }
            Event::Failed(message) => {
                // A channel that couldn't be read can be chosen again.
                if let Some(d) = self.store_dialog.as_mut().filter(|d| d.held.is_none()) {
                    d.channel = None;
                }
                self.error = Some(message);
            }
            Event::Busy(what) => self.busy = what,
            Event::Current(..) | Event::PanelChange { .. } => {}
        }
    }

    /// The amp's settings take over the editor; unsaved changes are kept in a
    /// quick save first, and the channel's copy is kept for viewing without the amp.
    fn show_amp_settings(&mut self, sel: u8, p: Patch, now: chrono::NaiveDateTime) {
        self.quick_save_if_dirty(now);
        if self.library.root().is_some() {
            if let Err(e) = self.library.save_channel_copy(sel, &p) {
                self.error = Some(e);
            }
        }
        self.editor.replace_from_amp(p);
    }

    fn quick_save_if_dirty(&mut self, now: chrono::NaiveDateTime) {
        if self.editor.is_dirty() {
            self.quick_save(now);
        }
    }

    /// An edit from the UI: into the editor and, with Live on, to the amp.
    pub fn edit(&mut self, def: &ParamDef, value: u32) -> Result<(), ProtoError> {
        self.editor.set(def, value)?;
        if self.live && self.connected {
            self.send(Job::SetParam { addr: def.addr, data: encode_param(def, value)? });
        }
        Ok(())
    }

    pub fn set_live(&mut self, on: bool) {
        self.live = on;
        if on {
            self.quick_save_if_dirty(chrono::Local::now().naive_local());
            self.send(Job::LiveOn);
        } else {
            self.send(Job::LiveOff);
        }
    }

    /// Whether a channel tab can be clicked now.
    pub fn channel_enabled(&self, selector: u8) -> bool {
        if selector == 0 && self.live && !device::PANEL_SELECTABLE {
            return false;
        }
        self.connected || self.library.has_channel_copy(selector)
    }

    /// A channel tab: with Live on the amp switches, with Live off the channel
    /// is only shown; without the amp, its last seen copy is shown.
    pub fn click_channel(&mut self, selector: u8) {
        let now = chrono::Local::now().naive_local();
        if self.connected {
            self.quick_save_if_dirty(now);
            self.send(if self.live { Job::Select(selector) } else { Job::ReadSlot(selector) });
        } else {
            let Some(copy) = self.library.channel_copy(selector) else { return };
            self.quick_save_if_dirty(now);
            self.editor.replace_from_amp(copy);
        }
        self.channel_tab = Some(selector);
    }

    /// Why Store to channel… can't be used now, if so.
    pub fn store_blocked(&self) -> Option<&'static str> {
        if !self.connected {
            Some("Amp not connected")
        } else if self.busy.is_some() {
            Some("The amp is busy")
        } else if self.library.root().is_none() {
            Some("No library folder for the backups")
        } else {
            None
        }
    }

    /// Opens the Store to channel… prompt, on the channel tab when it is A1–B4.
    pub fn open_store_dialog(&mut self) {
        let channel = self.channel_tab.filter(|&s| s > 0).and_then(|s| Channel::ALL.get(usize::from(s) - 1)).copied();
        if let Some(ch) = channel {
            self.send(Job::Peek(ch));
        }
        self.store_note = None;
        self.store_dialog = Some(StoreDialog { channel, held: None });
    }

    /// The user confirmed: the only way anything is stored into a channel.
    fn store(&mut self, ch: Channel) {
        let Some(root) = self.library.root() else { return };
        let job = Job::Store {
            ch,
            patch: self.editor.patch().clone(),
            backup_dir: root.join("backups"),
            stamp: chrono::Local::now().naive_local().format("%Y-%m-%d_%H-%M-%S").to_string(),
        };
        self.storing = Some(self.editor.patch().name());
        self.send(job);
    }

    fn store_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.store_dialog.take() else { return };
        let new_name = self.editor.patch().name();
        let mut chosen = None;
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::NULL.with("store")).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let caption = ui.label("Channel");
                let shown = dialog.channel.map_or_else(|| "Choose".to_string(), |c| c.to_string());
                let combo = egui::ComboBox::from_id_salt("store_channel").selected_text(shown).show_ui(ui, |ui| {
                    for ch in Channel::ALL {
                        if ui.selectable_label(dialog.channel == Some(ch), ch.to_string()).clicked() {
                            chosen = Some(ch);
                        }
                    }
                });
                let _ = combo.response.labelled_by(caption.id);
            });
            ui.add_space(8.0);
            match (dialog.channel, &dialog.held) {
                (Some(ch), Some(old)) => {
                    ui.label(format!("{ch} currently '{}' -> replace with '{new_name}'?", old.name()));
                }
                (Some(ch), None) => {
                    ui.label(format!("Reading {ch}\u{2026}"));
                }
                (None, _) => {}
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if dialog.held.is_some() && ui.button("Store").clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        // Escape or a click outside the prompt means Cancel.
        if answer.is_none() && modal.should_close() {
            answer = Some(false);
        }
        match (answer, dialog.channel) {
            (Some(true), Some(ch)) => self.store(ch),
            (Some(_), _) => {}
            (None, _) => {
                if let Some(ch) = chosen.filter(|&c| dialog.channel != Some(c)) {
                    dialog = StoreDialog { channel: Some(ch), held: None };
                    self.send(Job::Peek(ch));
                }
                self.store_dialog = Some(dialog);
            }
        }
    }

    /// Plays the editor's patch on the amp's current settings. Nothing is stored
    /// and the amp stays on its channel.
    pub fn load_to_amp(&mut self) {
        self.send(Job::WritePatch(self.editor.patch().clone()));
    }

    /// A file was opened into the editor: with Live on it plays; with Live off
    /// the editor no longer shows a channel.
    fn after_open(&mut self) {
        if self.live && self.connected {
            self.load_to_amp();
        } else if !self.live {
            self.channel_tab = None;
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
                self.after_open();
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
                self.after_open();
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
        self.clear_tab_after_save();
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
        self.clear_tab_after_save();
    }

    /// With Live off, a saved editor no longer shows a channel's stored settings.
    fn clear_tab_after_save(&mut self) {
        if !self.live && self.error.is_none() {
            self.channel_tab = None;
        }
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
        self.poll_amp(chrono::Local::now().naive_local());
        if self.quit_signal.load(Ordering::SeqCst) {
            // Nobody may be there to answer a prompt; the edits are in the auto-save.
            self.autosave(chrono::Local::now().naive_local());
            self.close_confirmed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
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
        self.store_prompt(&ctx);
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

    fn on_exit(&mut self) {
        self.shutdown_amp();
    }
}
