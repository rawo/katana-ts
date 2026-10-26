//! Colour themes. Dark is the default; colours follow the layout mockup.

use eframe::egui::{self, Color32, Stroke};

pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub line: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub accent: Color32,
    pub green: Color32,
    pub red: Color32,
    pub orange: Color32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeId {
    Dark,
    Light,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl ThemeId {
    pub const ALL: [ThemeId; 2] = [ThemeId::Dark, ThemeId::Light];

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            ThemeId::Dark => Palette {
                dark: true,
                bg: rgb(0x16181c),
                panel: rgb(0x1f2329),
                line: rgb(0x333a44),
                text: rgb(0xd8dde4),
                dim: rgb(0x8a93a0),
                accent: rgb(0xe0a030),
                green: rgb(0x3fbf5a),
                red: rgb(0xe0443c),
                orange: rgb(0xf08a24),
            },
            ThemeId::Light => Palette {
                dark: false,
                bg: rgb(0xf4f5f7),
                panel: rgb(0xe6e8ec),
                line: rgb(0xc4c9d1),
                text: rgb(0x1d2127),
                dim: rgb(0x5b6470),
                accent: rgb(0xb56f00),
                green: rgb(0x23893c),
                red: rgb(0xc22a22),
                orange: rgb(0xc8620a),
            },
        }
    }
}

/// Makes `p` egui's style.
pub fn apply(ctx: &egui::Context, p: &Palette) {
    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    let mut v = if p.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.panel;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.faint_bg_color = p.panel;
    v.extreme_bg_color = if p.dark { rgb(0x111214) } else { Color32::WHITE };
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0, p.bg);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    ctx.set_theme(theme);
    ctx.set_visuals(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_theme_has_readable_text() {
        for t in ThemeId::ALL {
            let p = t.palette();
            assert!(contrast(p.text, p.bg) >= 4.5, "{} text", t.name());
            assert!(contrast(p.text, p.panel) >= 4.5, "{} text on panel", t.name());
            assert!(contrast(p.dim, p.panel) >= 3.0, "{} dim text", t.name());
        }
    }
}
