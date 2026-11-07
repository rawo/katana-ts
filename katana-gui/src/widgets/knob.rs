//! A rotary knob: drag up/down, mouse wheel for single steps, double-click to type.

use eframe::egui::{self, Align2, FontId, Key, MouseWheelUnit, Sense, Shape, Stroke, TextEdit, Vec2};

use crate::theme::Palette;

/// Dragging this far sweeps the whole range.
pub const DRAG_PX_FULL_RANGE: f32 = 200.0;
/// Trackpad scrolling this far is one step.
const SCROLL_PX_PER_STEP: f32 = 20.0;

/// While the page has scrolled this recently (seconds), knobs let the wheel through.
pub const WHEEL_LATCH_S: f64 = 0.5;

/// When the page last got wheel input that no knob took (`ctx.input` time).
fn page_wheel_id() -> egui::Id {
    egui::Id::NULL.with("knob-page-wheel-at")
}

/// The pass in which a knob last took the wheel.
fn knob_wheel_id() -> egui::Id {
    egui::Id::NULL.with("knob-wheel-pass")
}

fn has_wheel(i: &egui::InputState) -> bool {
    i.events.iter().any(|e| matches!(e, egui::Event::MouseWheel { .. }))
}

/// Call once at the end of each frame: wheel input no knob took scrolled the
/// page, so for the next `WHEEL_LATCH_S` knobs leave the wheel to the page.
pub fn track_page_wheel(ctx: &egui::Context) {
    let (wheel, now) = ctx.input(|i| (has_wheel(i), i.time));
    let taken = ctx.data(|d| d.get_temp::<u64>(knob_wheel_id())) == Some(ctx.cumulative_pass_nr());
    if wheel && !taken {
        ctx.data_mut(|d| d.insert_temp(page_wheel_id(), now));
    }
}

/// The value after dragging `dy` points from `start` (down is negative).
pub fn drag_value(start: u32, dy: f32, min: u32, max: u32) -> u32 {
    let span = (max - min) as f32;
    let v = start as f32 - dy * span / DRAG_PX_FULL_RANGE;
    (v.round().max(0.0) as u32).clamp(min, max)
}

/// The value after `notches` wheel steps of one raw unit each.
pub fn wheel_value(v: u32, notches: i32, min: u32, max: u32) -> u32 {
    (v as i64 + notches as i64).clamp(min as i64, max as i64) as u32
}

/// A typed value, if it is a whole number within range.
/// A typed shown value (raw − `ofs`, may be signed) back to its raw value.
pub fn parse_typed(text: &str, min: u32, max: u32, ofs: u32) -> Option<u32> {
    let shown: i64 = text.trim().parse().ok()?;
    u32::try_from(shown + i64::from(ofs)).ok().filter(|v| (min..=max).contains(v))
}

/// The raw value as shown: raw − `ofs`, with a "+" when `ofs` makes it signed.
pub fn shown(raw: u32, ofs: u32) -> String {
    let v = i64::from(raw) - i64::from(ofs);
    if ofs > 0 && v > 0 {
        format!("+{v}")
    } else {
        v.to_string()
    }
}

pub struct Knob<'a> {
    pub label: &'a str,
    pub value: &'a mut u32,
    pub min: u32,
    pub max: u32,
    /// Shown value = raw − `ofs`.
    pub ofs: u32,
    pub palette: &'a Palette,
}

/// Per-knob state kept between frames.
#[derive(Clone, Default)]
struct KnobState {
    /// Value when the current drag started, and how far it has gone.
    drag: Option<(u32, f32)>,
    /// Unspent trackpad scroll.
    scroll: f32,
    /// Text being typed, while editing.
    typing: Option<String>,
}

