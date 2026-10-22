//! The current patch: every byte of `device::PATCH_BLOCKS`.

use std::time::{Duration, Instant};

use crate::device::{self, MAX_DT1_DATA, PATCH_BLOCKS, SEND_INTERVAL};
use crate::{decode, encode, read_syx, write_syx, Address, Command, MemoryMap, ProtoError, Transport};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Patch {
    pub mem: MemoryMap,
}

/// Reads the amp's current patch, one RQ1 per block. `timeout` applies per block.
pub fn read_patch(t: &mut dyn Transport, timeout: Duration) -> Result<Patch, ProtoError> {
    read_patch_blocks(t, PATCH_BLOCKS, timeout)
}

/// Writes `p` to the amp's current patch.
pub fn write_patch(t: &mut dyn Transport, p: &Patch) -> Result<(), ProtoError> {
    send_paced(t, &patch_messages(PATCH_BLOCKS, p)?, SEND_INTERVAL)
}

impl Patch {
    /// The DT1 messages `write_patch` sends, as a `.syx` file.
    pub fn to_syx(&self) -> Vec<u8> {
        self.to_syx_blocks(PATCH_BLOCKS)
    }

    /// Rejects anything that is not a DT1 for this model or leaves a block uncovered.
    pub fn from_syx(bytes: &[u8]) -> Result<Patch, ProtoError> {
        Patch::from_syx_blocks(bytes, PATCH_BLOCKS)
    }

    pub(crate) fn to_syx_blocks(&self, blocks: &[(u32, u32)]) -> Vec<u8> {
        // A Patch only ever holds full blocks, so encoding cannot fail.
        write_syx(&patch_messages(blocks, self).expect("patch covers its blocks"))
    }

    pub(crate) fn from_syx_blocks(bytes: &[u8], blocks: &[(u32, u32)]) -> Result<Patch, ProtoError> {
        let mut mem = MemoryMap::default();
        for m in read_syx(bytes)? {
            match decode(&device::header(), &m)? {
                Command::Dt1 { addr, data } => mem.apply(addr, &data),
                Command::Rq1 { .. } => return Err(ProtoError::UnknownCommand(0x11)),
            }
        }
        restrict(&mem, blocks)
    }
}

/// Copies exactly the bytes of `blocks` out of `mem`.
fn restrict(mem: &MemoryMap, blocks: &[(u32, u32)]) -> Result<Patch, ProtoError> {
    let mut out = MemoryMap::default();
    for &(a, s) in blocks {
        let data = mem.read(Address(a), s).ok_or(ProtoError::IncompletePatch { addr: Address(a) })?;
        out.apply(Address(a), &data);
    }
    Ok(Patch { mem: out })
}

pub(crate) fn read_patch_blocks(
    t: &mut dyn Transport,
    blocks: &[(u32, u32)],
    timeout: Duration,
) -> Result<Patch, ProtoError> {
    let h = device::header();
    let mut mem = MemoryMap::default();
    for &(a, s) in blocks {
        let (start, end) = (a, a + s);
        t.send(&encode(&h, &Command::Rq1 { addr: Address(a), size: Address(s) })?)?;
        let deadline = Instant::now() + timeout;
        while mem.read(Address(a), s).is_none() {
            let left = deadline.checked_duration_since(Instant::now()).unwrap_or_default();
            let Some(msg) = t.recv(left)? else {
                return Err(ProtoError::Timeout { addr: Address(a) });
            };
            // Anything that is not one of our DT1s is someone else's traffic.
            let Ok(Command::Dt1 { addr, data }) = decode(&h, &msg) else { continue };
            for (i, &b) in data.iter().enumerate() {
                let k = addr.0 + i as u32;
                if (start..end).contains(&k) {
                    mem.0.insert(k, b);
                }
            }
        }
    }
    Ok(Patch { mem })
}

#[cfg(test)]
pub(crate) fn write_patch_blocks(t: &mut dyn Transport, blocks: &[(u32, u32)], p: &Patch) -> Result<(), ProtoError> {
    send_paced(t, &patch_messages(blocks, p)?, Duration::ZERO)
}

/// DT1 messages for every block, chunked at `MAX_DT1_DATA`.
fn patch_messages(blocks: &[(u32, u32)], p: &Patch) -> Result<Vec<Vec<u8>>, ProtoError> {
    let h = device::header();
    let mut msgs = Vec::new();
    for &(a, s) in blocks {
        let data = p.mem.read(Address(a), s).ok_or(ProtoError::IncompletePatch { addr: Address(a) })?;
        for (i, chunk) in data.chunks(MAX_DT1_DATA).enumerate() {
            let addr = Address(a) + (i * MAX_DT1_DATA) as u32;
            msgs.push(encode(&h, &Command::Dt1 { addr, data: chunk.to_vec() })?);
        }
    }
    Ok(msgs)
}

