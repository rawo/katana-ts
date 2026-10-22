use std::time::Duration;

use katana_proto::{Header, MidirTransport, Transport};

use super::{describe, hex};

pub fn run(port: &str, h: &Header) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    eprintln!("listening on {port:?} (Ctrl-C to stop)");
    loop {
        if let Some(msg) = t.recv(Duration::from_secs(1))? {
            match describe(h, &msg) {
                Some(d) => println!("{}  ; {d}", hex(&msg)),
                None => println!("{}", hex(&msg)),
            }
        }
    }
}
