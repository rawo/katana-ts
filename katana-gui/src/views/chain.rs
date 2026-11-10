//! The signal chain: one card per block, in processing order.

use eframe::egui::{self, Color32, CornerRadius, Frame, RichText, Role, Sense, Stroke, UiBuilder, Vec2, WidgetInfo};
use katana_proto::find_param;
use katana_proto::layout::{BlockDef, CHAIN};

use crate::app::KatanaApp;
use crate::theme::Palette;
use crate::widgets::on_off_text;

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
                        // Not selectable: a selectable label would take the card's click.
                        ui.add(egui::Label::new(RichText::new(t).small().color(p.dim)).selectable(false));
                    }
                    if let Some(color) = block.color {
                        ui.horizontal(|ui| {
                            leds(ui, app, block.title, color, p);
                            if let Some(ms) = block.tap_time(app.editor.patch()) {
                                let on = block.switch.and_then(find_param).and_then(|d| app.editor.get(d)) == Some(1);
                                tap_light(ui, block.title, on, ms, p);
                            }
                        });
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
    let value = app.editor.get(def);
    let on = value == Some(1);
    let label = value.map_or_else(|| "—".to_string(), on_off_text);
    let (fill, text) = if on { (p.green, Color32::BLACK) } else { (p.line, p.dim) };
    let galley = ui.painter().layout_no_wrap(label.clone(), egui::FontId::proportional(10.0), text);
    let width = (galley.size().x + 8.0).max(34.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::click());
    ui.painter().rect_filled(rect, CornerRadius::same(3), fill);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text);
    let name = format!("{title} {label}");
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
    if response.clicked() {
        let r = app.edit(def, u32::from(!on));
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
                let r = app.edit(def, v);
                debug_assert!(r.is_ok());
            }
        }
    });
}

/// The TAP light: flashes red once per delay interval, like the amp's TAP button.
fn tap_light(ui: &mut egui::Ui, title: &str, on: bool, time_ms: u32, p: &Palette) {
    ui.add_space(6.0);
    let now = ui.input(|i| i.time);
    let lit = tap_lit(on, time_ms, now);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 5.0, if lit { p.red } else { p.red.gamma_multiply(0.25) });
    let label = format!("{title} TAP");
    response.widget_info(|| WidgetInfo::selected(Role::Image, true, lit, &label));
    response.on_hover_text(format!("TAP {time_ms} ms"));
    if let Some(next) = tap_wake(on, time_ms, now) {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(next.max(0.005)));
    }
}

/// Shorter delay times light the TAP light steadily: a blink that fast can't
/// be shown and would repaint the app constantly.
const MIN_BLINK_MS: u32 = 150;

fn flash_len(period: f64) -> f64 {
    (period / 4.0).min(0.15)
}

/// Whether the TAP light is on `now` seconds into the clock: a short flash at
/// the start of every delay interval, steady below `MIN_BLINK_MS`, dark with
/// the delay off (or a time of 0).
pub(crate) fn tap_lit(on: bool, time_ms: u32, now: f64) -> bool {
    if !on || time_ms == 0 {
        return false;
    }
    if time_ms < MIN_BLINK_MS {
        return true;
    }
    let period = f64::from(time_ms) / 1000.0;
    now.rem_euclid(period) < flash_len(period)
}

/// Seconds until the TAP light next changes, or `None` when it never does.
pub(crate) fn tap_wake(on: bool, time_ms: u32, now: f64) -> Option<f64> {
    if !on || time_ms < MIN_BLINK_MS {
        return None;
    }
    let period = f64::from(time_ms) / 1000.0;
    let phase = now.rem_euclid(period);
    let flash = flash_len(period);
    Some(if phase < flash { flash - phase } else { period - phase })
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

#[cfg(test)]
mod tests {
    use super::{tap_lit, tap_wake};

    #[test]
    fn tap_light_blinks_once_per_delay_time() {
        assert!(tap_lit(true, 500, 0.0));
        assert!(tap_lit(true, 500, 0.1));
        assert!(!tap_lit(true, 500, 0.3));
        assert!(tap_lit(true, 500, 0.51));
        assert!(!tap_lit(true, 2000, 1.0));
        assert!(tap_lit(true, 2000, 2.05));
    }

    #[test]
    fn tap_light_is_dark_with_delay_off() {
        assert!(!tap_lit(false, 500, 0.0));
        assert_eq!(tap_wake(false, 500, 0.0), None);
    }

    #[test]
    fn tap_light_wakes_only_for_flash_edges() {
        let near = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-9);
        assert!(near(tap_wake(true, 500, 0.0), 0.125));
        assert!(near(tap_wake(true, 500, 0.2), 0.3));
    }

    #[test]
    fn short_delay_times_light_steadily_without_repaints() {
        // A 2.5 ms flash every 10 ms can't be shown; it would repaint ~200 times a second.
        for now in [0.0, 0.003, 0.007, 1.234] {
            assert!(tap_lit(true, 10, now));
            assert!(tap_lit(true, 149, now));
        }
        assert_eq!(tap_wake(true, 10, 0.0), None);
        assert_eq!(tap_wake(true, 149, 0.0), None);
        assert!(tap_wake(true, 150, 0.0).is_some());
        // A damaged patch with time 0: dark, no repaints.
        assert!(!tap_lit(true, 0, 0.0));
        assert_eq!(tap_wake(true, 0, 0.0), None);
    }
}
