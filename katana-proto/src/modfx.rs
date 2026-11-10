//! MOD and FX share six effect slots, `FX(1..6)`: MOD green/red/orange are
//! slots 1..3, FX green/red/orange slots 4..6. Each slot has a type
//! (`FX(n)` block) and its settings (`FX_DETAIL(n)` block), where every type
//! keeps its own settings at a fixed offset.
//!
//! The panel knob of MOD and FX moves, per type, the settings in `KNOB`
//! (the knob's default "PRESET" assignment). Only types recorded on the amp
//! are listed; parameters are generated for every slot.

use std::sync::LazyLock;

use crate::device::addr;
use crate::params::{Encoding, ParamDef};

/// Value names of the type parameter (official app's `resource.js` order;
/// matches the knob captures, captures/108).
pub const FX_TYPES: &[(u32, &str)] = &[
    (0, "T.WAH"),
    (1, "AUTO WAH"),
    (2, "PEDAL WAH"),
    (3, "COMP"),
    (4, "LIMITER"),
    (5, "GRAPHIC EQ"),
    (6, "PARAMETRIC EQ"),
    (7, "GUITAR SIM"),
    (8, "SLOW GEAR"),
    (9, "WAVE SYNTH"),
    (10, "OCTAVE"),
    (11, "PITCH SHIFTER"),
    (12, "HARMONIST"),
    (13, "AC.PROCESSOR"),
    (14, "PHASER"),
    (15, "FLANGER"),
    (16, "TREMOLO"),
    (17, "ROTARY"),
    (18, "UNI-V"),
    (19, "SLICER"),
    (20, "VIBRATO"),
    (21, "RING MOD"),
    (22, "HUMANIZER"),
    (23, "CHORUS"),
    (24, "AC.GUITAR SIM"),
    (25, "PHASER 90E"),
    (26, "FLANGER 117E"),
    (27, "WAH 95E"),
    (28, "DC-30"),
    (29, "HEAVY OCTAVE"),
    (30, "PEDAL BEND"),
];

/// Slot name prefixes, slot index 0..6.
pub const SLOTS: [&str; 6] = ["mod1", "mod2", "mod3", "fx1", "fx2", "fx3"];

/// One setting the knob moves.
#[derive(Debug)]
pub struct KnobTarget {
    pub key: &'static str,
    pub label: &'static str,
    /// Offset inside `FX_DETAIL(n)` as the app's address map writes it (7-bit packed).
    pub offset: [u8; 2],
    /// Raw range.
    pub min: u32,
    pub max: u32,
    /// Shown value = raw − `ofs`.
    pub ofs: u32,
    /// Value names, for settings that pick from a list.
    pub values: Option<&'static [(u32, &'static str)]>,
}

/// What the knob moves for one type.
#[derive(Debug)]
pub struct TypeKnob {
    pub fx_type: u32,
    pub key: &'static str,
    pub targets: &'static [KnobTarget],
}

const fn t(key: &'static str, label: &'static str, offset: [u8; 2], min: u32, max: u32) -> KnobTarget {
    KnobTarget { key, label, offset, min, max, ofs: 0, values: None }
}

/// A signed setting shown as −`ofs`..+`ofs`, sent as 0..2·`ofs`.
const fn signed(key: &'static str, label: &'static str, offset: [u8; 2], ofs: u32) -> KnobTarget {
    KnobTarget { key, label, offset, min: 0, max: 2 * ofs, ofs, values: None }
}

/// A setting that picks one of `values` (raw 0..len−1).
const fn list(
    key: &'static str,
    label: &'static str,
    offset: [u8; 2],
    values: &'static [(u32, &'static str)],
) -> KnobTarget {
    KnobTarget { key, label, offset, min: 0, max: values.len() as u32 - 1, ofs: 0, values: Some(values) }
}