impl egui::Widget for Knob<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Knob { label, value, min, max, ofs, palette: p } = self;
        let size = Vec2::new(64.0, 84.0);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let id = response.id;
        let mut st: KnobState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let old = *value;

        if response.drag_started() {
            st.drag = Some((*value, 0.0));
        }
        if let Some((start, dy)) = &mut st.drag {
            *dy += response.drag_delta().y;
            *value = drag_value(*start, *dy, min, max);
        }
        if response.drag_stopped() {
            st.drag = None;
        }

        let now = ui.input(|i| i.time);
        let page_wheel_at = ui.data(|d| d.get_temp::<f64>(page_wheel_id()));
        let page_scrolling = page_wheel_at.is_some_and(|t| now - t < WHEEL_LATCH_S);
        if response.hovered() && !page_scrolling {
            if ui.input(has_wheel) {
                let pass = ui.ctx().cumulative_pass_nr();
                ui.data_mut(|d| d.insert_temp(knob_wheel_id(), pass));
            }
            let notches = ui.input(|i| {
                let mut n = 0;
                for e in &i.events {
                    if let egui::Event::MouseWheel { unit, delta, .. } = e {
                        match unit {
                            MouseWheelUnit::Line | MouseWheelUnit::Page => n += delta.y.signum() as i32,
                            MouseWheelUnit::Point => st.scroll += delta.y,
                        }
                    }
                }
                n
            });
            let trackpad = (st.scroll / SCROLL_PX_PER_STEP).trunc();
            st.scroll -= trackpad * SCROLL_PX_PER_STEP;
            let steps = notches + trackpad as i32;
            // Only actual wheel movement may change (and so clamp) the value.
            if steps != 0 {
                *value = wheel_value(*value, steps, min, max);
            }
            // The wheel turns the knob; it must not also scroll the page.
            ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }

        if response.double_clicked() {
            st.typing = Some(shown(*value, ofs));
            st.drag = None;
        }

        let painter = ui.painter_at(rect);
        painter.text(rect.center_top(), Align2::CENTER_TOP, label, FontId::proportional(11.0), p.dim);
        let center = rect.center() - Vec2::new(0.0, 2.0);
        let radius = 20.0;
        painter.circle(center, radius, p.panel, Stroke::new(3.0, p.line));
        let frac = if max > min { ((*value as f32 - min as f32) / (max - min) as f32).clamp(0.0, 1.0) } else { 0.0 };
        arc(&painter, center, radius, frac, Stroke::new(3.0, p.accent));

        let value_rect =
            egui::Rect::from_center_size(rect.center_bottom() - Vec2::new(0.0, 9.0), Vec2::new(56.0, 18.0));
        let mut commit = None;
        if let Some(text) = &mut st.typing {
            let edit_id = id.with("typing");
            let edit = ui.put(value_rect, TextEdit::singleline(text).id(edit_id).align(Align2::CENTER_CENTER));
            let edit = edit.labelled_by(id);
            if !edit.has_focus() && !edit.lost_focus() {
                edit.request_focus();
            }
            if edit.lost_focus() {
                let entered = ui.input(|i| i.key_pressed(Key::Enter));
                commit = Some(if entered { parse_typed(text, min, max, ofs) } else { None });
            }
        } else {
            painter.text(
                value_rect.center(),
                Align2::CENTER_CENTER,
                shown(*value, ofs),
                FontId::proportional(13.0),
                p.text,
            );
        }
        if let Some(typed) = commit {
            if let Some(v) = typed {
                *value = v;
            }
            st.typing = None;
        }

        ui.data_mut(|d| d.insert_temp(id, st));
        if *value != old {
            response.mark_changed();
        }
        let v = *value as f64 - f64::from(ofs);
        response.widget_info(|| egui::WidgetInfo::slider(true, v, label));
        response
    }
}

/// Paints the knob's value arc, from 7 o'clock clockwise to 5 o'clock.
fn arc(painter: &egui::Painter, center: egui::Pos2, r: f32, frac: f32, stroke: Stroke) {
    use std::f32::consts::PI;
    let start = 0.75 * PI;
    let sweep = 1.5 * PI * frac;
    let steps = 32;
    let points: Vec<_> = (0..=steps)
        .map(|i| {
            let a = start + sweep * i as f32 / steps as f32;
            center + r * Vec2::angled(a)
        })
        .collect();
    if frac > 0.0 {
        painter.add(Shape::line(points, stroke));
    }
    let a = start + sweep;
    painter.line_segment([center + (r - 9.0) * Vec2::angled(a), center + r * Vec2::angled(a)], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_full_range_over_200px() {
        assert_eq!(drag_value(0, -200.0, 0, 100), 100);
        assert_eq!(drag_value(100, 200.0, 0, 100), 0);
    }

    #[test]
    fn drag_small_moves_round() {
        assert_eq!(drag_value(50, -3.0, 0, 100), 52);
    }

    #[test]
    fn drag_from_out_of_range_clamps() {
        assert_eq!(drag_value(130, -1.0, 0, 120), 120);
    }

    #[test]
    fn wheel_steps_one_and_clamps() {
        assert_eq!(wheel_value(50, 1, 0, 100), 51);
        assert_eq!(wheel_value(0, -3, 0, 100), 0);
    }

    #[test]
    fn parse_typed_accepts_in_range() {
        assert_eq!(parse_typed(" 42 ", 0, 100, 0), Some(42));
    }

    #[test]
    fn parse_typed_rejects_bad_input() {
        assert_eq!(parse_typed("abc", 0, 100, 0), None);
        assert_eq!(parse_typed("101", 0, 100, 0), None);
        assert_eq!(parse_typed("-1", 0, 100, 0), None);
        assert_eq!(parse_typed("", 0, 100, 0), None);
    }

    #[test]
    fn parse_typed_takes_signed_values_with_an_offset() {
        assert_eq!(parse_typed("-5", 0, 40, 20), Some(15));
        assert_eq!(parse_typed("+20", 0, 40, 20), Some(40));
        assert_eq!(parse_typed("21", 0, 40, 20), None);
        assert_eq!(parse_typed("-21", 0, 40, 20), None);
    }

    #[test]
    fn shown_value_is_signed_with_an_offset() {
        assert_eq!(shown(15, 20), "-5");
        assert_eq!(shown(20, 20), "0");
        assert_eq!(shown(25, 20), "+5");
        assert_eq!(shown(42, 0), "42");
    }
}
