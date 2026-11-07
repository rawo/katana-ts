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
    pub min: u32,
    pub max: u32,
}

/// What the knob moves for one type.
#[derive(Debug)]
pub struct TypeKnob {
    pub fx_type: u32,
    pub key: &'static str,
    pub targets: &'static [KnobTarget],
}

const fn t(key: &'static str, label: &'static str, offset: [u8; 2], min: u32, max: u32) -> KnobTarget {
    KnobTarget { key, label, offset, min, max }
}

/// Recorded on the amp, with every knob assignment at PRESET.
pub const KNOB: &[TypeKnob] = &[
    // captures/108-mod-colours-knob.log
    TypeKnob {
        fx_type: 23,
        key: "chorus",
        targets: &[t("low-rate", "LOW RATE", [0x01, 0x37], 0, 100), t("high-rate", "HIGH RATE", [0x01, 0x3B], 0, 100)],
    },
    TypeKnob { fx_type: 25, key: "phaser90e", targets: &[t("speed", "SPEED", [0x01, 0x45], 0, 100)] },
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
                    values: None,
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
