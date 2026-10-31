//! Controlling the amp beyond its current patch: editor mode, channel select,
//! storing into a channel, and the messages the amp sends on its own.
//! Evidence: docs/protocol.md, "Amp control".

use std::time::{Duration, Instant};

use crate::device::{self, MAX_DT1_DATA, SEND_INTERVAL};
use crate::patch::{read_blocks, slot_offset};
use crate::{decode, encode, Address, Channel, Command, Header, Patch, ProtoError, Transport};

/// Pause between selector reads while waiting for a channel change.
const POLL: Duration = Duration::from_millis(20);

/// Editor mode for as long as the session lives. Dropping it (also on an
/// error) turns editor mode off again, best effort.
pub struct EditorSession<'t> {
    t: &'t mut dyn Transport,
    open: bool,
}

/// Turns editor mode on or off (`device::EDITOR_MODE`).
pub fn set_editor_mode(t: &mut dyn Transport, on: bool) -> Result<(), ProtoError> {
    send_dt1(t, device::EDITOR_MODE, &[u8::from(on)])
}

impl<'t> EditorSession<'t> {
    pub fn enter(t: &'t mut dyn Transport) -> Result<Self, ProtoError> {
        set_editor_mode(t, true)?;
        Ok(EditorSession { t, open: true })
    }

    pub fn transport(&mut self) -> &mut dyn Transport {
        self.t
    }

    pub fn close(mut self) -> Result<(), ProtoError> {
        self.open = false;
        set_editor_mode(self.t, false)
    }
}

impl Drop for EditorSession<'_> {
    fn drop(&mut self) {
        if self.open {
            let _ = set_editor_mode(self.t, false);
        }
    }
}

fn send_dt1(t: &mut dyn Transport, a: u32, data: &[u8]) -> Result<(), ProtoError> {
    t.send(&encode(&device::header(), &Command::Dt1 { addr: Address(a), data: data.to_vec() })?)
}

/// The selected channel: 0 = PANEL, 1–8 = A1–B4 (`Channel::selector`).
pub fn read_selector(t: &mut dyn Transport, timeout: Duration) -> Result<u8, ProtoError> {
    let p = read_blocks(t, &[(device::SELECTOR, 2)], timeout)?;
    let b = p.mem.read(Address(device::SELECTOR), 2).expect("read_blocks covers the block");
    Ok(b[0] << 4 | b[1])
}

/// Switches the amp to `ch` (it loads the channel into the current patch) and
/// waits until the selector shows it, then `device::SELECT_SETTLE`.
pub fn select_channel(s: &mut EditorSession, ch: Channel, timeout: Duration) -> Result<(), ProtoError> {
    select(s.t, ch.selector(), timeout)
}

/// `select_channel` for any selector value, 0 (PANEL) to 8; editor mode must be on.
pub fn select(t: &mut dyn Transport, selector: u8, timeout: Duration) -> Result<(), ProtoError> {
    send_dt1(t, device::PATCH_SELECT, &[0, selector])?;
    let deadline = Instant::now() + timeout;
    while read_selector(t, timeout)? != selector {
        if Instant::now() >= deadline {
            return Err(ProtoError::Timeout { addr: Address(device::SELECTOR) });
        }
        std::thread::sleep(POLL);
    }
    std::thread::sleep(device::SELECT_SETTLE);
    Ok(())
}

/// Stores the current patch into `ch`. Returns whether the amp's echo came
/// within `timeout`; its absence is no error — reading the channel back decides.
pub fn store_current(s: &mut EditorSession, ch: Channel, timeout: Duration) -> Result<bool, ProtoError> {
    send_dt1(s.t, device::PATCH_WRITE, &[0, ch.selector()])?;
    let h = device::header();
    let deadline = Instant::now() + timeout;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Some(msg) = s.t.recv(left)? else { break };
        if notification(&h, &msg) == Some(Notification::PatchWritten(ch.selector())) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Writes `p`'s bytes of the current-patch `block` straight into `ch`'s stored
/// slot (not through the current patch).
pub fn write_slot_block(t: &mut dyn Transport, ch: Channel, block: (u32, u32), p: &Patch) -> Result<(), ProtoError> {
    let (a, s) = block;
    let data = p.mem.read(Address(a), s).ok_or(ProtoError::IncompletePatch { addr: Address(a) })?;
    let base = a + slot_offset(ch.slot());
    for (i, chunk) in data.chunks(MAX_DT1_DATA).enumerate() {
        send_dt1(t, base + (i * MAX_DT1_DATA) as u32, chunk)?;
        std::thread::sleep(SEND_INTERVAL);
    }
    Ok(())
}

/// What `store_patch` could do beyond the checked bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreReport {
    /// False when the amp can't take SOLO_DELAY: the channel keeps its own.
    pub solo_delay_stored: bool,
    /// Whether the amp confirmed the store by itself.
    pub echo_seen: bool,
}

