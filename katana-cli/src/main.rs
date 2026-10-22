mod cmd;

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
    /// Model ID bytes as hex, used to decode captures (UNVERIFIED default from the official app).
    #[arg(long, global = true, default_value = "010507")]
    model_id: String,
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
    /// Print the SysEx messages in a capture export (.tsv).
    Extract { file: PathBuf },
    /// Show bytes that differ between the DT1 data in two files.
    Diff { a: PathBuf, b: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let header = cmd::header(&cli.model_id)?;
    match cli.command {
        Command::Ports => cmd::ports::run(),
        Command::Identity => cmd::identity::run(&cli.port),
        Command::Monitor => cmd::monitor::run(&cli.port, &header),
        Command::Extract { file } => cmd::extract::run(&file),
        Command::Diff { a, b } => cmd::diff::run(&header, &a, &b),
    }
}