/// GUITAR SIM TYPE (app layout; "->" because egui's default font has no arrow).
const GUITARSIM_TYPES: &[(u32, &str)] = &[
    (0, "S->H"),
    (1, "H->S"),
    (2, "H->HF"),
    (3, "S->HOLLOW"),
    (4, "H->HOLLOW"),
    (5, "S->AC"),
    (6, "H->AC"),
    (7, "P->AC"),
];

/// HARMONIST HARMONY (app layout).
const HARMONIES: &[(u32, &str)] = &[
    (0, "-2oct"),
    (1, "-14th"),
    (2, "-13th"),
    (3, "-12th"),
    (4, "-11th"),
    (5, "-10th"),
    (6, "-9th"),
    (7, "-1oct"),
    (8, "-7th"),
    (9, "-6th"),
    (10, "-5th"),
    (11, "-4th"),
    (12, "-3rd"),
    (13, "-2nd"),
    (14, "UNISON"),
    (15, "+2nd"),
    (16, "+3rd"),
    (17, "+4th"),
    (18, "+5th"),
    (19, "+6th"),
    (20, "+7th"),
    (21, "+1oct"),
    (22, "+9th"),
    (23, "+10th"),
    (24, "+11th"),
    (25, "+12th"),
    (26, "+13th"),
    (27, "+14th"),
    (28, "+2oct"),
    (29, "USER"),
];

/// Recorded on the amp, with every knob assignment at PRESET.
pub const KNOB: &[TypeKnob] = &[
    // captures/109-fx-colours-knob.log
    TypeKnob { fx_type: 0, key: "twah", targets: &[t("sens", "SENS", [0x00, 0x02], 0, 100)] },
    // captures/110-types-batch1.log (MOD green)
    TypeKnob { fx_type: 1, key: "awah", targets: &[t("rate", "RATE", [0x00, 0x0A], 0, 100)] },
    TypeKnob { fx_type: 2, key: "pedalwah", targets: &[t("pedal-max", "PEDAL MAX", [0x00, 0x11], 0, 100)] },
    TypeKnob {
        fx_type: 3,
        key: "comp",
        targets: &[t("sustain", "SUSTAIN", [0x00, 0x15], 0, 100), t("level", "LEVEL", [0x00, 0x18], 0, 100)],
    },
    // Ratio steps 0..17; the app's value names (1:1 …) are not found yet.
    TypeKnob { fx_type: 4, key: "limiter", targets: &[t("ratio", "RATIO", [0x00, 0x1C], 0, 17)] },
    TypeKnob {
        fx_type: 5,
        key: "geq",
        targets: &[signed("500hz", "500Hz", [0x00, 0x23], 20), signed("1khz", "1kHz", [0x00, 0x24], 20)],
    },
    TypeKnob {
        fx_type: 6,
        key: "peq",
        targets: &[signed("lowmid-gain", "LOW-MID GAIN", [0x00, 0x2E], 20), signed("level", "LEVEL", [0x00, 0x34], 20)],
    },
    // captures/111-types-batch2.log (MOD green); the knob covers only part of
    // some ranges (GUITAR SIM S->H..H->HF, RISE TIME/RESONANCE 0..80,
    // PITCH1 -12..+12, HARMONY1 -6th..+6th).
    TypeKnob { fx_type: 7, key: "guitarsim", targets: &[list("type", "TYPE", [0x00, 0x35], GUITARSIM_TYPES)] },
    TypeKnob { fx_type: 8, key: "slowgear", targets: &[t("rise-time", "RISE TIME", [0x00, 0x3B], 0, 100)] },
    TypeKnob { fx_type: 9, key: "wavesynth", targets: &[t("resonance", "RESONANCE", [0x00, 0x3F], 0, 100)] },
    TypeKnob { fx_type: 10, key: "octave", targets: &[t("effect-level", "EFFECT LEVEL", [0x00, 0x46], 0, 100)] },
    TypeKnob { fx_type: 11, key: "pitchshift", targets: &[signed("pitch1", "PITCH 1", [0x00, 0x4A], 24)] },
    TypeKnob { fx_type: 12, key: "harmonist", targets: &[list("harmony1", "HARMONY 1", [0x00, 0x5C], HARMONIES)] },
    // captures/112-types-batch3.log (MOD green); AC.PROCESSOR's knob moves three
    // settings (TREBLE -17..+24, PRESENCE 0..+48, LEVEL 37..48 seen).
    TypeKnob {
        fx_type: 13,
        key: "acprocess",
        targets: &[
            signed("treble", "TREBLE", [0x01, 0x06], 50),
            signed("presence", "PRESENCE", [0x01, 0x07], 50),
            t("level", "LEVEL", [0x01, 0x08], 0, 100),
        ],
    },
    TypeKnob { fx_type: 14, key: "phaser", targets: &[t("rate", "RATE", [0x01, 0x0A], 0, 100)] },
    TypeKnob { fx_type: 15, key: "flanger", targets: &[t("rate", "RATE", [0x01, 0x11], 0, 100)] },
    TypeKnob { fx_type: 17, key: "rotary", targets: &[t("rate", "RATE", [0x01, 0x1C], 0, 100)] },
    TypeKnob { fx_type: 18, key: "univ", targets: &[t("rate", "RATE", [0x01, 0x1F], 0, 100)] },
    TypeKnob { fx_type: 19, key: "slicer", targets: &[t("rate", "RATE", [0x01, 0x23], 0, 100)] },
    TypeKnob { fx_type: 16, key: "tremolo", targets: &[t("rate", "RATE", [0x01, 0x19], 0, 100)] },
    // captures/108-mod-colours-knob.log
    TypeKnob {
        fx_type: 23,
        key: "chorus",
        targets: &[t("low-rate", "LOW RATE", [0x01, 0x37], 0, 100), t("high-rate", "HIGH RATE", [0x01, 0x3B], 0, 100)],
    },
    TypeKnob { fx_type: 25, key: "phaser90e", targets: &[t("speed", "SPEED", [0x01, 0x45], 0, 100)] },
    // MOD (108) and FX (109): the same offset in both
    TypeKnob { fx_type: 26, key: "flanger117e", targets: &[t("speed", "SPEED", [0x01, 0x48], 0, 100)] },
];

