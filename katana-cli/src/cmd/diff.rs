use std::path::Path;

use katana_proto::{decode, Header, MemoryMap};

use super::load_messages;

fn image(h: &Header, file: &Path) -> anyhow::Result<MemoryMap> {
    let cmds: Vec<_> = load_messages(file)?.iter().filter_map(|(_, m)| decode(h, m).ok()).collect();
    Ok(MemoryMap::from_commands(&cmds))
}

pub fn run(h: &Header, a: &Path, b: &Path) -> anyhow::Result<()> {
    let fmt = |v: Option<u8>| v.map_or("--".to_string(), |b| format!("{b:02X}"));
    for (addr, old, new) in image(h, a)?.diff(&image(h, b)?) {
        println!("{addr}: {} -> {}", fmt(old), fmt(new));
    }
    Ok(())
}
