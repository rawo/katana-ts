//! Typed parameters. A row is added to `PARAMS` only once a capture confirms
//! its address and encoding (see the discovery loop in captures/README.md).

use crate::device::{self, addr};
use crate::{encode, Address, Command, Patch, ProtoError, Transport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// One byte, 0..=127.
    U7,
    /// Four bytes, 4 bits each, high nibble first (app: INTEGER4x4).
    Nibbles4,
}

impl Encoding {
    fn len(self) -> u32 {
        match self {
            Encoding::U7 => 1,
            Encoding::Nibbles4 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamDef {
    pub name: &'static str,
    /// Linear address.
    pub addr: u32,
    pub encoding: Encoding,
    /// Raw (wire) value range.
    pub min: u32,
    pub max: u32,
    /// Names for enumerated values.
    pub values: Option<&'static [(u32, &'static str)]>,
}

const fn p(name: &'static str, a: [u8; 4], encoding: Encoding, min: u32, max: u32) -> ParamDef {
    ParamDef { name, addr: addr(a), encoding, min, max, values: None }
}

const fn named(def: ParamDef, values: &'static [(u32, &'static str)]) -> ParamDef {
    ParamDef { values: Some(values), ..def }
}

use Encoding::{Nibbles4, U7};

// Value names: official app's panel labels / resource tables (see docs/protocol.md).
const ON_OFF: &[(u32, &str)] = &[(0, "OFF"), (1, "ON")];
const AMP_TYPES: &[(u32, &str)] =
    &[(0, "ACOUSTIC"), (1, "CLEAN"), (2, "PUSHED"), (3, "CRUNCH"), (4, "LEAD"), (5, "BROWN")];
const COLORS: &[(u32, &str)] = &[(0, "GREEN"), (1, "RED"), (2, "YELLOW")];
const CAB_RESONANCE: &[(u32, &str)] = &[(0, "VINTAGE"), (1, "MODERN"), (2, "DEEP")];
const CONTOUR: &[(u32, &str)] = &[(0, "OFF"), (1, "1"), (2, "2"), (3, "3")];

/// Confirmed current-patch parameters. Evidence for every row:
/// captures/100-watch-knobs.log (panel control moved, address changed on the
/// amp); ranges from the official app's table, matching the log where swept.
pub const PARAMS: &[ParamDef] = &[
    // AMP — knobs swept to 100 (bass, middle also to 0)
    p("gain", [0x20, 0x00, 0x06, 0x00], U7, 0, 100),
    p("volume", [0x20, 0x00, 0x06, 0x01], U7, 0, 100),
    p("bass", [0x20, 0x00, 0x06, 0x02], U7, 0, 100),
    p("middle", [0x20, 0x00, 0x06, 0x03], U7, 0, 100),
    p("treble", [0x20, 0x00, 0x06, 0x04], U7, 0, 100),
    p("presence", [0x20, 0x00, 0x06, 0x05], U7, 0, 100),
    named(p("amp-type", [0x20, 0x00, 0x06, 0x07], U7, 0, 5), AMP_TYPES),
    // Effect on/off buttons
    named(p("booster-sw", [0x20, 0x00, 0x08, 0x00], U7, 0, 1), ON_OFF),
    named(p("mod-sw", [0x20, 0x00, 0x08, 0x01], U7, 0, 1), ON_OFF),
    named(p("fx-sw", [0x20, 0x00, 0x08, 0x02], U7, 0, 1), ON_OFF),
    named(p("delay-sw", [0x20, 0x00, 0x08, 0x03], U7, 0, 1), ON_OFF),
    named(p("reverb-sw", [0x20, 0x00, 0x08, 0x05], U7, 0, 1), ON_OFF),
    named(p("solo-sw", [0x20, 0x00, 0x3A, 0x00], U7, 0, 1), ON_OFF),
    // Variation colour (selects BOOSTER(1..3))
    named(p("booster-color", [0x20, 0x00, 0x04, 0x00], U7, 0, 2), COLORS),
    named(p("cab-resonance", [0x20, 0x00, 0x02, 0x01], U7, 0, 2), CAB_RESONANCE),
    // Contour button cycles 0..3 (captures/101-watch-contour.log)
    named(p("contour", [0x20, 0x00, 0x40, 0x00], U7, 0, 3), CONTOUR),
    // Per-variation blocks, as edited by the panel knobs
    p("booster1-drive", [0x20, 0x00, 0x0A, 0x01], U7, 0, 120),
    p("booster2-drive", [0x20, 0x00, 0x0C, 0x01], U7, 0, 120),
    p("booster3-drive", [0x20, 0x00, 0x0E, 0x01], U7, 0, 120),
    p("delay2-time", [0x20, 0x00, 0x2A, 0x01], Nibbles4, 1, 2000),
    p("delay2-feedback", [0x20, 0x00, 0x2A, 0x05], U7, 0, 100),
    p("delay2-level", [0x20, 0x00, 0x2A, 0x07], U7, 0, 120),
    p("reverb1-level", [0x20, 0x00, 0x34, 0x0A], U7, 0, 100),
];

