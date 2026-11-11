//! A vertical slider (GE-10 bands): drag up/down, mouse wheel for single
//! steps, like `Knob`.

use eframe::egui::{self, Align2, CornerRadius, FontId, Sense, Stroke, Vec2};
use katana_proto::{format_value, ParamDef};

use super::knob::{take_wheel, wheel_value};
use crate::theme::Palette;

/// Height of the track; dragging this far sweeps the whole range.
const TRACK_PX: f32 = 100.0;

/// The value after dragging `dy` points from `start` on a track `track` high
/// (up is negative `dy` and raises the value).
pub fn fader_value(start: u32, dy: f32, min: u32, max: u32, track: f32) -> u32 {
    let v = start as f32 - dy * (max - min) as f32 / track;
    (v.round().max(0.0) as u32).clamp(min, max)
}

pub struct Fader<'a> {
    pub label: &'a str,
    pub value: &'a mut u32,
    pub def: &'a ParamDef,
    pub palette: &'a Palette,
}

#[derive(Clone, Default)]
struct FaderState {
    drag: Option<(u32, f32)>,
    scroll: f32,
}

impl egui::Widget for Fader<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Fader { label, value, def, palette: p } = self;
        let (min, max) = (def.min, def.max);
        let (rect, mut response) = ui.allocate_exact_size(Vec2::new(44.0, TRACK_PX + 40.0), Sense::click_and_drag());
        let id = response.id;
        let mut st: FaderState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let old = *value;
        if response.drag_started() {
            st.drag = Some((*value, 0.0));
        }
        if let Some((start, dy)) = &mut st.drag {
            *dy += response.drag_delta().y;
            *value = fader_value(*start, *dy, min, max, TRACK_PX);
        }
        if response.drag_stopped() {
            st.drag = None;
        }
        let steps = take_wheel(ui, &response, &mut st.scroll);
        if steps != 0 {
            *value = wheel_value(*value, steps, min, max);
        }

        let painter = ui.painter_at(rect);
        painter.text(rect.center_top(), Align2::CENTER_TOP, label, FontId::proportional(11.0), p.dim);
        let track = egui::Rect::from_center_size(rect.center(), Vec2::new(6.0, TRACK_PX));
        painter.rect_filled(track, CornerRadius::same(3), p.line);
        let frac = if max > min { (*value - min.min(*value)) as f32 / (max - min) as f32 } else { 0.0 };
        let y = track.bottom() - frac.clamp(0.0, 1.0) * TRACK_PX;
        let thumb = egui::Rect::from_center_size(egui::pos2(track.center().x, y), Vec2::new(20.0, 6.0));
        painter.rect(thumb, CornerRadius::same(2), p.accent, Stroke::NONE, egui::StrokeKind::Inside);
        painter.text(rect.center_bottom(), Align2::CENTER_BOTTOM, format_value(def, *value), FontId::proportional(11.0), p.text);

        ui.data_mut(|d| d.insert_temp(id, st));
        if *value != old {
            response.mark_changed();
        }
        let v = *value as f64 - f64::from(def.ofs);
        response.widget_info(|| egui::WidgetInfo::slider(true, v, label));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_the_track_height_sweeps_the_range() {
        assert_eq!(fader_value(0, -100.0, 0, 48, 100.0), 48);
        assert_eq!(fader_value(48, 100.0, 0, 48, 100.0), 0);
        assert_eq!(fader_value(24, -50.0, 0, 48, 100.0), 48);
        assert_eq!(fader_value(24, 10.0, 0, 48, 100.0), 19);
    }
}
