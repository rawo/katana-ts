//! The signal chain: one card per block, in processing order.

use eframe::egui::{self, Color32, CornerRadius, Frame, RichText, Role, Sense, Stroke, UiBuilder, Vec2, WidgetInfo};
use katana_proto::find_param;
use katana_proto::layout::{BlockDef, CHAIN};

use crate::app::KatanaApp;
use crate::theme::Palette;

pub fn chain(ui: &mut egui::Ui, app: &mut KatanaApp, p: &Palette) {
    // Fixed row height, so the IN label and arrows centre on the cards.
    let row = Vec2::new(ui.available_width(), 84.0);
    ui.allocate_ui_with_layout(row, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(RichText::new("IN").color(p.dim));
        for block in CHAIN {
            arrow(ui, p);
            card(ui, app, block, p);
        }
        arrow(ui, p);
        ui.label(RichText::new("OUT").color(p.dim));
    });
}

fn card(ui: &mut egui::Ui, app: &mut KatanaApp, block: &'static BlockDef, p: &Palette) {
    let selected = app.selected_block == block.id;
    let scope = ui.scope_builder(UiBuilder::new().sense(Sense::click()), |ui| {
        Frame::new()
            .fill(p.panel)
            .stroke(Stroke::new(1.0, if selected { p.accent } else { p.line }))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(120.0, 64.0));
                ui.vertical(|ui| {
                    let title = ui.horizontal(|ui| {
                        let title = ui.label(RichText::new(block.title).strong());
                        if let Some(sw) = block.switch {
                            pill(ui, app, block.title, sw, p);
                        }
                        title
                    });
                    if let Some(t) = block.type_label(app.editor.patch()) {
                        ui.label(RichText::new(t).small().color(p.dim));
                    }
                    if let Some(color) = block.color {
                        leds(ui, app, block.title, color, p);
                    }
                    title.inner.id
                })
                .inner
            })
            .inner
    });
    let response = scope.response.labelled_by(scope.inner);
    let title = block.title;
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, title));
    if response.clicked() {
        app.selected_block = block.id;
    }
}

/// The block's on/off switch.
fn pill(ui: &mut egui::Ui, app: &mut KatanaApp, title: &str, param: &str, p: &Palette) {
    let Some(def) = find_param(param) else { return };
    let on = app.editor.get(def) == Some(1);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 16.0), Sense::click());
    let (fill, text) = if on { (p.green, Color32::BLACK) } else { (p.line, p.dim) };
    ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        if on { "ON" } else { "OFF" },
        egui::FontId::proportional(10.0),
        text,
    );
    let name = format!("{title} {}", if on { "ON" } else { "OFF" });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    if response.clicked() {
        let r = app.editor.set(def, u32::from(!on));
        debug_assert!(r.is_ok());
    }
}

/// Variation lights; clicking one selects that colour.
fn leds(ui: &mut egui::Ui, app: &mut KatanaApp, title: &str, param: &str, p: &Palette) {
    let Some(def) = find_param(param) else { return };
    let current = app.editor.get(def);
    ui.horizontal(|ui| {
        for &(v, name) in def.values.unwrap_or(&[]) {
            let color = match name {
                "GREEN" => p.green,
                "RED" => p.red,
                _ => p.orange,
            };
            let lit = current == Some(v);
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
            ui.painter().circle_filled(rect.center(), 5.0, if lit { color } else { color.gamma_multiply(0.25) });
            let label = format!("{title} {name}");
            response.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, lit, &label));
            if response.clicked() {
                let r = app.editor.set(def, v);
                debug_assert!(r.is_ok());
            }
        }
    });
}

/// A painted right arrow (the default fonts have no "→").
fn arrow(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 12.0), Sense::hover());
    let stroke = Stroke::new(1.5, p.dim);
    let (l, r) = (rect.left_center(), rect.right_center());
    ui.painter().line_segment([l, r], stroke);
    ui.painter().line_segment([r, r + Vec2::new(-5.0, -4.0)], stroke);
    ui.painter().line_segment([r, r + Vec2::new(-5.0, 4.0)], stroke);
}
