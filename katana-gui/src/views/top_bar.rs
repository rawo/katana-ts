use eframe::egui::{self, Align, Button, Layout, RichText};

use crate::app::{KatanaApp, CHANNEL_NAMES};
use crate::theme::Palette;

const NO_AMP: &str = "Amp not connected";

/// First row: connection, channel tabs, and the preset's name and Live edit.
pub fn top_bar(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.horizontal(|ui| {
        if app.connected {
            super::dot_label(ui, p.green, "connected");
        } else {
            super::dot_label(ui, p.dim, "not connected");
        }
        ui.add_space(8.0);
        for (selector, name) in (0u8..).zip(CHANNEL_NAMES) {
            let selected = app.channel_tab == Some(selector);
            let tab = ui.add_enabled(app.channel_enabled(selector), Button::selectable(selected, name));
            let hover = if !app.amp_idle() {
                "The amp is busy"
            } else if app.connected {
                "Select PANEL on the amp"
            } else {
                "Amp not connected; no copy of this channel yet"
            };
            if tab.on_disabled_hover_text(hover).clicked() {
                app.click_channel(selector);
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Right to left: the last item here sits leftmost.
            let mut live = app.live;
            let check = ui.add_enabled(app.connected && app.amp_idle(), egui::Checkbox::new(&mut live, "Live edit"));
            if check.on_disabled_hover_text(NO_AMP).changed() {
                app.set_live(live);
            }
            ui.add_space(8.0);
            if app.editor.is_dirty() {
                super::dot_label(ui, p.accent, "unsaved");
            }
            // A file name can be long; it gives way to the channel tabs.
            let name = ui.add(egui::Label::new(RichText::new(app.editor.display_name()).strong()).truncate());
            if let Some(path) = app.editor.path() {
                name.on_hover_text(path.display().to_string());
            }
            if let Some(note) = app.editor.replaced_note() {
                ui.add(egui::Label::new(RichText::new(note).small().color(p.dim)).truncate());
            }
        });
    });
}
