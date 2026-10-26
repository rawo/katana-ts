use eframe::egui::{self, Align, ComboBox, Layout, RichText};

use crate::app::KatanaApp;
use crate::theme::{Palette, ThemeId};

pub fn status(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Amp: not connected · Live edit off · last auto-save — · panel change —")
                .color(p.dim)
                .small(),
        );
        if let Some(e) = &app.error {
            ui.label(RichText::new(e).color(p.red).small());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ComboBox::from_label("Theme").selected_text(app.theme.name()).show_ui(ui, |ui| {
                for t in ThemeId::ALL {
                    ui.selectable_value(&mut app.theme, t, t.name());
                }
            });
        });
    });
}
