use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use katana_proto::{device::addr, read_patch, Address, MidirTransport, Patch};

use crate::addrmap::{AddrMap, DEFAULT_PATH};

/// The official app's address table in the default Wine prefix, if present.
pub fn default_map_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let prefix = std::env::var_os("WINEPREFIX").map_or_else(|| Path::new(&home).join(".wine"), PathBuf::from);
    let user = std::env::var("USER").ok()?;
    Some(prefix.join(DEFAULT_PATH.replace("{user}", &user))).filter(|p| p.exists())
}

fn load_map(path: Option<&Path>) -> AddrMap {
    let base = addr([0x20, 0, 0, 0]);
    let path = path.map(Path::to_path_buf).or_else(default_map_path);
    match path.as_deref().map(|p| (p, std::fs::read_to_string(p))) {
        Some((p, Ok(js))) => {
            let m = AddrMap::parse(&js, base);
            eprintln!("labels: {} patch bytes named from {}", m.len(), p.display());
            m
        }
        Some((p, Err(e))) => {
            eprintln!("labels: cannot read {}: {e}; printing addresses only", p.display());
            AddrMap::parse("", base)
        }
        None => {
            eprintln!("labels: official app's address_map.js not found (use --map); printing addresses only");
            AddrMap::parse("", base)
        }
    }
}

/// Polls the current patch and prints every parameter that changes.
pub fn run(port: &str, map: Option<&Path>, interval: Duration) -> anyhow::Result<()> {
    let map = load_map(map);
    let mut t = MidirTransport::open(port)?;
    let mut prev = read_patch(&mut t, Duration::from_secs(2))?;
    eprintln!("watching the current patch — turn a knob (Ctrl-C to stop)");
    loop {
        std::thread::sleep(interval);
        let cur = read_patch(&mut t, Duration::from_secs(2))?;
        report(&map, &prev, &cur);
        prev = cur;
    }
}

fn report(map: &AddrMap, prev: &Patch, cur: &Patch) {
    let mut seen = BTreeSet::new();
    for (a, old, new) in prev.mem.diff(&cur.mem) {
        let Some(l) = map.label(a.0) else {
            println!("{a}  ?  {old:02X?} -> {new:02X?}");
            continue;
        };
        let start = Address(a.0 - l.byte);
        if !seen.insert(start) {
            continue;
        }
        let (Some(ob), Some(nb)) = (prev.mem.read(start, l.len), cur.mem.read(start, l.len)) else { continue };
        let (ov, nv) = (l.decode(&ob), l.decode(&nb));
        let shown = |v: u32| v as i64 - l.ofs as i64;
        println!("{start}  {:<12} {:<44} {} -> {}   (raw {ov} -> {nv})", l.block, l.param, shown(ov), shown(nv));
    }
}
