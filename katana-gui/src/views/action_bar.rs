use eframe::egui::{self, Align, Button, Layout};

use super::NEEDS_AMP;
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
            let can_load = app.connected && app.busy.is_none();
            let load = ui.add_enabled(can_load, Button::new("Load to amp"));
            let why = if app.connected { "The amp is busy" } else { "Amp not connected" };
            if load.on_disabled_hover_text(why).clicked() {
                app.load_to_amp();
            }
            ui.add_space(8.0);
            if ui.button("Save as…").clicked() {
                app.open_save_as();
            }
            let save = ui.add_enabled(app.can_save(), Button::new("Save"));
            if save.on_disabled_hover_text("This patch is read-only; use Save as…").clicked() {
                app.save();
            }
            if ui.button("Quick save").clicked() {
                app.quick_save(chrono::Local::now().naive_local());
            }
            ui.add_space(8.0);
            if ui.button("Export .syx…").clicked() {
                let file = crate::app::default_file_name(&app.editor.display_name());
                if let Some(path) = rfd::FileDialog::new().add_filter("SysEx", &["syx"]).set_file_name(file).save_file()
                {
                    app.export_to(path);
                }
            }
            if ui.button("Import…").clicked() {
                if let Some(path) = super::pick_import_file() {
                    app.import(path);
                }
            }
        });
    });
}
