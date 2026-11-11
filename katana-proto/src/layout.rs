//! What the editor shows for each block of the signal chain. Lists only
//! parameters from `PARAMS` (verified on the amp).

use crate::{find_param, modfx, Patch};

/// One parameter as the editor shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    pub param: &'static str,
    pub label: &'static str,
    /// The amp's panel knob also moves this setting (PRESET assignment).
    pub knob: bool,
}

/// A block of the signal chain.
#[derive(Debug)]
pub struct BlockDef {
    pub id: &'static str,
    pub title: &'static str,
    /// On/off parameter.
    pub switch: Option<&'static str>,
    /// Parameter whose value name is the block's type line.
    pub kind: Option<&'static str>,
    /// Variation colour parameter; selects an entry of `per_color`.
    pub color: Option<&'static str>,
    pub common: &'static [Control],
    /// Indexed by the colour value.
    pub per_color: &'static [&'static [Control]],
    /// Delay time the TAP light blinks at, indexed by the colour value.
    pub tap: &'static [&'static str],
    /// MOD/FX slot (`modfx::SLOTS` index), indexed by the colour value: the
    /// slot's type names the block and picks the knob's settings.
    pub slots: &'static [usize],
}

impl BlockDef {
    /// `common` followed by the current variation's controls.
    pub fn controls(&self, patch: &Patch) -> Vec<Control> {
        let color = self.color.and_then(find_param).and_then(|p| patch.get(p));
        let variation = color.and_then(|c| self.per_color.get(c as usize)).copied().unwrap_or(&[]);
        let mut out: Vec<Control> = self.common.iter().chain(variation).copied().collect();
        if let Some((slot, fx_type)) = self.slot_type(patch) {
            out.extend(modfx::knob_targets(slot, fx_type).into_iter().map(|(param, label)| Control { param, label, knob: true }));
        }
        out
    }

    /// The current colour's MOD/FX slot and its type value.
    fn slot_type(&self, patch: &Patch) -> Option<(usize, u32)> {
        let color = patch.get(find_param(self.color?)?)?;
        let slot = *self.slots.get(color as usize)?;
        let def = find_param(&format!("{}-type", modfx::SLOTS[slot]))?;
        Some((slot, patch.get(def)?))
    }

    /// The current colour's TAP time in ms, for blocks with a TAP light.
    pub fn tap_time(&self, patch: &Patch) -> Option<u32> {
        let color = patch.get(find_param(self.color?)?)?;
        patch.get(find_param(self.tap.get(color as usize)?)?)
    }

    /// Value name of `kind`, e.g. "CRUNCH", or of the current slot's type.
    pub fn type_label(&self, patch: &Patch) -> Option<&'static str> {
        if let Some((_, v)) = self.slot_type(patch) {
            return modfx::FX_TYPES.iter().find(|&&(n, _)| n == v).map(|&(_, name)| name);
        }
        let p = find_param(self.kind?)?;
        let v = patch.get(p)?;
        p.values?.iter().find(|&&(n, _)| n == v).map(|&(_, name)| name)
    }
}

const fn c(param: &'static str, label: &'static str) -> Control {
    Control { param, label, knob: false }
}

const fn block(id: &'static str, title: &'static str, switch: Option<&'static str>) -> BlockDef {
    BlockDef { id, title, switch, kind: None, color: None, common: &[], per_color: &[], tap: &[], slots: &[] }
}

