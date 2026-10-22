use std::time::Duration;

use katana_proto::{read_patch, MidirTransport, PARAMS};

pub fn run(port: &str) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let patch = read_patch(&mut t, Duration::from_secs(2))?;
    for p in PARAMS {
        let current = patch.get(p).map_or("?".to_string(), |v| {
            let name = p.values.and_then(|vs| vs.iter().find(|(x, _)| *x == v)).map(|(_, n)| *n);
            name.map_or(v.to_string(), |n| format!("{v} ({n})"))
        });
        println!("{:<32} {:>3}..={:<3} {current}", p.name, p.min, p.max);
    }
    Ok(())
}
