//! What the editor shows for each block of the signal chain. Lists only
//! parameters from `PARAMS` (verified on the amp).

use crate::{find_param, Patch};

/// One parameter as the editor shows it.
#[derive(Debug)]
pub struct Control {
    pub param: &'static str,
    pub label: &'static str,
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
}

impl BlockDef {
    /// `common` followed by the current variation's controls.
    pub fn controls(&self, patch: &Patch) -> Vec<&'static Control> {
        let color = self.color.and_then(find_param).and_then(|p| patch.get(p));
        let variation = color.and_then(|c| self.per_color.get(c as usize)).copied().unwrap_or(&[]);
        self.common.iter().chain(variation).collect()
    }

    /// The current colour's TAP time in ms, for blocks with a TAP light.
    pub fn tap_time(&self, patch: &Patch) -> Option<u32> {
        let color = patch.get(find_param(self.color?)?)?;
        patch.get(find_param(self.tap.get(color as usize)?)?)
    }

    /// Value name of `kind`, e.g. "CRUNCH".
    pub fn type_label(&self, patch: &Patch) -> Option<&'static str> {
        let p = find_param(self.kind?)?;
        let v = patch.get(p)?;
        p.values?.iter().find(|&&(n, _)| n == v).map(|&(_, name)| name)
    }
}

const fn c(param: &'static str, label: &'static str) -> Control {
    Control { param, label }
}

const fn block(id: &'static str, title: &'static str, switch: Option<&'static str>) -> BlockDef {
    BlockDef { id, title, switch, kind: None, color: None, common: &[], per_color: &[], tap: &[] }
}

/// The signal chain in processing order. Fixed for now: the patch's chain
/// setting is not verified yet, so this follows the default order.
pub const CHAIN: &[BlockDef] = &[
    BlockDef {
        color: Some("booster-color"),
        per_color: &[&[c("booster1-drive", "DRIVE")], &[c("booster2-drive", "DRIVE")], &[c("booster3-drive", "DRIVE")]],
        ..block("booster", "BOOSTER", Some("booster-sw"))
    },
    block("mod", "MOD", Some("mod-sw")),
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
    block("fx", "FX", Some("fx-sw")),
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
        common: &[c("reverb1-level", "EFFECT LEVEL (REVERB 1)")],
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
            assert_eq!(b.per_color.len(), n, "{}", b.id);
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
