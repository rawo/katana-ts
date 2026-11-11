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
    /// Shown value = raw − `ofs` (app map `ofs`, e.g. 20 for −20..20 dB).
    pub ofs: i32,
    pub format: Format,
}

/// How a shown value (raw − `ofs`) is written, after the official app's
/// display formats (docs/settings.md, "Values").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The number; signed ("+5") when `ofs` is positive.
    Plain,
    /// "+5", "0", "-5".
    Signed,
    /// One more than the value (a 0-based number shown 1-based).
    PlusOne,
    /// The number and a unit, e.g. "400 ms".
    Unit(&'static str),
    /// "+5 dB".
    SignedDb,
    /// Half-dB steps: "+3.5 dB".
    HalfDb,
    /// Tenths of a second: "3.0 s".
    Tenths,
    /// Twice the value, in percent.
    Percent2,
    /// "OFF" below zero, else the number.
    OffBelowZero,
    /// The value's name from `values`.
    Names,
}

const fn p(name: &'static str, a: [u8; 4], encoding: Encoding, min: u32, max: u32) -> ParamDef {
    ParamDef { name, addr: addr(a), encoding, min, max, values: None, ofs: 0, format: Format::Plain }
}

fn signed(v: impl std::fmt::Display + PartialOrd + Default) -> String {
    if v > Default::default() {
        format!("+{v}")
    } else {
        v.to_string()
    }
}

/// The value as shown to the user, e.g. "400 ms", "+3.5 dB", "PLATE".
pub fn format_value(def: &ParamDef, raw: u32) -> String {
    let v = i64::from(raw) - i64::from(def.ofs);
    let half = v as f64 / 2.0;
    match def.format {
        Format::Plain if def.ofs > 0 => signed(v),
        Format::Plain => v.to_string(),
        Format::Signed => signed(v),
        Format::PlusOne => (v + 1).to_string(),
        Format::Unit(u) => format!("{v} {u}"),
        Format::SignedDb => format!("{} dB", signed(v)),
        Format::HalfDb => format!("{}{half:.1} dB", if half > 0.0 { "+" } else { "" }),
        Format::Tenths => format!("{:.1} s", v as f64 / 10.0),
        Format::Percent2 => format!("{} %", v * 2),
        Format::OffBelowZero if v < 0 => "OFF".to_string(),
        Format::OffBelowZero => v.to_string(),
        Format::Names => def
            .values
            .and_then(|vs| vs.iter().find(|&&(n, _)| n == raw))
            .map_or_else(|| format!("? ({raw})"), |&(_, name)| name.to_string()),
    }
}

/// A typed value back to raw: a number as shown (a unit may follow), or a
/// value name. `None` if it doesn't parse or lies outside the range.
pub fn parse_shown(def: &ParamDef, text: &str) -> Option<u32> {
    let text = text.trim();
    if def.format == Format::Names {
        let vs = def.values?;
        return vs.iter().find(|(_, n)| n.eq_ignore_ascii_case(text)).map(|&(v, _)| v);
    }
    let number = text.trim_end_matches(|c: char| c.is_alphabetic() || c == '%' || c.is_whitespace());
    let shown: f64 = number.parse().ok()?;
    let v = match def.format {
        Format::HalfDb => shown * 2.0,
        Format::Tenths => shown * 10.0,
        Format::Percent2 => shown / 2.0,
        Format::PlusOne => shown - 1.0,
        _ => shown,
    };
    if v.fract() != 0.0 {
        return None;
    }
    let raw = v as i64 + i64::from(def.ofs);
    u32::try_from(raw).ok().filter(|r| (def.min..=def.max).contains(r))
}

const fn named(def: ParamDef, values: &'static [(u32, &'static str)]) -> ParamDef {
    ParamDef { values: Some(values), ..def }
}

use Encoding::{Nibbles4, U7};

// Value names: official app's panel labels / resource tables (see docs/protocol.md).
const ON_OFF: &[(u32, &str)] = &[(0, "OFF"), (1, "ON")];
const AMP_TYPES: &[(u32, &str)] =
    &[(0, "ACOUSTIC"), (1, "CLEAN"), (2, "PUSHED"), (3, "CRUNCH"), (4, "LEAD"), (5, "BROWN")];
