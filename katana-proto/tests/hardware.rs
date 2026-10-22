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
