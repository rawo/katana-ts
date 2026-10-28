//! The Library tab (layout C): My patches with its Quick saves group on the
//! left, downloaded Tone Exchange sets on the right.

use std::time::SystemTime;

use eframe::egui::{self, Align, Button, Layout, RichText};

use crate::app::KatanaApp;
use crate::library::{self, Entry, Kind, SetEntry, Sort};
use crate::theme::Palette;

const SORTS: [(Sort, &str); 4] = [
    (Sort::NewestFirst, "Newest first"),
    (Sort::OldestFirst, "Oldest first"),
    (Sort::NameAz, "Name A–Z"),
    (Sort::NameZa, "Name Z–A"),
];

pub fn library(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.horizontal(|ui| {
        let label = ui.label("Search");
        ui.add(egui::TextEdit::singleline(&mut app.search).desired_width(320.0)).labelled_by(label.id);
        ui.add_space(12.0);
        let current = SORTS.iter().find(|(s, _)| *s == app.sort).map_or("", |(_, l)| l);
        egui::ComboBox::from_label("Sort").selected_text(current).show_ui(ui, |ui| {
            for (sort, label) in SORTS {
                ui.selectable_value(&mut app.sort, sort, label);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Import…").clicked() {
                if let Some(path) = super::pick_import_file() {
                    app.import(path);
                }
            }
        });
    });
    ui.add_space(8.0);
    ui.columns(2, |cols| {
        super::section(&mut cols[0], p, |ui| my_patches(ui, app, p));
        super::section(&mut cols[1], p, |ui| sets(ui, app, p));
    });
}

fn heading(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).small().strong().color(p.dim));
    ui.add_space(4.0);
}

fn my_patches(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    heading(ui, p, "MY PATCHES");
    let shown = |kind| {
        let mut v: Vec<Entry> = app
            .library
            .patches()
            .iter()
            .filter(|e| e.kind == kind && library::matches(&app.search, &e.name))
            .cloned()
            .collect();
        library::sort_entries(&mut v, app.sort);
        v
    };
    let (named, quick) = (shown(Kind::Patch), shown(Kind::QuickSave));
    egui::ScrollArea::vertical().id_salt("my_patches").auto_shrink(false).show(ui, |ui| {
        if named.is_empty() && quick.is_empty() {
            let text = if app.library.patches().is_empty() { "No patches yet." } else { "No matches." };
            ui.label(RichText::new(text).color(p.dim));
        }
        for e in &named {
            patch_row(ui, app, p, e);
        }
        if !quick.is_empty() {
            ui.add_space(8.0);
            heading(ui, p, "QUICK SAVES");
            for e in &quick {
                patch_row(ui, app, p, e);
            }
        }
    });
}

fn patch_row(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette, e: &Entry) {
    ui.horizontal(|ui| {
        let current = app.editor.path() == Some(e.path.as_path());
        if ui.selectable_label(current, &e.name).clicked() {
            app.request_open(e.path.clone());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let delete = ui.add_enabled(!e.read_only, Button::new("Delete"));
            named(&delete, egui::Role::Button, !e.read_only, &format!("Delete {}", e.name));
            if delete.on_disabled_hover_text("Read-only: unlock it first").clicked() {
                app.request_delete(e.path.clone(), e.name.clone());
            }
            let mut locked = e.read_only;
            let lock = ui.toggle_value(&mut locked, "read-only");
            lock.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::CheckBox, true, locked, format!("Lock {}", e.name))
            });
            if lock.clicked() {
                if let Err(err) = app.library.set_read_only(&e.path, locked) {
                    app.error = Some(err);
                }
            }
            ui.label(RichText::new(date(e.modified)).small().color(p.dim));
        });
    });
}

fn sets(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    heading(ui, p, "DOWNLOADED SETS");
    let mut sets: Vec<SetEntry> = app.library.sets().to_vec();
    match app.sort {
        Sort::NewestFirst => sets.sort_by_key(|s| std::cmp::Reverse(s.modified)),
        Sort::OldestFirst => sets.sort_by_key(|s| s.modified),
        Sort::NameAz => sets.sort_by_key(|s| s.name.to_lowercase()),
        Sort::NameZa => sets.sort_by_key(|s| std::cmp::Reverse(s.name.to_lowercase())),
    }
    egui::ScrollArea::vertical().id_salt("sets").auto_shrink(false).show(ui, |ui| {
        if sets.is_empty() {
            ui.label(RichText::new("No downloaded sets yet. Import a .tsl from Tone Exchange.").color(p.dim));
        }
        let searching = !app.search.trim().is_empty();
        let mut any = false;
        for set in &sets {
            // A set matches by its name (all patches shown) or by patch names (only those shown).
            let whole = library::matches(&app.search, &set.name);
            let patches: Vec<(usize, &String)> =
                set.patches.iter().enumerate().filter(|(_, n)| whole || library::matches(&app.search, n)).collect();
            if patches.is_empty() {
                continue;
            }
            any = true;
            let header = egui::CollapsingHeader::new(RichText::new(&set.name).color(p.accent)).id_salt(&set.path);
            header.open(searching.then_some(true)).show(ui, |ui| {
                for (i, name) in patches {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let open = ui.button("Open");
                            named(&open, egui::Role::Button, true, &format!("Open {name}"));
                            if open.clicked() {
                                app.request_pick(set.path.clone(), i);
                            }
                        });
                    });
                }
                ui.add_space(4.0);
                let delete = ui.button("Delete set");
                named(&delete, egui::Role::Button, true, &format!("Delete set {}", set.name));
                if delete.clicked() {
                    app.request_delete(set.path.clone(), set.name.clone());
                }
            });
        }
        if !sets.is_empty() && !any {
            ui.label(RichText::new("No matches.").color(p.dim));
        }
    });
}

/// Gives a widget whose visible text is generic ("Delete") a specific accessible name.
fn named(r: &egui::Response, kind: egui::Role, enabled: bool, name: &str) {
    r.widget_info(|| egui::WidgetInfo::labeled(kind, enabled, name));
}

/// "28 Oct 14:02", local time.
fn date(t: SystemTime) -> String {
    chrono::DateTime::<chrono::Local>::from(t).format("%-d %b %H:%M").to_string()
}