fn send_paced(t: &mut dyn Transport, msgs: &[Vec<u8>], interval: Duration) -> Result<(), ProtoError> {
    for m in msgs {
        t.send(m)?;
        std::thread::sleep(interval);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, device, encode, Address, Command, FakeAmp, Header, ProtoError, IDENTITY_REQUEST};

    const BLOCKS: &[(u32, u32)] = &[(0x100, 20), (0x200, 300), (0x400, 1)];

    fn filled_amp(max_dt1: usize) -> FakeAmp {
        let mut amp = FakeAmp::new(device::header(), max_dt1);
        let mut x: u32 = 12345;
        for &(a, s) in BLOCKS {
            let data: Vec<u8> = (0..s)
                .map(|_| {
                    x = x.wrapping_mul(1103515245).wrapping_add(12345);
                    ((x >> 16) & 0x7F) as u8
                })
                .collect();
            amp.mem.apply(Address(a), &data);
        }
        amp
    }

    #[test]
    fn read_then_write_is_byte_identical() {
        let mut amp1 = filled_amp(128);
        let p = read_patch_blocks(&mut amp1, BLOCKS, Duration::from_millis(50)).unwrap();
        let mut amp2 = FakeAmp::new(device::header(), 128);
        write_patch_blocks(&mut amp2, BLOCKS, &p).unwrap();
        assert_eq!(amp2.mem, amp1.mem);
    }

    #[test]
    fn read_patch_spans_7bit_boundary() {
        let start = Address::from_bytes(&[0x00, 0x00, 0x7F, 0x00]).unwrap();
        let blocks = [(start.0, 300)];
        let mut amp = FakeAmp::new(device::header(), 128);
        amp.mem.apply(start, &[0x11; 300]);
        let p = read_patch_blocks(&mut amp, &blocks, Duration::from_millis(50)).unwrap();
        assert_eq!(p.mem.read(start, 300), Some(vec![0x11; 300]));

        let mut amp2 = FakeAmp::new(device::header(), 128);
        write_patch_blocks(&mut amp2, &blocks, &p).unwrap();
        let addrs: Vec<Vec<u8>> = amp2
            .sent
            .iter()
            .map(|m| match decode(&device::header(), m).unwrap() {
                Command::Dt1 { addr, .. } => addr.to_bytes(4).unwrap(),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(addrs[0], vec![0x00, 0x00, 0x7F, 0x00]);
        assert_eq!(addrs[1], vec![0x00, 0x00, 0x7F, 0x73]); // + 115
        assert_eq!(addrs[2], vec![0x00, 0x01, 0x00, 0x66]); // + 230, carried
        assert_eq!(amp2.mem.read(start, 300), Some(vec![0x11; 300]));
    }

    #[test]
    fn read_patch_ignores_unrelated_messages() {
        let mut clean = filled_amp(128);
        let expected = read_patch_blocks(&mut clean, BLOCKS, Duration::from_millis(50)).unwrap();

        let mut noisy = filled_amp(128);
        let other_model = Header { model_id: vec![0x00, 0x00, 0x33], ..device::header() };
        noisy.noise = vec![
            IDENTITY_REQUEST.to_vec(),
            encode(&other_model, &Command::Dt1 { addr: Address(0x100), data: vec![0x7F; 4] }).unwrap(),
            encode(&device::header(), &Command::Dt1 { addr: Address(0x7000), data: vec![1] }).unwrap(),
        ];
        let got = read_patch_blocks(&mut noisy, BLOCKS, Duration::from_millis(50)).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn read_patch_times_out_when_amp_silent() {
        let mut amp = filled_amp(128);
        amp.silent = true;
        assert!(matches!(
            read_patch_blocks(&mut amp, BLOCKS, Duration::from_millis(50)),
            Err(ProtoError::Timeout { .. })
        ));
    }

    #[test]
    fn syx_round_trip_preserves_patch() {
        let mut amp = filled_amp(128);
        let p = read_patch_blocks(&mut amp, BLOCKS, Duration::from_millis(50)).unwrap();
        assert_eq!(Patch::from_syx_blocks(&p.to_syx_blocks(BLOCKS), BLOCKS).unwrap(), p);
    }

    #[test]
    fn load_rejects_foreign_model() {
        let other_model = Header { model_id: vec![0x00, 0x00, 0x33], ..device::header() };
        let syx = encode(&other_model, &Command::Dt1 { addr: Address(0x400), data: vec![1] }).unwrap();
        assert!(matches!(Patch::from_syx_blocks(&syx, &[(0x400, 1)]), Err(ProtoError::WrongModel { .. })));
    }

    #[test]
    fn from_syx_rejects_incomplete_patch() {
        let mut amp = filled_amp(128);
        let p = read_patch_blocks(&mut amp, BLOCKS, Duration::from_millis(50)).unwrap();
        let mut msgs = crate::read_syx(&p.to_syx_blocks(BLOCKS)).unwrap();
        msgs.remove(1);
        assert!(matches!(
            Patch::from_syx_blocks(&crate::write_syx(&msgs), BLOCKS),
            Err(ProtoError::IncompletePatch { .. })
        ));
    }
}
