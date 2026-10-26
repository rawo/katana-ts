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
}

impl BlockDef {
    /// `common` followed by the current variation's controls.
    pub fn controls(&self, patch: &Patch) -> Vec<&'static Control> {
        let color = self.color.and_then(find_param).and_then(|p| patch.get(p));
        let variation = color.and_then(|c| self.per_color.get(c as usize)).copied().unwrap_or(&[]);
        self.common.iter().chain(variation).collect()
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
    BlockDef { id, title, switch, kind: None, color: None, common: &[], per_color: &[] }
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
        common: &[
            c("delay2-time", "TIME (DELAY 2)"),
            c("delay2-feedback", "FEEDBACK (DELAY 2)"),
            c("delay2-level", "EFFECT LEVEL (DELAY 2)"),
        ],
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
            for p in [b.switch, b.kind, b.color].into_iter().flatten() {
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
