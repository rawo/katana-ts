//! The amp knobs, always visible.

use eframe::egui::{self, RichText};
use katana_proto::find_param;
use katana_proto::layout::AMP_STRIP;

use crate::app::KatanaApp;
use crate::theme::Palette;
use crate::widgets::param_control;

pub fn amp_strip(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    ui.label(RichText::new("AMP KNOBS").small().strong().color(p.dim));
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for c in AMP_STRIP {
            if let Some(def) = find_param(c.param) {
                param_control(ui, &mut app.editor, def, c.label, p);
            }
        }
    });
}
