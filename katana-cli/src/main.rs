mod addrmap;
mod ask;
mod backup;
mod cmd;
mod files;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use katana_proto::DEFAULT_PORT;

/// Command-line tool for the BOSS KATANA Gen 3 SysEx protocol.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Substring of the MIDI port name to use.
    #[arg(long, global = true, default_value = DEFAULT_PORT)]
    port: String,
    /// Override the model ID (hex) used to decode messages.
    #[arg(long, global = true)]
    model_id: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List MIDI ports.
    Ports,
    /// Send a MIDI Identity Request and print the reply.
    Identity,
    /// Print SysEx received from the amp. Only sees amp->host traffic;
    /// host->amp traffic needs a usbmon capture (captures/tools/capture.sh).
    Monitor,
    /// Print the SysEx messages in a capture export (.tsv) or .syx file.
    Extract { file: PathBuf },
    /// Show bytes that differ between the DT1 data in two files.
    Diff { a: PathBuf, b: PathBuf },
    /// Read the amp's current patch into a .syx file (`dump <file>`), or a
    /// stored channel into a .syx or .tsl file (`dump <channel> <file>`).
    Dump {
        /// A file for the current patch, or a channel A1–A4, B1–B4.
        #[arg(value_name = "CHANNEL|FILE")]
        first: String,
        /// The file for a channel; .syx or .tsl picks the format.
        #[arg(value_name = "FILE")]
        second: Option<PathBuf>,
        /// Overwrite an existing file without asking.
        #[arg(long)]
        yes: bool,
    },
    /// Store a .syx patch, or patch N of a .tsl set, into a channel A1–A4,
    /// B1–B4. Backs up the channel and the current sound first, asks, reads
    /// the channel back, and leaves the amp on that channel.
    Store {
        /// A .syx patch or a .tsl set.
        file: PathBuf,
        /// A1–A4 or B1–B4.
        channel: String,
        /// Which patch of a .tsl set, from 1 (see tsl-list).
        #[arg(long)]
        patch: Option<usize>,
        /// Replace the channel without asking.
        #[arg(long)]
        yes: bool,
    },
    /// List the patches of a .tsl set, numbered for store --patch.
    TslList { file: PathBuf },
    /// Write a .syx patch (from `dump`) into the amp's current patch.
    Load { file: PathBuf },
    /// Prove writes land: write a different stored patch into the current
    /// patch, read it back, restore the original, read that back.
    Verify {
        /// Stored slot PATCH(n), 1..=10, to use as the different patch.
        #[arg(long)]
        slot: Option<u8>,
    },
    /// Set one parameter of the current patch.
    Set { name: String, value: String },
    /// List known parameters with their current values.
    Params,
    /// Poll the current patch and print each parameter that changes, named
    /// from the official app's address table when it is installed.
    Watch {
        /// Path to the official app's address_map.js (default: Wine prefix).
        #[arg(long)]
        map: Option<PathBuf>,
        /// Milliseconds between reads.
        #[arg(long, default_value_t = 300)]
        interval_ms: u64,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let header = cmd::header(cli.model_id.as_deref())?;
    match cli.command {
        Command::Ports => cmd::ports::run(),
        Command::Identity => cmd::identity::run(&cli.port),
        Command::Monitor => cmd::monitor::run(&cli.port, &header),
        Command::Extract { file } => cmd::extract::run(&file),
        Command::Diff { a, b } => cmd::diff::run(&header, &a, &b),
        Command::Dump { first, second, yes } => cmd::dump::run(&cli.port, &first, second.as_deref(), yes),
        Command::Store { file, channel, patch, yes } => cmd::store::run(&cli.port, &file, &channel, patch, yes),
        Command::TslList { file } => cmd::tsl_list::run(&file),
        Command::Load { file } => cmd::load::run(&cli.port, &file),
        Command::Verify { slot } => cmd::verify::run(&cli.port, slot),
        Command::Set { name, value } => cmd::set::run(&cli.port, &name, &value),
        Command::Params => cmd::params::run(&cli.port),
        Command::Watch { map, interval_ms } => {
            cmd::watch::run(&cli.port, map.as_deref(), std::time::Duration::from_millis(interval_ms))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses() {
        for args in [
            &["katana-cli", "store", "x.tsl", "A3", "--patch", "4", "--yes"][..],
            &["katana-cli", "dump", "now.syx"],
            &["katana-cli", "dump", "A3", "x.tsl", "--yes"],
            &["katana-cli", "tsl-list", "x.tsl"],
            &["katana-cli", "set", "gain", "40"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok(), "{args:?}");
        }
    }
}
