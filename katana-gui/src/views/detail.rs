//! All settings of the selected block. Colour blocks show three colour tabs
//! (each with its own type), a TYPE drop-down and every setting of the type.

use eframe::egui::{self, Button, RichText, Role, Sense, Stroke, Vec2, WidgetInfo};
use katana_proto::layout::{BlockDef, Control};
use katana_proto::settings::{self, color_of, groups, panel_controls, type_def, Panel};
use katana_proto::{find_param, format_value, Patch};

use crate::app::KatanaApp;
use crate::theme::Palette;
use crate::widgets::{param_control, Fader};

const COLORS: [&str; 3] = ["GREEN", "RED", "ORANGE"];

/// The colour panel of a chain block, if it has one.
pub fn panel_of(block_id: &str) -> Option<Panel> {
    Some(match block_id {
        "booster" => Panel::Booster,
        "mod" => Panel::Mod,
        "fx" => Panel::Fx,
        "delay" => Panel::Delay,
        _ => return None,
    })
}

pub fn detail(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    match panel_of(block.id) {
        Some(panel) => colour_panel(ui, app, block, panel, p),
        None if block.id == "dly2rev" => dly2rev_panel(ui, app, block, p),
        None if block.id == "eq1" => eq_panel(ui, app, block, 1, p),
        None if block.id == "eq2" => eq_panel(ui, app, block, 2, p),
        None if block.id == "solo" => solo_panel(ui, app, block, p),
        None => plain(ui, app, block, p),
    }
}

/// DELAY 2 / REVERB: colour tabs (the REVERB colour), the colour's layer
/// MODE, then DELAY 2 and REVERB side by side; the half the mode doesn't
/// play is dimmed but stays editable.
fn dly2rev_panel(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    let color = app.shown_color(block.id, Panel::Reverb).unwrap_or(0);
    let patch = app.editor.patch();
    let heading = format!(
        "{} · {} variation · type {}",
        block.title,
        COLORS[color as usize % 3],
        type_name(Panel::Reverb, color, patch)
    );
    ui.label(RichText::new(heading).small().strong().color(p.dim));
    ui.add_space(6.0);
    colour_tabs(ui, app, block.id, Panel::Reverb, p);
    ui.add_space(6.0);
    let Some(mode_def) = settings::setting("REVERB", color as usize, "layer-mode") else { return };
    let before = app.editor.get(mode_def);
    param_control(ui, app, mode_def, "MODE", p);
    let after = app.editor.get(mode_def);
    // With the block on, the amp moves the switches to the new mode by itself
    // (docs/protocol.md); the editor does the same so both stay alike.
    if let (Some(mode), true) = (after, after != before) {
        if color_of(Panel::Reverb, app.editor.patch()) == Some(color)
            && settings::delay2_reverb_switches(before.unwrap_or(2))
                .iter()
                .any(|n| find_param(n).and_then(|d| app.editor.get(d)) == Some(1))
        {
            for (name, v) in settings::delay2_reverb_edits(true, mode) {
                if let Some(d) = find_param(name) {
                    let r = app.edit(d, v);
                    debug_assert!(r.is_ok());
                }
            }
        }
    }
    ui.add_space(6.0);
    let used = settings::delay2_reverb_switches(after.unwrap_or(2));
    ui.columns(2, |cols| {
        for (col, (panel, title, sw)) in
            cols.iter_mut().zip([(Panel::Delay2, "DELAY 2", "sw-delay2-sw"), (Panel::Reverb, "REVERB", "reverb-sw")])
        {
            if !used.contains(&sw) {
                col.multiply_opacity(0.4);
            }
            col.label(RichText::new(title).strong());
            if let Some(d) = find_param(sw) {
                param_control(col, app, d, &format!("{title} SW"), p);
            }
            if let Some(d) = type_def(panel, color) {
                param_control(col, app, d, &format!("{title} TYPE"), p);
            }
            settings_list(col, app, panel, color, p);
        }
    });
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
    let patch = app.editor.patch();
    let controls = panel_controls(panel, color, patch);
    if controls.is_empty() {
        // A type value the amp has no type for (e.g. from an old .syx).
        let unknown = type_def(panel, color).and_then(|d| patch.get(d).filter(|&v| v > d.max));
        let text = match unknown {
            Some(v) => format!("Unknown type ? ({v}): nothing to show. Pick a TYPE to edit this colour."),
            None => "No parameters verified on the amp for this block yet.".to_string(),
        };
        ui.label(RichText::new(text).color(p.dim));
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
    // Top-aligned: a centred row inside a wrapping row is pushed down as the
    // wrapping row grows, so knobs would step down one after another.
    ui.horizontal_top(|ui| {
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

/// EQ 1 / EQ 2: POSITION and TYPE, then GE-10 faders or PARAMETRIC knobs.
fn eq_panel(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, k: u8, p: &Palette) {
    ui.label(RichText::new(block.title).small().strong().color(p.dim));
    ui.add_space(6.0);
    let i = usize::from(k) - 1;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        for (key, label) in [("position", "POSITION"), ("type", "TYPE")] {
            if let Some(d) = settings::setting("EQ_EACH", i, key) {
                param_control(ui, app, d, label, p);
            }
        }
    });
    ui.add_space(6.0);
    let ge10 = type_def(Panel::Eq(k), 0).and_then(|d| app.editor.get(d)) == Some(1);
    if !ge10 {
        settings_list(ui, app, Panel::Eq(k), 0, p);
        return;
    }
    ui.horizontal(|ui| {
        for c in panel_controls(Panel::Eq(k), 0, app.editor.patch()) {
            let Some(def) = find_param(c.param) else { continue };
            let Some(value) = app.editor.get(def) else { continue };
            let mut new = value;
            ui.add(Fader { label: c.label, value: &mut new, def, palette: p });
            if new != value {
                let r = app.edit(def, new);
                debug_assert!(r.is_ok());
            }
        }
    });
}

/// SOLO: its LEVEL, then SOLO EQ and the solo delay, which the amp doesn't
/// let us change (docs/protocol.md), so it is shown as text.
fn solo_panel(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    ui.label(RichText::new(block.title).small().strong().color(p.dim));
    ui.add_space(6.0);
    if let Some(d) = settings::setting("SOLO_COM", 0, "effect-level") {
        param_control(ui, app, d, "SOLO LEVEL", p);
    }
    ui.add_space(6.0);
    ui.columns(2, |cols| {
        let [eq, delay] = cols else { return };
        eq.label(RichText::new("SOLO EQ").strong());
        eq.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            for (key, label) in [("sw", "SOLO EQ SW"), ("position", "POSITION")] {
                if let Some(d) = settings::setting("SOLO_EQ", 0, key) {
                    param_control(ui, app, d, label, p);
                }
            }
        });
        settings_list(eq, app, Panel::SoloEq, 0, p);
        delay.label(RichText::new("SOLO DELAY").strong());
        delay.label(RichText::new("The amp doesn't accept changes to the solo delay over USB.").small().color(p.dim));
        let patch = app.editor.patch();
        for c in panel_controls(Panel::SoloDelay, 0, patch) {
            if let Some(v) = find_param(c.param).and_then(|d| patch.get(d).map(|v| format_value(d, v))) {
                delay.label(format!("{}: {v}", c.label));
            }
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
