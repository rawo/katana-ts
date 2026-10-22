use std::path::Path;

use katana_proto::Direction;

use super::{hex, load_messages};

pub fn run(file: &Path) -> anyhow::Result<()> {
    for (dir, bytes) in load_messages(file)? {
        let arrow = match dir {
            Some(Direction::HostToAmp) => '>',
            Some(Direction::AmpToHost) => '<',
            None => ' ',
        };
        println!("{arrow} {}", hex(&bytes));
    }
    Ok(())
}