/// Stores `p` into channel `ch` and reads the channel back. The amp ends on
/// `ch`. It is switched there first, so a channel keeps its own solo delay
/// when the amp can't take `p`'s (`device::SLOT_SOLO_DELAY_WRITABLE`).
pub fn store_patch(
    t: &mut dyn Transport,
    ch: Channel,
    p: &Patch,
    timeout: Duration,
) -> Result<StoreReport, ProtoError> {
    let solo_delay = device::WRITE_PROTECTED[0];
    let mut s = EditorSession::enter(t)?;
    select_channel(&mut s, ch, timeout)?;
    crate::write_patch(s.t, p)?;
    let echo_seen = store_current(&mut s, ch, timeout)?;
    if device::SLOT_SOLO_DELAY_WRITABLE {
        write_slot_block(s.t, ch, solo_delay, p)?;
    }
    let got = crate::read_slot(s.t, ch.slot(), timeout)?;
    let skip: Vec<(u32, u32)> = device::NOT_STORED
        .iter()
        .copied()
        .filter(|&r| !(device::SLOT_SOLO_DELAY_WRITABLE && r == solo_delay))
        .collect();
    if let Some((addr, expected, got)) = crate::patch::mismatch_skipping(p, &got, &skip) {
        return Err(ProtoError::StoreMismatch { channel: ch.to_string(), addr, expected, got });
    }
    s.close()?;
    Ok(StoreReport { solo_delay_stored: device::SLOT_SOLO_DELAY_WRITABLE, echo_seen })
}

/// A message the amp sent by itself (while editor mode is on).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notification {
    /// The selected channel changed (`Channel::selector` value).
    Selector(u8),
    /// The current patch was stored into this selector's channel.
    PatchWritten(u8),
    Other {
        addr: Address,
        data: Vec<u8>,
    },
}

