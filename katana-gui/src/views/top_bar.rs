use eframe::egui::{self, Align, Button, Layout, RichText};

use super::NEEDS_AMP;
use crate::app::KatanaApp;
use crate::theme::Palette;

const CHANNELS: [&str; 9] = ["PANEL", "A1", "A2", "A3", "A4", "B1", "B2", "B3", "B4"];

/// First row: connection, channel tabs, and the preset's name and Live edit.
pub fn top_bar(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.horizontal(|ui| {
        super::dot_label(ui, p.dim, "not connected");
        ui.add_space(8.0);
        for ch in CHANNELS {
            ui.add_enabled(false, Button::selectable(false, ch)).on_disabled_hover_text(NEEDS_AMP);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Right to left: the last item here sits leftmost.
            ui.add_enabled(false, egui::Checkbox::new(&mut false, "Live edit")).on_disabled_hover_text(NEEDS_AMP);
            ui.add_space(8.0);
            if app.editor.is_dirty() {
                super::dot_label(ui, p.accent, "unsaved");
            }
            // A file name can be long; it gives way to the channel tabs.
            let name = ui.add(egui::Label::new(RichText::new(app.editor.display_name()).strong()).truncate());
            if let Some(path) = app.editor.path() {
                name.on_hover_text(path.display().to_string());
            }
        });
    });
}
