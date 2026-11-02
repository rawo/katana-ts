use eframe::egui::{self, Align, ComboBox, Label, Layout, RichText};

use crate::app::{KatanaApp, CHANNEL_NAMES};
use crate::theme::{Palette, ThemeId};

pub fn status(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    // Right to left, so the Theme picker always gets its room and the text takes what is left.
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ComboBox::from_label("Theme").selected_text(app.theme.name()).show_ui(ui, |ui| {
            for t in ThemeId::ALL {
                ui.selectable_value(&mut app.theme, t, t.name());
            }
        });
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let autosave = app.last_autosave.map_or("—".to_string(), |t| t.format("%H:%M").to_string());
            let amp = match (app.connected, app.amp_on.and_then(|s| CHANNEL_NAMES.get(usize::from(s)))) {
                (false, _) => "not connected".to_string(),
                (true, Some(channel)) => format!("connected, on {channel}"),
                (true, None) => "connected".to_string(),
            };
            let live = if app.live { "on" } else { "off" };
            let panel = app
                .last_panel_change
                .as_ref()
                .map_or("—".to_string(), |(what, at)| format!("{what} {}", at.format("%H:%M")));
            ui.label(
                RichText::new(format!(
                    "Amp: {amp} · Live edit {live} · last auto-save {autosave} · panel change {panel}"
                ))
                .color(p.dim)
                .small(),
            );
            if let Some(busy) = &app.busy {
                ui.label(RichText::new(busy).color(p.accent).small());
            }
            if let Some(e) = &app.error {
                ui.add(Label::new(RichText::new(e).color(p.red).small()).truncate()).on_hover_text(e);
            }
        });
    });
}
