//! All parameters of the selected block.

use eframe::egui::{self, RichText};
use katana_proto::find_param;
use katana_proto::layout::BlockDef;

use crate::app::KatanaApp;
use crate::theme::Palette;
use crate::widgets::param_control;

pub fn detail(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    let patch = app.editor.patch();
    let mut heading = block.title.to_string();
    if let Some(def) = block.color.and_then(find_param) {
        let v = patch.get(def);
        if let Some(&(_, name)) = def.values.unwrap_or(&[]).iter().find(|&&(n, _)| Some(n) == v) {
            heading += &format!(" · {name} variation");
        }
    }
    if let Some(t) = block.type_label(patch) {
        heading += &format!(" · type {t}");
    }
    ui.label(RichText::new(heading).small().strong().color(p.dim));
    ui.add_space(6.0);
    let controls = block.controls(patch);
    if controls.is_empty() {
        let text = match block.type_label(patch) {
            Some(t) if !block.slots.is_empty() => format!("The knob's settings for {t} are not mapped yet."),
            _ => "No parameters verified on the amp for this block yet.".to_string(),
        };
        ui.label(RichText::new(text).color(p.dim));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for c in controls {
            if let Some(def) = find_param(c.param) {
                param_control(ui, app, def, c.label, p);
            }
        }
    });
}
