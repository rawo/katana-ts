//! Tests against a real amp. Run manually: `cargo test -p katana-proto --features hardware-tests`
//! with the amp connected and the official app closed.
#![cfg(feature = "hardware-tests")]

use std::time::Duration;

use katana_proto::{parse_identity_reply, MidirTransport, Transport, DEFAULT_PORT, IDENTITY_REQUEST};

#[test]
fn amp_answers_identity_request() {
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    t.send(&IDENTITY_REQUEST).unwrap();
    let reply = t.recv(Duration::from_secs(2)).unwrap().expect("no identity reply");
    assert_eq!(parse_identity_reply(&reply).unwrap().manufacturer, 0x41);
}

#[test]
fn hardware_patch_round_trip_is_byte_identical() {
    use katana_proto::{read_patch, read_slot, verify_write_back, SLOTS};
    let timeout = Duration::from_secs(2);
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let current = read_patch(&mut t, timeout).unwrap();
    let scratch = (1..=SLOTS)
        .map(|n| read_slot(&mut t, n, timeout).unwrap())
        .find(|p| *p != current)
        .expect("a stored slot that differs from the current patch");
    verify_write_back(&mut t, &scratch, timeout).unwrap();
}

const T: Duration = Duration::from_secs(2);

fn today() -> String {
    let out = std::process::Command::new("date").arg("+%F").output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Backs up the scratch channels A3, A4, B3, B4 (slots 4, 5, 8, 9) to
/// `backups/<date>-slot<n>.syx` in the repository (untracked).
#[test]
fn backup_scratch_channels() {
    use katana_proto::{read_slot, Patch};
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../backups");
    std::fs::create_dir_all(&dir).unwrap();
    for n in [4u8, 5, 8, 9] {
        let p = read_slot(&mut t, n, T).unwrap();
        let path = dir.join(format!("{}-slot{n}.syx", today()));
        std::fs::write(&path, p.to_syx()).unwrap();
        assert_eq!(Patch::from_syx(&std::fs::read(&path).unwrap()).unwrap(), p);
        println!("slot {n} '{}': {}", p.name(), path.display());
    }
}

/// Whether SOLO_DELAY can be written straight into a stored channel (B4).
#[test]
fn slot_solo_delay_direct_write() {
    use katana_proto::amp::write_slot_block;
    use katana_proto::{device, read_slot, Address, Channel};
    let (a, s) = device::WRITE_PROTECTED[0];
    let level = Address(a + 7);
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let orig = read_slot(&mut t, Channel::B4.slot(), T).unwrap();
    let old = orig.mem.read(level, 1).unwrap()[0];
    let mut changed = orig.clone();
    let new = if old == 30 { 31 } else { 30 };
    changed.mem.apply(level, &[new]);
    write_slot_block(&mut t, Channel::B4, (a, s), &changed).unwrap();
    let after = read_slot(&mut t, Channel::B4.slot(), T).unwrap();
    println!("SOLO_DELAY level in B4: was {old}, wrote {new}, read back {}", after.mem.read(level, 1).unwrap()[0]);
    write_slot_block(&mut t, Channel::B4, (a, s), &orig).unwrap();
    let restored = read_slot(&mut t, Channel::B4.slot(), T).unwrap();
    println!("after restore: {}", restored.mem.read(level, 1).unwrap()[0]);
    assert_eq!(restored, orig);
}

/// Prints every message the amp sends within `d`.
fn drain(t: &mut dyn Transport, what: &str, d: Duration) {
    use katana_proto::amp::notification;
    let h = katana_proto::device::header();
    let end = std::time::Instant::now() + d;
    while let Some(left) = end.checked_duration_since(std::time::Instant::now()) {
        match t.recv(left).unwrap() {
            Some(m) => println!("{what}: {:?}", notification(&h, &m)),
            None => break,
        }
    }
}

/// Stores a changed B4 and the original back; also prints what the amp sends
/// by itself around a select and a store.
#[test]
fn store_patch_round_trip_on_b4() {
    use katana_proto::amp::{store_current, store_patch, EditorSession};
    use katana_proto::{device, encode, find_param, read_slot, Address, Channel, Command};
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let orig = read_slot(&mut t, Channel::B4.slot(), T).unwrap();

    // Raw look at notifications: select B4, then store it unchanged into B4.
    {
        let mut s = EditorSession::enter(&mut t).unwrap();
        drain(s.transport(), "editor on", Duration::from_secs(1));
        let sel = Command::Dt1 { addr: Address(device::PATCH_SELECT), data: vec![0, Channel::B4.selector()] };
        s.transport().send(&encode(&device::header(), &sel).unwrap()).unwrap();
        drain(s.transport(), "select B4", Duration::from_secs(2));
        let echo = store_current(&mut s, Channel::B4, T).unwrap();
        println!("store B4 unchanged: echo {echo}");
        drain(s.transport(), "after store", Duration::from_secs(1));
        s.close().unwrap();
    }
    assert_eq!(read_slot(&mut t, Channel::B4.slot(), T).unwrap(), orig, "unchanged store changed B4");

    let mut p = orig.clone();
    p.set_name("KTS TEST");
    let gain = find_param("gain").unwrap();
    let g = p.get(gain).unwrap();
    p.set(gain, if g == 70 { 71 } else { 70 }).unwrap();
    let r1 = store_patch(&mut t, Channel::B4, &p, T);
    println!("store test patch: {r1:?}");
    let r2 = store_patch(&mut t, Channel::B4, &orig, T);
    println!("store original: {r2:?}");
    r1.unwrap();
    r2.unwrap();
    assert_eq!(read_slot(&mut t, Channel::B4.slot(), T).unwrap(), orig);
}

/// Whether `PATCH_SELECT` can switch the amp to PANEL (selector 0); goes back after.
#[test]
fn select_panel_and_back() {
    use katana_proto::amp::{read_selector, select, set_editor_mode};
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let s = read_selector(&mut t, T).unwrap();
    set_editor_mode(&mut t, true).unwrap();
    let to_panel = select(&mut t, 0, T);
    println!("select PANEL from selector {s}: {to_panel:?}");
    let back = select(&mut t, s, T);
    println!("select back to {s}: {back:?}");
    set_editor_mode(&mut t, false).unwrap();
    to_panel.unwrap();
    back.unwrap();
}
