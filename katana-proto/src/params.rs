//! Typed parameters. A row is added to `PARAMS` only once a capture confirms
//! its address and encoding (see the discovery loop in captures/README.md).

use crate::{device, encode, Address, Command, Patch, ProtoError, Transport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// One byte, 0..=127.
    U7,
}

impl Encoding {
    fn len(self) -> u32 {
        match self {
            Encoding::U7 => 1,
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

pub const PARAMS: &[ParamDef] = &[];

pub fn find_param(name: &str) -> Option<&'static ParamDef> {
    PARAMS.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

pub fn encode_param(p: &ParamDef, value: u32) -> Result<Vec<u8>, ProtoError> {
    if !(p.min..=p.max).contains(&value) {
        return Err(ProtoError::OutOfRange { name: p.name, value, min: p.min, max: p.max });
    }
    match p.encoding {
        Encoding::U7 => Ok(vec![value as u8]),
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

    #[test]
    fn param_values_lie_within_range() {
        for p in PARAMS {
            for &(v, name) in p.values.unwrap_or(&[]) {
                assert!((p.min..=p.max).contains(&v), "{}: {name} = {v} out of range", p.name);
            }
        }
    }
}
