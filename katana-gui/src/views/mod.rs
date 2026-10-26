//! The parts of the main window.

mod library;
mod status;
mod top_bar;

pub use library::library;
pub use status::status;
pub use top_bar::top_bar;

use eframe::egui::{self, Color32, RichText, Sense};

/// A painted status dot and its text (the default fonts have no "●").
pub fn dot_label(ui: &mut egui::Ui, color: Color32, text: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        ui.label(RichText::new(text).color(color))
    })
    .inner
}

/// Tooltips for actions that later stages bring.
pub const NEEDS_AMP: &str = "Needs amp control (stage 1)";
pub const NEEDS_LIBRARY: &str = "Preset library (stage 3)";