const COLORS: &[(u32, &str)] = &[(0, "GREEN"), (1, "RED"), (2, "ORANGE")];
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
    // Selects DELAY(1..3) (captures/105-delay-colours-knob.log)
    named(p("delay-color", [0x20, 0x00, 0x04, 0x03], U7, 0, 2), COLORS),
    // Selects REVERB(1..3) (captures/107-reverb-colours-knob.log)
    named(p("reverb-color", [0x20, 0x00, 0x04, 0x04], U7, 0, 2), COLORS),
    // Selects MOD slot FX(1..3) (captures/108-mod-colours-knob.log)
    named(p("mod-color", [0x20, 0x00, 0x04, 0x01], U7, 0, 2), COLORS),
    // Selects FX slot FX(4..6) (captures/109-fx-colours-knob.log)
    named(p("fx-color", [0x20, 0x00, 0x04, 0x02], U7, 0, 2), COLORS),
    named(p("cab-resonance", [0x20, 0x00, 0x02, 0x01], U7, 0, 2), CAB_RESONANCE),
    // Contour button cycles 0..3 (captures/101-watch-contour.log)
    named(p("contour", [0x20, 0x00, 0x40, 0x00], U7, 0, 3), CONTOUR),
    // Per-variation blocks, as edited by the panel knobs
    p("booster1-drive", [0x20, 0x00, 0x0A, 0x01], U7, 0, 120),
    p("booster2-drive", [0x20, 0x00, 0x0C, 0x01], U7, 0, 120),
    p("booster3-drive", [0x20, 0x00, 0x0E, 0x01], U7, 0, 120),
    // DELAY(1..3) = green/red/orange: the knob edits feedback and level, TAP
    // the time (captures/105-delay-colours-knob.log, 106-delay-tap.log)
    p("delay1-time", [0x20, 0x00, 0x28, 0x01], Nibbles4, 1, 2000),
    p("delay1-feedback", [0x20, 0x00, 0x28, 0x05], U7, 0, 100),
    p("delay1-level", [0x20, 0x00, 0x28, 0x07], U7, 0, 120),
    p("delay2-time", [0x20, 0x00, 0x2A, 0x01], Nibbles4, 1, 2000),
    p("delay2-feedback", [0x20, 0x00, 0x2A, 0x05], U7, 0, 100),
    p("delay2-level", [0x20, 0x00, 0x2A, 0x07], U7, 0, 120),
    p("delay3-time", [0x20, 0x00, 0x2C, 0x01], Nibbles4, 1, 2000),
    p("delay3-feedback", [0x20, 0x00, 0x2C, 0x05], U7, 0, 100),
    p("delay3-level", [0x20, 0x00, 0x2C, 0x07], U7, 0, 120),
    // REVERB(1..3) = green/red/orange: the knob edits the level (captures/107)
    p("reverb1-level", [0x20, 0x00, 0x34, 0x0A], U7, 0, 100),
    p("reverb2-level", [0x20, 0x00, 0x36, 0x0A], U7, 0, 100),
    p("reverb3-level", [0x20, 0x00, 0x38, 0x0A], U7, 0, 100),
];

/// `PARAMS`, the generated MOD/FX slot parameters (`modfx`), then every other
/// setting from Tone Studio's tables (`settings`).
pub fn all_params() -> &'static [ParamDef] {
    static ALL: std::sync::LazyLock<Vec<ParamDef>> = std::sync::LazyLock::new(|| {
        PARAMS
            .iter()
            .chain(crate::modfx::GENERATED.iter())
            .chain(crate::settings::GENERATED.iter())
            .copied()
            .collect()
    });
    &ALL
}

