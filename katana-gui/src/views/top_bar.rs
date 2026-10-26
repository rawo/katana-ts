use eframe::egui::{self, Align, Button, Layout, RichText};

use super::{NEEDS_AMP, NEEDS_LIBRARY};
use crate::app::KatanaApp;
use crate::theme::Palette;

const CHANNELS: [&str; 9] = ["PANEL", "A1", "A2", "A3", "A4", "B1", "B2", "B3", "B4"];

pub fn top_bar(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.horizontal(|ui| {
        super::dot_label(ui, p.dim, "not connected");
        ui.add_space(8.0);
        for ch in CHANNELS {
            ui.add_enabled(false, Button::selectable(false, ch)).on_disabled_hover_text(NEEDS_AMP);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Right to left: the last item here sits leftmost.
            ui.add_enabled(false, Button::new("Store to channel…")).on_disabled_hover_text(NEEDS_AMP);
            ui.add_enabled(false, Button::new("Load to amp")).on_disabled_hover_text(NEEDS_AMP);
            for label in ["Save as…", "Save", "Quick save"] {
                ui.add_enabled(false, Button::new(label)).on_disabled_hover_text(NEEDS_LIBRARY);
            }
            if ui.button("Save .syx…").clicked() {
                let file = crate::app::default_file_name(&app.editor.name());
                if let Some(path) = rfd::FileDialog::new().add_filter("SysEx", &["syx"]).set_file_name(file).save_file()
                {
                    app.save_to(path);
                }
            }
            if ui.button("Open .syx…").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("SysEx", &["syx"]).pick_file() {
                    app.request_open(path);
                }
            }
            ui.add_enabled(false, egui::Checkbox::new(&mut false, "Live edit")).on_disabled_hover_text(NEEDS_AMP);
            if app.editor.is_dirty() {
                super::dot_label(ui, p.accent, "unsaved");
            }
            let name = ui.label(RichText::new(app.editor.name()).strong());
            if let Some(path) = app.editor.path() {
                name.on_hover_text(path.display().to_string());
            }
        });
    });
}
