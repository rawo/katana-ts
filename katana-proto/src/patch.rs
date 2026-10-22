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

/// Writes `p` to the amp's current patch, skipping `device::WRITE_PROTECTED`.
pub fn write_patch(t: &mut dyn Transport, p: &Patch) -> Result<(), ProtoError> {
    send_paced(t, &patch_messages(&writable(PATCH_BLOCKS), p)?, SEND_INTERVAL)
}

fn writable(blocks: &[(u32, u32)]) -> Vec<(u32, u32)> {
    blocks.iter().copied().filter(|&(a, _)| !device::is_write_protected(a)).collect()
}

/// Stored patch slots PATCH(1)..PATCH(10) sit 16 << 14 apart above the
/// current patch (`20 10 00 00`, … `20 70 00 00`, `21 00 00 00`, …).
pub const SLOTS: u8 = 10;

pub(crate) fn slot_offset(slot: u8) -> u32 {
    slot as u32 * (0x10 << 14)
}

/// Reads stored slot `slot` (1..=10; selector + 1, see docs/protocol.md),
/// relocated to current-patch addresses so it can be compared or written.
pub fn read_slot(t: &mut dyn Transport, slot: u8, timeout: Duration) -> Result<Patch, ProtoError> {
    if !(1..=SLOTS).contains(&slot) {
        return Err(ProtoError::OutOfRange { name: "slot", value: slot as u32, min: 1, max: SLOTS as u32 });
    }
    let off = slot_offset(slot);
    let blocks: Vec<(u32, u32)> = PATCH_BLOCKS.iter().map(|&(a, s)| (a + off, s)).collect();
    let stored = read_patch_blocks(t, &blocks, timeout)?;
    let mut mem = MemoryMap::default();
    for (&k, &b) in &stored.mem.0 {
        mem.0.insert(k - off, b);
    }
    Ok(Patch { mem })
}

/// Proves that writes land: writes `scratch` (which must differ from the
/// current patch) and reads it back, then restores the original and reads
/// that back. The original is restored even when the first check fails.
pub fn verify_write_back(t: &mut dyn Transport, scratch: &Patch, timeout: Duration) -> Result<(), ProtoError> {
    let original = read_patch(t, timeout)?;
    if mismatch(scratch, &original).is_none() {
        return Err(ProtoError::ScratchEqualsCurrent);
    }
    write_patch(t, scratch)?;
    let first = read_patch(t, timeout).map(|got| mismatch(scratch, &got));
    write_patch(t, &original)?;
    if let Some(e) = first? {
        return Err(e);
    }
    match mismatch(&original, &read_patch(t, timeout)?) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn mismatch(expected: &Patch, got: &Patch) -> Option<ProtoError> {
    expected.mem.0.iter().find(|(k, b)| !device::is_write_protected(**k) && got.mem.0.get(k) != Some(b)).map(
        |(&k, &b)| ProtoError::WriteBackMismatch { addr: Address(k), expected: b, got: got.mem.0.get(&k).copied() },
    )
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
    send_paced(t, &patch_messages(&writable(blocks), p)?, Duration::ZERO)
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

    fn slot_amp(ignore_dt1: bool) -> FakeAmp {
        let mut amp = FakeAmp::new(device::header(), 128);
        amp.ignore_dt1 = ignore_dt1;
        for &(a, s) in PATCH_BLOCKS {
            amp.mem.apply(Address(a), &vec![0x01; s as usize]);
            amp.mem.apply(Address(a + slot_offset(3)), &vec![0x02; s as usize]);
        }
        amp
    }

    #[test]
    fn read_slot_relocates_to_current_patch_addresses() {
        let mut amp = slot_amp(false);
        let s = read_slot(&mut amp, 3, Duration::from_millis(50)).unwrap();
        let (a, n) = PATCH_BLOCKS[0];
        assert_eq!(s.mem.read(Address(a), n), Some(vec![0x02; n as usize]));
    }

    #[test]
    fn slot_offsets_follow_channel_layout() {
        let current = device::addr([0x20, 0, 0, 0]);
        assert_eq!(Address(current + slot_offset(1)).to_bytes(4).unwrap(), vec![0x20, 0x10, 0x00, 0x00]);
        assert_eq!(Address(current + slot_offset(8)).to_bytes(4).unwrap(), vec![0x21, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn verify_write_back_restores_original() {
        let mut amp = slot_amp(false);
        let before = amp.mem.clone();
        let other = read_slot(&mut amp, 3, Duration::from_millis(50)).unwrap();
        verify_write_back(&mut amp, &other, Duration::from_millis(50)).unwrap();
        assert_eq!(amp.mem, before);
    }

    #[test]
    fn verify_write_back_fails_when_amp_ignores_writes() {
        let mut amp = slot_amp(true);
        let other = read_slot(&mut amp, 3, Duration::from_millis(50)).unwrap();
        assert!(matches!(
            verify_write_back(&mut amp, &other, Duration::from_millis(50)),
            Err(ProtoError::WriteBackMismatch { .. })
        ));
    }

    #[test]
    fn verify_write_back_passes_when_only_write_protected_blocks_are_ignored() {
        let mut amp = slot_amp(false);
        amp.read_only = device::WRITE_PROTECTED.to_vec();
        let other = read_slot(&mut amp, 3, Duration::from_millis(50)).unwrap();
        verify_write_back(&mut amp, &other, Duration::from_millis(50)).unwrap();
    }

    #[test]
    fn verify_write_back_still_fails_when_another_block_is_ignored() {
        let mut amp = slot_amp(false);
        amp.read_only = vec![PATCH_BLOCKS[3]];
        let other = read_slot(&mut amp, 3, Duration::from_millis(50)).unwrap();
        assert!(matches!(
            verify_write_back(&mut amp, &other, Duration::from_millis(50)),
            Err(ProtoError::WriteBackMismatch { .. })
        ));
    }

    #[test]
    fn write_patch_skips_write_protected_blocks() {
        let mut src = slot_amp(false);
        let p = read_patch(&mut src, Duration::from_millis(50)).unwrap();
        let mut amp = FakeAmp::new(device::header(), 128);
        write_patch(&mut amp, &p).unwrap();
        for m in &amp.sent {
            let Command::Dt1 { addr, data } = decode(&device::header(), m).unwrap() else { panic!() };
            for &(a, n) in device::WRITE_PROTECTED {
                assert!(
                    addr.0 + data.len() as u32 <= a || addr.0 >= a + n,
                    "DT1 at {addr:?} overlaps a protected block"
                );
            }
        }
    }

    #[test]
    fn verify_write_back_rejects_identical_scratch() {
        let mut amp = slot_amp(false);
        let same = read_patch(&mut amp, Duration::from_millis(50)).unwrap();
        assert!(matches!(
            verify_write_back(&mut amp, &same, Duration::from_millis(50)),
            Err(ProtoError::ScratchEqualsCurrent)
        ));
    }
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
