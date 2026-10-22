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
    use katana_proto::{read_patch, write_patch};
    let mut t = MidirTransport::open(DEFAULT_PORT).unwrap();
    let a = read_patch(&mut t, Duration::from_secs(2)).unwrap();
    write_patch(&mut t, &a).unwrap();
    let b = read_patch(&mut t, Duration::from_secs(2)).unwrap();
    assert_eq!(a, b);
}
