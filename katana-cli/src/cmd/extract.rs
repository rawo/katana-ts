use std::path::Path;

use katana_proto::Direction;

use super::{hex, load_messages};

pub fn run(file: &Path) -> anyhow::Result<()> {
    for m in load_messages(file)? {
        let arrow = match m.dir {
            Direction::HostToAmp => '>',
            Direction::AmpToHost => '<',
        };
        println!("{arrow} {}", hex(&m.bytes));
    }
    Ok(())
}
