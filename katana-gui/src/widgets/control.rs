//! Picks the widget for a parameter from its definition.

use eframe::egui::{self, Button, ComboBox, RichText};
use katana_proto::ParamDef;

use super::Knob;
use crate::editor::Editor;
use crate::theme::Palette;

const ON_OFF: &[(u32, &str)] = &[(0, "OFF"), (1, "ON")];

/// Draws the right widget for `def` and writes changes into the editor:
/// on/off → toggle, named values → dropdown, anything else → knob.
pub fn param_control(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    def: &'static ParamDef,
    label: &str,
    p: &Palette,
) -> egui::Response {
    let Some(value) = editor.get(def) else {
        return ui
            .vertical(|ui| {
                ui.label(RichText::new(label).small().color(p.dim));
                ui.label("—");
            })
            .response;
    };
    let mut new = value;
    let response = match def.values {
        Some(ON_OFF) => {
            ui.vertical(|ui| {
                let caption = ui.label(RichText::new(label).small().color(p.dim));
                let on = value == 1;
                let r = ui.add(Button::selectable(on, if on { "ON" } else { "OFF" })).labelled_by(caption.id);
                if r.clicked() {
                    new = u32::from(!on);
                }
            })
            .response
        }
        Some(values) => {
            ui.vertical(|ui| {
                let caption = ui.label(RichText::new(label).small().color(p.dim));
                let current = values
                    .iter()
                    .find(|&&(v, _)| v == value)
                    .map_or_else(|| format!("? ({value})"), |&(_, name)| name.to_string());
                let combo = ComboBox::from_id_salt(def.name).selected_text(current).show_ui(ui, |ui| {
                    for &(v, name) in values {
                        ui.selectable_value(&mut new, v, name);
                    }
                });
                let _ = combo.response.labelled_by(caption.id);
            })
            .response
        }
        None => ui.add(Knob { label, value: &mut new, min: def.min, max: def.max, palette: p }),
    };
    if new != value {
        let r = editor.set(def, new);
        debug_assert!(r.is_ok(), "{} = {new}: {r:?}", def.name);
    }
    response
}
