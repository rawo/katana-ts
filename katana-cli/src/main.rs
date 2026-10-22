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
    /// Read the amp's current patch into a .syx file.
    Dump { out: PathBuf },
    /// Write a .syx patch (from `dump`) into the amp's current patch.
    Load { file: PathBuf },
    /// Read the current patch, write it back, read again, and compare.
    Verify,
    /// Set one parameter of the current patch.
    Set { name: String, value: String },
    /// List known parameters with their current values.
    Params,
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
        Command::Dump { out } => cmd::dump::run(&cli.port, &out),
        Command::Load { file } => cmd::load::run(&cli.port, &file),
        Command::Verify => cmd::verify::run(&cli.port),
        Command::Set { name, value } => cmd::set::run(&cli.port, &name, &value),
        Command::Params => cmd::params::run(&cli.port),
    }
}
