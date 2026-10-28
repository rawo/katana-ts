use eframe::egui::{self, Align, Button, Layout};

use super::{NEEDS_AMP, NEEDS_LIBRARY};
use crate::app::{KatanaApp, Tab};
use crate::theme::Palette;

/// Second row: the Editor/Library tabs and the file and amp actions.
pub fn action_bar(ui: &mut egui::Ui, app: &mut KatanaApp, _p: &Palette) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.tab, Tab::Editor, "Editor");
        ui.selectable_value(&mut app.tab, Tab::Library, "Library");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Right to left: the last item here sits leftmost.
            ui.add_enabled(false, Button::new("Store to channel…")).on_disabled_hover_text(NEEDS_AMP);
            ui.add_enabled(false, Button::new("Load to amp")).on_disabled_hover_text(NEEDS_AMP);
            ui.add_space(8.0);
            for label in ["Save as…", "Save", "Quick save"] {
                ui.add_enabled(false, Button::new(label)).on_disabled_hover_text(NEEDS_LIBRARY);
            }
            ui.add_space(8.0);
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
        });
    });
}
