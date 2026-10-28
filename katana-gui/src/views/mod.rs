//! The parts of the main window.

mod action_bar;
mod amp_strip;
mod chain;
mod detail;
mod library;
mod status;
mod top_bar;

use katana_proto::layout::CHAIN;

use crate::app::KatanaApp;
use crate::theme::Palette;

pub use action_bar::action_bar;
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

/// The Editor tab: signal chain, detail panel and amp strip, scrolling when
/// the window is too small.
pub fn editor_tab(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        section(ui, p, |ui| {
            ui.label(RichText::new("SIGNAL CHAIN").small().strong().color(p.dim));
            ui.add_space(6.0);
            chain::chain(ui, app, p);
        });
        let block = CHAIN.iter().find(|b| b.id == app.selected_block).unwrap_or(&CHAIN[0]);
        section(ui, p, |ui| detail::detail(ui, app, block, p));
        section(ui, p, |ui| amp_strip::amp_strip(ui, app, p));
    });
}

fn section(ui: &mut egui::Ui, p: &Palette, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(p.panel)
        .stroke(egui::Stroke::new(1.0, p.line))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(12.0);
}
