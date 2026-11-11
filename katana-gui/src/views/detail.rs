//! All settings of the selected block. Colour blocks show three colour tabs
//! (each with its own type), a TYPE drop-down and every setting of the type.

use eframe::egui::{self, Button, RichText, Role, Sense, Stroke, Vec2, WidgetInfo};
use katana_proto::layout::{BlockDef, Control};
use katana_proto::settings::{color_of, groups, panel_controls, type_def, Panel};
use katana_proto::{find_param, Patch};

use crate::app::KatanaApp;
use crate::theme::Palette;
use crate::widgets::param_control;

const COLORS: [&str; 3] = ["GREEN", "RED", "ORANGE"];

/// The colour panel of a chain block, if it has one.
pub fn panel_of(block_id: &str) -> Option<Panel> {
    Some(match block_id {
        "booster" => Panel::Booster,
        "mod" => Panel::Mod,
        "fx" => Panel::Fx,
        "delay" => Panel::Delay,
        "reverb" => Panel::Reverb,
        _ => return None,
    })
}

pub fn detail(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    match panel_of(block.id) {
        Some(panel) => colour_panel(ui, app, block, panel, p),
        None => plain(ui, app, block, p),
    }
}

/// The type name of a panel's instance for `color`, e.g. "CHORUS".
pub(crate) fn type_name(panel: Panel, color: u32, patch: &Patch) -> String {
    let Some(def) = type_def(panel, color) else { return String::new() };
    match patch.get(def) {
        Some(v) => def.values.and_then(|vs| vs.iter().find(|&&(n, _)| n == v)).map_or(format!("? ({v})"), |&(_, n)| n.into()),
        None => "—".into(),
    }
}

/// GREEN / RED / ORANGE tabs labelled with each colour's type. The playing
/// colour is outlined; clicking a tab only changes what is shown.
pub(crate) fn colour_tabs(ui: &mut egui::Ui, app: &mut KatanaApp, block_id: &'static str, panel: Panel, p: &Palette) {
    let patch = app.editor.patch();
    let playing = color_of(panel, patch);
    let shown = app.shown_color(block_id, panel);
    let labels: Vec<String> = (0..3u32).map(|c| format!("{} · {}", COLORS[c as usize], type_name(panel, c, patch))).collect();
    ui.horizontal(|ui| {
        for (c, label) in labels.iter().enumerate() {
            let c = c as u32;
            let fill = match c {
                0 => p.green,
                1 => p.red,
                _ => p.orange,
            };
            let stroke = if playing == Some(c) { Stroke::new(2.0, p.accent) } else { Stroke::NONE };
            let selected = shown == Some(c);
            let text = RichText::new(label.as_str()).color(if selected { egui::Color32::BLACK } else { p.text });
            let button = Button::new(text).fill(if selected { fill } else { fill.gamma_multiply(0.25) }).stroke(stroke);
            let r = ui.add(button);
            r.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
            let r = if playing == Some(c) { r.on_hover_text("The amp plays this colour") } else { r };
            if r.clicked() {
                app.panel_color.insert(block_id, c);
            }
        }
    });
}

fn colour_panel(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, panel: Panel, p: &Palette) {
    let color = app.shown_color(block.id, panel).unwrap_or(0);
    let patch = app.editor.patch();
    let heading = format!("{} · {} variation · type {}", block.title, COLORS[color as usize % 3], type_name(panel, color, patch));
    ui.label(RichText::new(heading).small().strong().color(p.dim));
    ui.add_space(6.0);
    colour_tabs(ui, app, block.id, panel, p);
    ui.add_space(6.0);
    if let Some(def) = type_def(panel, color) {
        param_control(ui, app, def, &format!("{} TYPE", block.title), p);
    }
    ui.add_space(6.0);
    settings_list(ui, app, panel, color, p);
}

/// The panel's settings for `color`, grouped by their labels' first word.
pub(crate) fn settings_list(ui: &mut egui::Ui, app: &mut KatanaApp, panel: Panel, color: u32, p: &Palette) {
    let controls = panel_controls(panel, color, app.editor.patch());
    if controls.is_empty() {
        ui.label(RichText::new("No parameters verified on the amp for this block yet.").color(p.dim));
        return;
    }
    for (name, group) in groups(&controls) {
        if let Some(name) = name {
            ui.label(RichText::new(name).small().color(p.dim));
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            for c in group {
                control(ui, app, c, p);
            }
        });
    }
}

/// One setting, with a mark when the amp's panel knob also moves it.
pub(crate) fn control(ui: &mut egui::Ui, app: &mut KatanaApp, c: Control, p: &Palette) {
    let Some(def) = find_param(c.param) else { return };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        param_control(ui, app, def, c.label, p);
        if c.knob {
            let (rect, r) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_stroke(rect.center(), 3.0, Stroke::new(1.5, p.accent));
            let name = format!("Panel knob: {}", c.label);
            r.widget_info(|| WidgetInfo::labeled(Role::Image, true, &name));
            r.on_hover_text("The panel knob also moves this");
        }
    });
}

/// Blocks without colours (the AMP card): their fixed controls.
fn plain(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    let patch = app.editor.patch();
    let mut heading = block.title.to_string();
    if let Some(t) = block.type_label(patch) {
        heading += &format!(" · type {t}");
    }
    ui.label(RichText::new(heading).small().strong().color(p.dim));
    ui.add_space(6.0);
    let controls = block.controls(patch);
    if controls.is_empty() {
        ui.label(RichText::new("No parameters verified on the amp for this block yet.").color(p.dim));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for c in controls {
            control(ui, app, c, p);
        }
    });
}
