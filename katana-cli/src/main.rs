use clap::Parser;

/// Command-line tool for the BOSS KATANA Gen 3 SysEx protocol.
#[derive(Parser)]
#[command(version)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