pub fn find_param(name: &str) -> Option<&'static ParamDef> {
    all_params().iter().find(|p| p.name.eq_ignore_ascii_case(name))
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

    /// Same range check as `encode_param`; the patch is unchanged on error.
    pub fn set(&mut self, p: &ParamDef, value: u32) -> Result<(), ProtoError> {
        let data = encode_param(p, value)?;
        self.mem.apply(Address(p.addr), &data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{device, encode, Address, Command, FakeAmp, MemoryMap, Patch, ProtoError};

    const P: ParamDef =
        ParamDef { name: "TEST", addr: 0x10, encoding: Encoding::U7, min: 0, max: 100, values: None, ofs: 0, format: Format::Plain };

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
        ParamDef { name: "NIB", addr: 0x20, encoding: Encoding::Nibbles4, min: 1, max: 2000, values: None, ofs: 0, format: Format::Plain };

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
    fn patch_set_then_get_round_trips() {
        let mut p = Patch::default_preset();
        let gain = find_param("gain").unwrap();
        p.set(gain, 77).unwrap();
        assert_eq!(p.get(gain), Some(77));
    }

    #[test]
    fn patch_set_rejects_out_of_range_and_keeps_value() {
        let mut p = Patch::default_preset();
        let gain = find_param("gain").unwrap();
        assert!(matches!(p.set(gain, 101), Err(ProtoError::OutOfRange { .. })));
        assert_eq!(p.get(gain), Some(0));
    }

    #[test]
    fn param_names_are_unique() {
        // `find_param` ignores case and returns the first match.
        let mut names: Vec<_> = all_params().iter().map(|p| p.name.to_ascii_lowercase()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all_params().len());
    }

    #[test]
    fn param_bytes_do_not_overlap() {
        let mut spans: Vec<_> = all_params().iter().map(|p| (p.addr, p.addr + p.encoding.len(), p.name)).collect();
        spans.sort();
        for w in spans.windows(2) {
            assert!(w[0].1 <= w[1].0, "{} and {} share bytes", w[0].2, w[1].2);
        }
    }

    fn def(min: u32, max: u32, ofs: i32, format: Format) -> ParamDef {
        ParamDef { name: "T", addr: 0, encoding: Encoding::U7, min, max, values: None, ofs, format }
    }

    #[test]
    fn formats_show_values_like_tone_studio() {
        assert_eq!(format_value(&def(1, 2000, 0, Format::Unit("ms")), 400), "400 ms");
        // Reverb TIME: raw 0..99 = 0.1..10.0 s.
        assert_eq!(format_value(&def(0, 99, -1, Format::Tenths), 29), "3.0 s");
        assert_eq!(format_value(&def(0, 48, 24, Format::HalfDb), 30), "+3.0 dB");
        assert_eq!(format_value(&def(0, 48, 24, Format::HalfDb), 23), "-0.5 dB");
        assert_eq!(format_value(&def(0, 40, 20, Format::SignedDb), 20), "0 dB");
        assert_eq!(format_value(&def(0, 40, 20, Format::SignedDb), 25), "+5 dB");
        assert_eq!(format_value(&def(0, 100, 50, Format::Plain), 60), "+10");
        assert_eq!(format_value(&def(0, 100, 0, Format::Plain), 60), "60");
        assert_eq!(format_value(&def(0, 3, 0, Format::PlusOne), 0), "1");
        assert_eq!(format_value(&def(0, 50, 0, Format::Percent2), 25), "50 %");
        assert_eq!(format_value(&def(0, 101, 1, Format::OffBelowZero), 0), "OFF");
    }

    #[test]
    fn names_format_shows_the_value_name() {
        let d = ParamDef { values: Some(&[(0, "A"), (1, "B"), (2, "C")]), ..def(0, 2, 0, Format::Names) };
        assert_eq!(format_value(&d, 1), "B");
        assert_eq!(format_value(&d, 7), "? (7)");
        assert_eq!(parse_shown(&d, "c"), Some(2));
    }

    #[test]
    fn typed_values_parse_back() {
        let d = def(0, 48, 24, Format::HalfDb);
        assert_eq!(parse_shown(&d, "+3.5"), Some(31));
        assert_eq!(parse_shown(&d, "-12 dB"), Some(0));
        assert_eq!(parse_shown(&d, "13"), None);
        assert_eq!(parse_shown(&def(1, 2000, 0, Format::Unit("ms")), "651ms"), Some(651));
        assert_eq!(parse_shown(&def(0, 99, -1, Format::Tenths), "3.0"), Some(29));
        assert_eq!(parse_shown(&def(0, 100, 50, Format::Plain), "-10"), Some(40));
        assert_eq!(parse_shown(&def(0, 100, 0, Format::Plain), "abc"), None);
    }

    #[test]
    fn param_values_lie_within_range() {
        for p in all_params() {
            for &(v, name) in p.values.unwrap_or(&[]) {
                assert!((p.min..=p.max).contains(&v), "{}: {name} = {v} out of range", p.name);
            }
        }
    }
}