/// The signal chain in processing order. Fixed for now: the patch's chain
/// setting is not verified yet, so this follows the default order.
pub const CHAIN: &[BlockDef] = &[
    BlockDef {
        color: Some("booster-color"),
        per_color: &[&[c("booster1-drive", "DRIVE")], &[c("booster2-drive", "DRIVE")], &[c("booster3-drive", "DRIVE")]],
        ..block("booster", "BOOSTER", Some("booster-sw"))
    },
    BlockDef { color: Some("mod-color"), slots: &[0, 1, 2], ..block("mod", "MOD", Some("mod-sw")) },
    BlockDef {
        kind: Some("amp-type"),
        common: &[
            c("amp-type", "TYPE"),
            c("cab-resonance", "CAB RESONANCE"),
            c("contour", "CONTOUR"),
            c("solo-sw", "SOLO"),
        ],
        ..block("amp", "AMP", None)
    },
    BlockDef { color: Some("fx-color"), slots: &[3, 4, 5], ..block("fx", "FX", Some("fx-sw")) },
    BlockDef {
        color: Some("delay-color"),
        per_color: &[
            &[c("delay1-time", "TIME"), c("delay1-feedback", "FEEDBACK"), c("delay1-level", "EFFECT LEVEL")],
            &[c("delay2-time", "TIME"), c("delay2-feedback", "FEEDBACK"), c("delay2-level", "EFFECT LEVEL")],
            &[c("delay3-time", "TIME"), c("delay3-feedback", "FEEDBACK"), c("delay3-level", "EFFECT LEVEL")],
        ],
        tap: &["delay1-time", "delay2-time", "delay3-time"],
        ..block("delay", "DELAY", Some("delay-sw"))
    },
    BlockDef {
        color: Some("reverb-color"),
        per_color: &[
            &[c("reverb1-level", "EFFECT LEVEL")],
            &[c("reverb2-level", "EFFECT LEVEL")],
            &[c("reverb3-level", "EFFECT LEVEL")],
        ],
        ..block("reverb", "REVERB", Some("reverb-sw"))
    },
];

