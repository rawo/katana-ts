//! Picks the widget for a parameter from its definition.

use eframe::egui::{self, Button, ComboBox, RichText};
use katana_proto::{Format, ParamDef};

use super::Knob;
use crate::app::KatanaApp;
use crate::theme::Palette;

const ON_OFF: &[(u32, &str)] = &[(0, "OFF"), (1, "ON")];

/// "OFF", "ON", or "? (n)" for a raw value that is neither (e.g. from an odd `.syx`).
pub fn on_off_text(value: u32) -> String {
    match value {
        0 => "OFF".to_string(),
        1 => "ON".to_string(),
        n => format!("? ({n})"),
    }
}

/// Which widget edits a parameter.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Widget {
    Toggle,
    DropDown,
    Knob,
}

/// On/off → toggle; named values → drop-down, unless the names are a
/// knob's display (`Format::Names`); anything else → knob.
pub(crate) fn widget_for(def: &ParamDef) -> Widget {
    match def.values {
        Some(ON_OFF) => Widget::Toggle,
        Some(_) if def.format != Format::Names => Widget::DropDown,
        _ => Widget::Knob,
    }
}

/// Draws the right widget for `def` (`widget_for`) and applies changes with
/// `KatanaApp::edit`.
pub fn param_control(
    ui: &mut egui::Ui,
    app: &mut KatanaApp,
    def: &'static ParamDef,
    label: &str,
    p: &Palette,
) -> egui::Response {
    let Some(value) = app.editor.get(def) else {
        return ui
            .vertical(|ui| {
                ui.label(RichText::new(label).small().color(p.dim));
                ui.label("—");
            })
            .response;
    };
    let mut new = value;
    let response = match widget_for(def) {
        Widget::Toggle => {
            ui.vertical(|ui| {
                let caption = ui.label(RichText::new(label).small().color(p.dim));
                let on = value == 1;
                let r = ui.add(Button::selectable(on, on_off_text(value))).labelled_by(caption.id);
                if r.clicked() {
                    new = u32::from(!on);
                }
            })
            .response
        }
        Widget::DropDown => {
            let values = def.values.unwrap_or(&[]);
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
        Widget::Knob => ui.add(Knob { label, value: &mut new, def, palette: p }),
    };
    if new != value {
        let r = app.edit(def, new);
        debug_assert!(r.is_ok(), "{} = {new}: {r:?}", def.name);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use katana_proto::Encoding;

    fn def(values: Option<&'static [(u32, &'static str)]>, format: Format) -> ParamDef {
        ParamDef { name: "T", addr: 0, encoding: Encoding::U7, min: 0, max: 2, values, ofs: 0, format }
    }

    #[test]
    fn widget_follows_values_and_format() {
        const LIST: &[(u32, &str)] = &[(0, "A"), (1, "B"), (2, "C")];
        assert_eq!(widget_for(&def(Some(ON_OFF), Format::Plain)), Widget::Toggle);
        assert_eq!(widget_for(&def(Some(LIST), Format::Plain)), Widget::DropDown);
        assert_eq!(widget_for(&def(Some(LIST), Format::Names)), Widget::Knob);
        assert_eq!(widget_for(&def(None, Format::Unit("ms"))), Widget::Knob);
    }

    #[test]
    fn on_off_text_names_unknown_values() {
        assert_eq!(on_off_text(0), "OFF");
        assert_eq!(on_off_text(1), "ON");
        assert_eq!(on_off_text(5), "? (5)");
    }
}
