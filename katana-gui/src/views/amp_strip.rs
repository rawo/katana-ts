//! The amp knobs, always visible, and CONTOUR.

use eframe::egui::{self, Button, RichText, Role, WidgetInfo};
use katana_proto::find_param;
use katana_proto::layout::AMP_STRIP;
use katana_proto::settings::setting;

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
                param_control(ui, app, def, c.label, p);
            }
        }
        ui.separator();
        contour(ui, app, p);
    });
}

/// CONTOUR OFF / 1 / 2 / 3, then the selected contour's TYPE and FREQ SHIFT.
fn contour(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    let Some(def) = find_param("contour") else { return };
    let current = app.editor.get(def);
    ui.vertical(|ui| {
        ui.label(RichText::new("CONTOUR").small().color(p.dim));
        ui.horizontal(|ui| {
            for &(v, name) in def.values.unwrap_or(&[]) {
                let on = current == Some(v);
                let r = ui.add(Button::selectable(on, name));
                let label = format!("CONTOUR {name}");
                r.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, on, &label));
                if r.clicked() && !on {
                    let res = app.edit(def, v);
                    debug_assert!(res.is_ok());
                }
            }
        });
    });
    let Some(n @ 1..=3) = current else { return };
    let i = n as usize - 1;
    for (key, label) in [("type", "CONTOUR TYPE"), ("freq-shift", "FREQ SHIFT")] {
        if let Some(d) = setting("CONTOUR", i, key) {
            param_control(ui, app, d, label, p);
        }
    }
}