fn block(first: u8, slot: usize) -> u32 {
    addr([0x20, 0x00, first + 2 * slot as u8, 0x00])
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// `<slot>-type` and `<slot>-<type>-<setting>` for every slot.
pub(crate) static GENERATED: LazyLock<Vec<ParamDef>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for (i, slot) in SLOTS.iter().enumerate() {
        out.push(ParamDef {
            name: leak(format!("{slot}-type")),
            addr: block(0x10, i),
            encoding: Encoding::U7,
            min: 0,
            max: 30,
            values: Some(FX_TYPES),
            ofs: 0,
        });
        for k in KNOB {
            for tg in k.targets {
                let ofs = u32::from(tg.offset[0]) << 7 | u32::from(tg.offset[1]);
                out.push(ParamDef {
                    name: leak(format!("{slot}-{}-{}", k.key, tg.key)),
                    addr: block(0x1C, i) + ofs,
                    encoding: Encoding::U7,
                    min: tg.min,
                    max: tg.max,
                    values: tg.values,
                    ofs: tg.ofs,
                });
            }
        }
    }
    out
});

/// The knob's settings for `fx_type` in slot `slot` (0..6), as (param, label).
pub fn knob_targets(slot: usize, fx_type: u32) -> Vec<(&'static str, &'static str)> {
    let Some(k) = KNOB.iter().find(|k| k.fx_type == fx_type) else { return Vec::new() };
    k.targets
        .iter()
        .filter_map(|tg| {
            let name = format!("{}-{}-{}", SLOTS.get(slot)?, k.key, tg.key);
            GENERATED.iter().find(|p| p.name == name).map(|p| (p.name, tg.label))
        })
        .collect()
}