pub fn notification(h: &Header, msg: &[u8]) -> Option<Notification> {
    let Ok(Command::Dt1 { addr, data }) = decode(h, msg) else { return None };
    Some(match (addr.0, data.as_slice()) {
        (device::SELECTOR, &[b0, b1]) => Notification::Selector(b0 << 4 | b1),
        (device::PATCH_WRITE, &[0, n]) => Notification::PatchWritten(n),
        _ => Notification::Other { addr, data },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{self, PATCH_BLOCKS, WRITE_PROTECTED};
    use crate::patch::slot_offset;
    use crate::{encode, read_slot, write_patch, Address, Command, FakeAmp, Patch};

    const T: Duration = Duration::from_millis(50);

    /// Current patch filled with 0x40, stored slot n with n, selector at A4.
    pub(crate) fn amp() -> FakeAmp {
        let mut amp = FakeAmp::new(device::header(), 128);
        for &(a, s) in PATCH_BLOCKS {
            amp.mem.apply(Address(a), &vec![0x40; s as usize]);
            for n in 1..=9u8 {
                amp.mem.apply(Address(a + slot_offset(n)), &vec![n; s as usize]);
            }
        }
        amp.mem.apply(Address(device::SELECTOR), &[0, 4]);
        amp
    }

    fn dt1(addr: u32, data: &[u8]) -> Vec<u8> {
        encode(&device::header(), &Command::Dt1 { addr: Address(addr), data: data.to_vec() }).unwrap()
    }

    fn current(amp: &mut FakeAmp) -> Patch {
        crate::read_patch(amp, T).unwrap()
    }

    #[test]
    fn editor_session_turns_mode_off_on_drop() {
        let mut amp = amp();
        {
            let _s = EditorSession::enter(&mut amp).unwrap();
        }
        assert!(!amp.editor);
        assert_eq!(amp.sent.last().unwrap(), &dt1(device::EDITOR_MODE, &[0]));
    }

    #[test]
    fn set_editor_mode_toggles() {
        let mut amp = amp();
        set_editor_mode(&mut amp, true).unwrap();
        assert!(amp.editor);
        set_editor_mode(&mut amp, false).unwrap();
        assert!(!amp.editor);
    }

    #[test]
    fn select_reaches_panel() {
        let mut amp = amp();
        set_editor_mode(&mut amp, true).unwrap();
        select(&mut amp, 0, T).unwrap();
        assert_eq!(read_selector(&mut amp, T).unwrap(), 0);
        assert_eq!(current(&mut amp), read_slot(&mut amp, 1, T).unwrap());
    }

    #[test]
    fn select_needs_editor_mode() {
        let mut amp = amp();
        let before = current(&mut amp);
        amp.send(&dt1(device::PATCH_SELECT, &[0, 3])).unwrap();
        assert_eq!(read_selector(&mut amp, T).unwrap(), 4);
        assert_eq!(current(&mut amp), before);
    }

    #[test]
    fn select_channel_switches_and_loads_slot() {
        let mut amp = amp();
        {
            let mut s = EditorSession::enter(&mut amp).unwrap();
            select_channel(&mut s, Channel::A2, T).unwrap();
        }
        assert_eq!(read_selector(&mut amp, T).unwrap(), 2);
        assert_eq!(current(&mut amp), read_slot(&mut amp, 3, T).unwrap());
    }

    #[test]
    fn select_channel_times_out_when_amp_ignores_it() {
        let mut amp = amp();
        amp.ignore_dt1 = true;
        let mut s = EditorSession::enter(&mut amp).unwrap();
        assert!(matches!(select_channel(&mut s, Channel::A2, T), Err(ProtoError::Timeout { .. })));
    }

    #[test]
    fn store_current_copies_into_slot_and_sees_echo() {
        for echo in [true, false] {
            let mut amp = amp();
            amp.echo = echo;
            let mut p = current(&mut amp);
            p.set_name("Stored");
            {
                let mut s = EditorSession::enter(&mut amp).unwrap();
                select_channel(&mut s, Channel::B3, T).unwrap();
                write_patch(s.transport(), &p).unwrap();
                assert_eq!(store_current(&mut s, Channel::B3, T).unwrap(), echo);
            }
            assert_eq!(read_slot(&mut amp, Channel::B3.slot(), T).unwrap().name(), "Stored");
        }
    }

    #[test]
    fn write_slot_block_writes_relocated_bytes() {
        let mut amp = amp();
        let before = amp.mem.clone();
        let (a, s) = WRITE_PROTECTED[0];
        let mut p = Patch::default();
        p.mem.apply(Address(a), &vec![0x77; s as usize]);
        write_slot_block(&mut amp, Channel::B4, (a, s), &p).unwrap();
        let changed: Vec<u32> = amp.mem.diff(&before).into_iter().map(|(k, _, _)| k.0).collect();
        let off = slot_offset(9);
        assert_eq!(changed, (a + off..a + off + s).collect::<Vec<_>>());
        assert_eq!(amp.mem.read(Address(a + off), s).unwrap(), vec![0x77; s as usize]);
    }

    fn named(name: &str) -> Patch {
        let mut p = Patch::default_preset();
        p.set_name(name);
        p
    }

    fn skipping_solo_delay(p: &Patch) -> Patch {
        let mut p = p.clone();
        let (a, s) = WRITE_PROTECTED[0];
        p.mem.0.retain(|&k, _| !(a..a + s).contains(&k));
        p
    }

    #[test]
    fn store_patch_stores_and_ends_on_channel() {
        let mut amp = amp();
        let p = named("Iron Man");
        store_patch(&mut amp, Channel::B3, &p, T).unwrap();
        assert_eq!(read_selector(&mut amp, T).unwrap(), 7);
        let got = read_slot(&mut amp, Channel::B3.slot(), T).unwrap();
        assert_eq!(skipping_solo_delay(&got), skipping_solo_delay(&p));
        assert!(!amp.editor);
    }

    #[test]
    fn store_patch_keeps_channel_solo_delay() {
        let mut amp = amp();
        let (a, s) = WRITE_PROTECTED[0];
        amp.read_only.push((a, s));
        amp.mem.apply(Address(a), &vec![0x66; s as usize]);
        amp.mem.apply(Address(a + slot_offset(8)), &vec![0x55; s as usize]);
        let mut p = named("Iron Man");
        p.mem.apply(Address(a), &vec![0x77; s as usize]);
        let report = store_patch(&mut amp, Channel::B3, &p, T).unwrap();
        assert!(!report.solo_delay_stored);
        assert_eq!(amp.mem.read(Address(a + slot_offset(8)), s).unwrap(), vec![0x55; s as usize]);
    }

    #[test]
    fn store_patch_detects_write_that_did_not_land() {
        let mut amp = amp();
        // The AMP block of the current patch ignores writes, so the store copies the old one.
        amp.read_only.push(PATCH_BLOCKS[3]);
        let r = store_patch(&mut amp, Channel::B3, &named("Iron Man"), T);
        assert!(matches!(r, Err(ProtoError::StoreMismatch { .. })), "{r:?}");
        assert!(!amp.editor);
    }

    #[test]
    fn store_patch_leaves_editor_mode_on_error() {
        let mut amp = amp();
        amp.ignore_dt1 = true;
        assert!(store_patch(&mut amp, Channel::B3, &named("x"), T).is_err());
        assert_eq!(amp.sent.last().unwrap(), &dt1(device::EDITOR_MODE, &[0]));
    }

    #[test]
    fn store_patch_without_echo_still_verifies() {
        let mut amp = amp();
        amp.echo = false;
        let report = store_patch(&mut amp, Channel::A4, &named("Quiet"), T).unwrap();
        assert!(!report.echo_seen);
        assert_eq!(read_slot(&mut amp, Channel::A4.slot(), T).unwrap().name(), "Quiet");
    }

    #[test]
    fn notifications_decode() {
        let h = device::header();
        assert_eq!(notification(&h, &dt1(device::SELECTOR, &[0, 5])), Some(Notification::Selector(5)));
        assert_eq!(notification(&h, &dt1(device::PATCH_WRITE, &[0, 8])), Some(Notification::PatchWritten(8)));
        let status = device::addr([0x7F, 0x01, 0x01, 0x05]);
        assert_eq!(
            notification(&h, &dt1(status, &[0x12])),
            Some(Notification::Other { addr: Address(status), data: vec![0x12] })
        );
        let rq1 = encode(&h, &Command::Rq1 { addr: Address(0), size: Address(2) }).unwrap();
        assert_eq!(notification(&h, &rq1), None);
    }
}
