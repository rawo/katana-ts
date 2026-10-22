use std::time::{Duration, Instant};

use anyhow::bail;
use katana_proto::{parse_identity_reply, MidirTransport, Transport, IDENTITY_REQUEST};

use super::hex;

pub fn run(port: &str) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    t.send(&IDENTITY_REQUEST)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Some(msg) = t.recv(left)? else { break };
        if let Ok(r) = parse_identity_reply(&msg) {
            println!("raw:          {}", hex(&msg));
            println!("device id:    {:02X}", r.device_id);
            println!("manufacturer: {:02X}", r.manufacturer);
            println!("family:       {}", hex(&r.family));
            println!("member:       {}", hex(&r.member));
            println!("version:      {}", hex(&r.version));
            return Ok(());
        }
    }
    bail!("no identity reply within 2s")
}
