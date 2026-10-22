//! Golden-byte tests: real traffic recorded from a KATANA Gen 3 (see captures/README.md).

use katana_proto::device::{self, PATCH_BLOCKS};
use katana_proto::{decode, encode, parse_identity_reply, read_syx, Address, Command, MemoryMap};

const RQ1: &[u8] = include_bytes!("../../captures/02-patch-read-rq1.syx");
const DT1: &[u8] = include_bytes!("../../captures/02-patch-read-dt1.syx");

#[test]
fn every_captured_message_decodes_and_reencodes_identically() {
    for m in read_syx(RQ1).unwrap().into_iter().chain(read_syx(DT1).unwrap()) {
        let cmd = decode(&device::header(), &m).unwrap();
        assert_eq!(encode(&device::header(), &cmd).unwrap(), m);
    }
}

#[test]
fn captured_patch_read_matches_patch_blocks() {
    let rq1: Vec<Command> = read_syx(RQ1).unwrap().iter().map(|m| decode(&device::header(), m).unwrap()).collect();
    let expected: Vec<Command> =
        PATCH_BLOCKS.iter().map(|&(a, s)| Command::Rq1 { addr: Address(a), size: Address(s) }).collect();
    assert_eq!(rq1, expected);
}

#[test]
fn captured_replies_cover_every_patch_block() {
    let cmds: Vec<Command> = read_syx(DT1).unwrap().iter().map(|m| decode(&device::header(), m).unwrap()).collect();
    let mem = MemoryMap::from_commands(&cmds);
    for &(a, s) in PATCH_BLOCKS {
        assert!(mem.read(Address(a), s).is_some(), "block {:?} not covered", Address(a));
    }
}

#[test]
fn captured_patch_name_reply_decodes() {
    // RQ1 20 00 00 00 size 16 sent with amidi; amp reply:
    let reply = [
        0xF0, 0x41, 0x10, 0x01, 0x05, 0x07, 0x12, 0x20, 0x00, 0x00, 0x00, 0x4B, 0x41, 0x54, 0x41, 0x4E, 0x41, 0x20,
        0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x70, 0xF7,
    ];
    let Command::Dt1 { addr, data } = decode(&device::header(), &reply).unwrap() else { panic!("not DT1") };
    assert_eq!(addr, Address::from_bytes(&[0x20, 0, 0, 0]).unwrap());
    assert_eq!(data, b"KATANA          ");
}

#[test]
fn captured_identity_reply_parses() {
    let r = parse_identity_reply(&[
        0xF0, 0x7E, 0x10, 0x06, 0x02, 0x41, 0x07, 0x05, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0xF7,
    ])
    .unwrap();
    assert_eq!((r.device_id, r.manufacturer, r.family), (device::DEVICE_ID, 0x41, [0x07, 0x05]));
}

#[test]
fn captured_patch_survives_syx_round_trip() {
    let p = katana_proto::Patch::from_syx(DT1).unwrap();
    assert_eq!(katana_proto::Patch::from_syx(&p.to_syx()).unwrap(), p);
    let total: u32 = PATCH_BLOCKS.iter().map(|&(_, s)| s).sum();
    assert_eq!(p.mem.0.len() as u32, total);
}

#[test]
fn every_param_lies_in_a_patch_block_with_an_in_range_value() {
    let p = katana_proto::Patch::from_syx(DT1).unwrap();
    assert!(!katana_proto::PARAMS.is_empty());
    for def in katana_proto::PARAMS {
        let v = p.get(def).unwrap_or_else(|| panic!("{} not inside the captured patch", def.name));
        assert!((def.min..=def.max).contains(&v), "{} = {v} outside {}..={}", def.name, def.min, def.max);
    }
}

#[test]
fn set_gain_100_bytes() {
    // Checksum by hand: 20+00+06+00+64 = 0x8A; 128 - (138 % 128) = 0x76
    let p = katana_proto::find_param("gain").unwrap();
    let data = katana_proto::encode_param(p, 100).unwrap();
    let msg = encode(&device::header(), &Command::Dt1 { addr: Address(p.addr), data }).unwrap();
    assert_eq!(msg, [0xF0, 0x41, 0x10, 0x01, 0x05, 0x07, 0x12, 0x20, 0x00, 0x06, 0x00, 0x64, 0x76, 0xF7]);
}

#[test]
fn set_delay_time_2000_bytes() {
    // 2000 = 0x7D0 -> nibbles 0 7 D 0; checksum: 20+2A+01+07+0D = 0x5F -> 0x21
    let p = katana_proto::find_param("delay2-time").unwrap();
    let data = katana_proto::encode_param(p, 2000).unwrap();
    let msg = encode(&device::header(), &Command::Dt1 { addr: Address(p.addr), data }).unwrap();
    assert_eq!(
        msg,
        [0xF0, 0x41, 0x10, 0x01, 0x05, 0x07, 0x12, 0x20, 0x00, 0x2A, 0x01, 0x00, 0x07, 0x0D, 0x00, 0x21, 0xF7]
    );
}

#[test]
fn contour_button_cycles_0_to_3() {
    // captures/101-watch-contour.log
    let p = katana_proto::find_param("contour").expect("contour param");
    assert_eq!((p.addr, p.min, p.max), (device::addr([0x20, 0x00, 0x40, 0x00]), 0, 3));
}

#[test]
fn enumerated_params_have_value_names() {
    let name = |p: &str, v: u32| {
        katana_proto::find_param(p)
            .and_then(|d| d.values)
            .and_then(|vs| vs.iter().find(|(x, _)| *x == v))
            .map(|(_, n)| *n)
    };
    assert_eq!(name("amp-type", 0), Some("ACOUSTIC"));
    assert_eq!(name("amp-type", 5), Some("BROWN"));
    assert_eq!(name("contour", 0), Some("OFF"));
    assert_eq!(name("cab-resonance", 2), Some("DEEP"));
    assert_eq!(name("booster-color", 0), Some("GREEN"));
    assert_eq!(name("reverb-sw", 1), Some("ON"));
}