/// Always-visible amp knobs.
pub const AMP_STRIP: &[Control] = &[
    c("gain", "GAIN"),
    c("volume", "VOLUME"),
    c("bass", "BASS"),
    c("middle", "MIDDLE"),
    c("treble", "TREBLE"),
    c("presence", "PRESENCE"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_names_a_verified_param() {
        let all = CHAIN.iter().flat_map(|b| b.common.iter().chain(b.per_color.iter().flat_map(|c| c.iter())));
        for c in all.chain(AMP_STRIP) {
            assert!(find_param(c.param).is_some(), "{} not in PARAMS", c.param);
        }
        for b in CHAIN {
            for p in [b.switch, b.kind, b.color].into_iter().flatten().chain(b.tap.iter().copied()) {
                assert!(find_param(p).is_some(), "{p} not in PARAMS");
            }
        }
    }

    #[test]
    fn per_color_has_one_entry_per_colour_value() {
        for b in CHAIN.iter().filter(|b| b.color.is_some()) {
            let n = find_param(b.color.unwrap()).unwrap().values.unwrap().len();
            // Either fixed controls or a MOD/FX slot for every colour.
            let per = if b.slots.is_empty() { b.per_color.len() } else { b.slots.len() };
            assert_eq!(per, n, "{}", b.id);
        }
    }

    #[test]
    fn booster_controls_follow_colour() {
        let booster = CHAIN.iter().find(|b| b.id == "booster").unwrap();
        let mut p = Patch::default_preset();
        p.set(find_param("booster-color").unwrap(), 2).unwrap();
        assert_eq!(booster.controls(&p)[0].param, "booster3-drive");
    }

    #[test]
    fn delay_controls_follow_colour() {
        // captures/105: green edits DELAY(1) 28 xx, red DELAY(2) 2A xx, orange DELAY(3) 2C xx.
        let delay = CHAIN.iter().find(|b| b.id == "delay").unwrap();
        let color = find_param("delay-color").unwrap();
        assert_eq!(color.addr, crate::device::addr([0x20, 0x00, 0x04, 0x03]));
        let mut p = Patch::default_preset();
        for (v, n, block) in [(0, 1, 0x28), (1, 2, 0x2A), (2, 3, 0x2C)] {
            p.set(color, v).unwrap();
            let names: Vec<_> = delay.controls(&p).iter().map(|c| c.param).collect();
            let want = [format!("delay{n}-time"), format!("delay{n}-feedback"), format!("delay{n}-level")];
            assert_eq!(names, want, "colour {v}");
            let addrs: Vec<_> = names.iter().map(|n| find_param(n).unwrap().addr).collect();
            let want: Vec<_> =
                [0x01, 0x05, 0x07].iter().map(|&o| crate::device::addr([0x20, 0x00, block, o])).collect();
            assert_eq!(addrs, want, "colour {v}");
        }
    }

    #[test]
    fn reverb_level_follows_colour() {
        // captures/107: green edits REVERB(1) 34 0A, red REVERB(2) 36 0A, orange REVERB(3) 38 0A.
        let reverb = CHAIN.iter().find(|b| b.id == "reverb").unwrap();
        let color = find_param("reverb-color").unwrap();
        assert_eq!(color.addr, crate::device::addr([0x20, 0x00, 0x04, 0x04]));
        let mut p = Patch::default_preset();
        for (v, n, block) in [(0, 1, 0x34), (1, 2, 0x36), (2, 3, 0x38)] {
            p.set(color, v).unwrap();
            let names: Vec<_> = reverb.controls(&p).iter().map(|c| c.param).collect();
            assert_eq!(names, [format!("reverb{n}-level")], "colour {v}");
            let def = find_param(names[0]).unwrap();
            assert_eq!(def.addr, crate::device::addr([0x20, 0x00, block, 0x0A]), "colour {v}");
        }
    }

    #[test]
    fn mod_controls_follow_colour_and_type() {
        // captures/108: the knob in each colour, with the patch's own types.
        let m = CHAIN.iter().find(|b| b.id == "mod").unwrap();
        let color = find_param("mod-color").unwrap();
        assert_eq!(color.addr, crate::device::addr([0x20, 0x00, 0x04, 0x01]));
        let a = |b: [u8; 4]| crate::device::addr(b);
        let mut p = Patch::default_preset();
        let cases = [
            (
                0,
                "mod1-type",
                23,
                "CHORUS",
                vec![
                    ("mod1-chorus-low-rate", a([0x20, 0, 0x1D, 0x37])),
                    ("mod1-chorus-high-rate", a([0x20, 0, 0x1D, 0x3B])),
                ],
            ),
            (1, "mod2-type", 25, "PHASER 90E", vec![("mod2-phaser90e-speed", a([0x20, 0, 0x1F, 0x45]))]),
            (2, "mod3-type", 26, "FLANGER 117E", vec![("mod3-flanger117e-speed", a([0x20, 0, 0x21, 0x48]))]),
        ];
        for (c, type_param, t, label, want) in cases {
            p.set(color, c).unwrap();
            p.set(find_param(type_param).unwrap(), t).unwrap();
            assert_eq!(m.type_label(&p), Some(label), "colour {c}");
            let got: Vec<_> = m.controls(&p).iter().map(|c| (c.param, find_param(c.param).unwrap().addr)).collect();
            assert_eq!(got, want, "colour {c}");
        }
    }

    #[test]
    fn fx_controls_follow_colour_and_type() {
        // captures/109: FX green/red/orange = FX(4/5/6), FX_DETAIL at 22/24/26 00.
        let fx = CHAIN.iter().find(|b| b.id == "fx").unwrap();
        let color = find_param("fx-color").unwrap();
        assert_eq!(color.addr, crate::device::addr([0x20, 0x00, 0x04, 0x02]));
        let a = |b: [u8; 4]| crate::device::addr(b);
        let mut p = Patch::default_preset();
        let cases = [
            (0, "fx1-type", 16, "TREMOLO", vec![("fx1-tremolo-rate", a([0x20, 0, 0x23, 0x19]))]),
            (1, "fx2-type", 0, "T.WAH", vec![("fx2-twah-sens", a([0x20, 0, 0x24, 0x02]))]),
            (2, "fx3-type", 26, "FLANGER 117E", vec![("fx3-flanger117e-speed", a([0x20, 0, 0x27, 0x48]))]),
        ];
        for (c, type_param, t, label, want) in cases {
            p.set(color, c).unwrap();
            p.set(find_param(type_param).unwrap(), t).unwrap();
            assert_eq!(fx.type_label(&p), Some(label), "colour {c}");
            let got: Vec<_> = fx.controls(&p).iter().map(|c| (c.param, find_param(c.param).unwrap().addr)).collect();
            assert_eq!(got, want, "colour {c}");
        }
    }

    #[test]
    fn signed_settings_carry_their_offset() {
        // App map: GEQ_500Hz ofs 20, -20..20 (sent as 0..40).
        let g = find_param("mod1-geq-500hz").unwrap();
        assert_eq!(g.addr, crate::device::addr([0x20, 0, 0x1C, 0x23]));
        assert_eq!((g.min, g.max, g.ofs), (0, 40, 20));
        assert_eq!(find_param("gain").unwrap().ofs, 0);
    }

    #[test]
    fn type_batch_2_knob_settings() {
        // captures/111-types-batch2.log (MOD green), offsets and ranges from the
        // app's address map, value names from its layout.
        let a = |o: u8| crate::device::addr([0x20, 0, 0x1C, o]);
        let cases = [
            (7, "mod1-guitarsim-type", a(0x35), (0, 7, 0)),
            (8, "mod1-slowgear-rise-time", a(0x3B), (0, 100, 0)),
            (9, "mod1-wavesynth-resonance", a(0x3F), (0, 100, 0)),
            (10, "mod1-octave-effect-level", a(0x46), (0, 100, 0)),
            (11, "mod1-pitchshift-pitch1", a(0x4A), (0, 48, 24)),
            (12, "mod1-harmonist-harmony1", a(0x5C), (0, 29, 0)),
        ];
        for (t, name, addr, range) in cases {
            let d = find_param(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(d.addr, addr, "{name}");
            assert_eq!((d.min, d.max, d.ofs), range, "{name}");
            assert_eq!(crate::modfx::knob_targets(0, t).iter().map(|x| x.0).collect::<Vec<_>>(), [name]);
        }
        let gs = find_param("fx2-guitarsim-type").unwrap().values.unwrap();
        assert_eq!(gs.len(), 8);
        assert_eq!((gs[0], gs[7]), ((0, "S->H"), (7, "P->AC")));
        let hr = find_param("mod1-harmonist-harmony1").unwrap().values.unwrap();
        assert_eq!(hr.len(), 30);
        assert_eq!((hr[0], hr[14], hr[28], hr[29]), ((0, "-2oct"), (14, "UNISON"), (28, "+2oct"), (29, "USER")));
        assert!(find_param("mod1-slowgear-rise-time").unwrap().values.is_none());
    }

    #[test]
    fn type_batch_3_knob_settings() {
        // captures/112-types-batch3.log (MOD green); FX_DETAIL 01 xx = 20 00 1D xx.
        type Want = (&'static str, u32, (u32, u32, i32));
        let a = |o: u8| crate::device::addr([0x20, 0, 0x1D, o]);
        let cases: [(u32, &[Want]); 6] = [
            (
                13,
                &[
                    ("mod1-acprocess-treble", a(0x06), (0, 100, 50)),
                    ("mod1-acprocess-presence", a(0x07), (0, 100, 50)),
                    ("mod1-acprocess-level", a(0x08), (0, 100, 0)),
                ],
            ),
            (14, &[("mod1-phaser-rate", a(0x0A), (0, 100, 0))]),
            (15, &[("mod1-flanger-rate", a(0x11), (0, 100, 0))]),
            (17, &[("mod1-rotary-rate", a(0x1C), (0, 100, 0))]),
            (18, &[("mod1-univ-rate", a(0x1F), (0, 100, 0))]),
            (19, &[("mod1-slicer-rate", a(0x23), (0, 100, 0))]),
        ];
        for (t, want) in cases {
            for &(name, addr, range) in want {
                let d = find_param(name).unwrap_or_else(|| panic!("{name}"));
                assert_eq!(d.addr, addr, "{name}");
                assert_eq!((d.min, d.max, d.ofs), range, "{name}");
            }
            let got: Vec<_> = crate::modfx::knob_targets(0, t).iter().map(|x| x.0).collect();
            assert_eq!(got, want.iter().map(|w| w.0).collect::<Vec<_>>(), "type {t}");
        }
    }

    #[test]
    fn type_batch_4_knob_settings() {
        // captures/113-types-batch4.log (MOD green); FX_DETAIL 01 xx = 20 00 1D xx.
        type Want = (&'static str, u32, (u32, u32, i32));
        let a = |o: u8| crate::device::addr([0x20, 0, 0x1D, o]);
        let cases: [(u32, &[Want]); 8] = [
            (20, &[("mod1-vibrato-rate", a(0x27), (0, 100, 0))]),
            (21, &[("mod1-ringmod-frequency", a(0x2B), (0, 100, 0))]),
            (22, &[("mod1-humanizer-rate", a(0x32), (0, 100, 0))]),
            (24, &[("mod1-acsim-body", a(0x41), (0, 100, 0)), ("mod1-acsim-low", a(0x42), (0, 100, 50))]),
            (27, &[("mod1-wah95e-pedal-max", a(0x4C), (0, 100, 0))]),
            (
                28,
                &[
                    ("mod1-dc30-chorus-intensity", a(0x51), (0, 100, 0)),
                    ("mod1-dc30-echo-volume", a(0x57), (0, 100, 0)),
                ],
            ),
            (29, &[("mod1-heavyoctave-1oct-level", a(0x5A), (0, 100, 0))]),
            (30, &[("mod1-pedalbend-pitch", a(0x5D), (0, 48, 24))]),
        ];
        for (t, want) in cases {
            for &(name, addr, range) in want {
                let d = find_param(name).unwrap_or_else(|| panic!("{name}"));
                assert_eq!(d.addr, addr, "{name}");
                assert_eq!((d.min, d.max, d.ofs), range, "{name}");
            }
            let got: Vec<_> = crate::modfx::knob_targets(0, t).iter().map(|x| x.0).collect();
            assert_eq!(got, want.iter().map(|w| w.0).collect::<Vec<_>>(), "type {t}");
        }
    }

    #[test]
    fn knob_table_lists_each_type_once_in_order() {
        let types: Vec<u32> = crate::modfx::KNOB.iter().map(|k| k.fx_type).collect();
        assert_eq!(types, (0..=30).collect::<Vec<_>>());
    }

    #[test]
    fn every_modfx_type_has_knob_settings_in_every_slot() {
        for slot in 0..6 {
            for &(t, name) in crate::modfx::FX_TYPES {
                assert!(!crate::modfx::knob_targets(slot, t).is_empty(), "slot {slot}, {name}");
            }
        }
        let m = CHAIN.iter().find(|b| b.id == "mod").unwrap();
        let mut p = Patch::default_preset();
        p.set(find_param("mod1-type").unwrap(), 30).unwrap();
        assert_eq!(m.type_label(&p), Some("PEDAL BEND"));
        assert_eq!(m.controls(&p).len(), 1);
    }

    #[test]
    fn modfx_type_params_sit_in_the_fx_blocks() {
        let a = |b: [u8; 4]| crate::device::addr(b);
        for (name, blk) in [("mod1", 0x10), ("mod2", 0x12), ("mod3", 0x14), ("fx1", 0x16), ("fx2", 0x18), ("fx3", 0x1A)]
        {
            let t = find_param(&format!("{name}-type")).unwrap();
            assert_eq!(t.addr, a([0x20, 0, blk, 0]), "{name}");
            assert_eq!((t.min, t.max, t.values.unwrap().len()), (0, 30, 31), "{name}");
        }
    }

    #[test]
    fn tap_time_is_the_current_colours_delay_time() {
        let delay = CHAIN.iter().find(|b| b.id == "delay").unwrap();
        let mut p = Patch::default_preset();
        p.set(find_param("delay-color").unwrap(), 1).unwrap();
        p.set(find_param("delay2-time").unwrap(), 526).unwrap();
        p.set(find_param("delay3-time").unwrap(), 1186).unwrap();
        assert_eq!(delay.tap_time(&p), Some(526));
        let booster = CHAIN.iter().find(|b| b.id == "booster").unwrap();
        assert_eq!(booster.tap_time(&p), None);
    }

    #[test]
    fn amp_type_label_comes_from_value_names() {
        let amp = CHAIN.iter().find(|b| b.id == "amp").unwrap();
        assert_eq!(amp.type_label(&Patch::default_preset()), Some("CLEAN"));
    }

    #[test]
    fn block_ids_are_unique() {
        let mut ids: Vec<_> = CHAIN.iter().map(|b| b.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CHAIN.len());
    }
}
