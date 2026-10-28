use eframe::egui::{self, Align, ComboBox, Label, Layout, RichText};

use crate::app::KatanaApp;
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
            ui.label(
                RichText::new(format!(
                    "Amp: not connected · Live edit off · last auto-save {autosave} · panel change —"
                ))
                .color(p.dim)
                .small(),
            );
            if let Some(e) = &app.error {
                ui.add(Label::new(RichText::new(e).color(p.red).small()).truncate()).on_hover_text(e);
            }
        });
    });
}