pub fn find_param(name: &str) -> Option<&'static ParamDef> {
    PARAMS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

pub fn encode_param(p: &ParamDef, value: u32) -> Result<Vec<u8>, ProtoError> {
    if !(p.min..=p.max).contains(&value) {
        return Err(ProtoError::OutOfRange { name: p.name, value, min: p.min, max: p.max });
    }
    match p.encoding {
        Encoding::U7 => Ok(vec![value as u8]),
        Encoding::Nibbles4 => Ok((0..4).rev().map(|i| ((value >> (4 * i)) & 0x0F) as u8).collect()),
    }
}

/// Sends one DT1; nothing is sent if the value is invalid.
pub fn set_param(t: &mut dyn Transport, p: &ParamDef, value: u32) -> Result<(), ProtoError> {
    let data = encode_param(p, value)?;
    t.send(&encode(&device::header(), &Command::Dt1 { addr: Address(p.addr), data })?)
}

impl Patch {
    pub fn get(&self, p: &ParamDef) -> Option<u32> {
        let bytes = self.mem.read(Address(p.addr), p.encoding.len())?;
        match p.encoding {
            Encoding::U7 => Some(bytes[0] as u32),
            Encoding::Nibbles4 => Some(bytes.iter().fold(0, |v, &b| (v << 4) | (b & 0x0F) as u32)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{device, encode, Address, Command, FakeAmp, MemoryMap, Patch, ProtoError};

    const P: ParamDef = ParamDef { name: "TEST", addr: 0x10, encoding: Encoding::U7, min: 0, max: 100, values: None };

    #[test]
    fn set_sends_single_dt1() {
        let mut amp = FakeAmp::new(device::header(), 128);
        set_param(&mut amp, &P, 50).unwrap();
        let expected = encode(&device::header(), &Command::Dt1 { addr: Address(0x10), data: vec![50] }).unwrap();
        assert_eq!(amp.sent, vec![expected]);
    }

    #[test]
    fn set_rejects_out_of_range() {
        let mut amp = FakeAmp::new(device::header(), 128);
        assert!(matches!(set_param(&mut amp, &P, 101), Err(ProtoError::OutOfRange { .. })));
        assert!(amp.sent.is_empty());
    }

    #[test]
    fn patch_get_reads_param() {
        let mut mem = MemoryMap::default();
        mem.apply(Address(0x10), &[42]);
        assert_eq!(Patch { mem }.get(&P), Some(42));
    }

    const NIB: ParamDef =
        ParamDef { name: "NIB", addr: 0x20, encoding: Encoding::Nibbles4, min: 1, max: 2000, values: None };

    #[test]
    fn nibbles4_encodes_high_nibble_first() {
        assert_eq!(encode_param(&NIB, 2000).unwrap(), vec![0x00, 0x07, 0x0D, 0x00]);
    }

    #[test]
    fn nibbles4_round_trips_through_patch() {
        let mut mem = MemoryMap::default();
        mem.apply(Address(0x20), &encode_param(&NIB, 489).unwrap());
        assert_eq!(Patch { mem }.get(&NIB), Some(489));
    }

    #[test]
    fn param_names_are_unique() {
        let mut names: Vec<_> = PARAMS.iter().map(|p| p.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), PARAMS.len());
    }

    #[test]
    fn param_values_lie_within_range() {
        for p in PARAMS {
            for &(v, name) in p.values.unwrap_or(&[]) {
                assert!((p.min..=p.max).contains(&v), "{}: {name} = {v} out of range", p.name);
            }
        }
    }
}
