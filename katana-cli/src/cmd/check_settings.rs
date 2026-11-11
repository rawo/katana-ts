use std::time::Duration;

use anyhow::bail;
use katana_proto::check::check_settings;
use katana_proto::{read_patch, settings, MidirTransport, ParamDef};

use crate::ask::confirm_stdin;
use crate::backup::{backup_dir, backup_name, write_backup};

const TIMEOUT: Duration = Duration::from_secs(2);

/// Writes every setting from Tone Studio's tables that the app hasn't
/// verified before (or those of one table), reads each back, then restores
/// the current patch. Backs the current patch up first. Never stores.
pub fn run(port: &str, table: Option<&str>, yes: bool) -> anyhow::Result<()> {
    let defs: Vec<&'static ParamDef> = settings::generated()
        .iter()
        .filter(|d| table.is_none_or(|t| settings::table_of(d.addr).is_some_and(|x| x.eq_ignore_ascii_case(t))))
        .collect();
    if defs.is_empty() {
        bail!("no settings to check{}", table.map(|t| format!(" in table {t}")).unwrap_or_default());
    }
    let mut t = MidirTransport::open(port)?;
    let current = read_patch(&mut t, TIMEOUT)?;
    let now = chrono::Local::now().naive_local();
    let backup = write_backup(&backup_dir()?, &backup_name(now, "before-check", &current.name()), &current)?;
    println!("current sound backed up to {}", backup.display());
    let question = format!(
        "Write and read back {} settings in the current sound, then put it back (nothing is stored)?",
        defs.len()
    );
    if !confirm_stdin(&question, yes)? {
        return Ok(());
    }
    let results = check_settings(&mut t, &defs, TIMEOUT)?;
    let failed: Vec<_> = results.iter().filter(|r| !r.passed()).collect();
    for r in &failed {
        let read = r.read.map_or("nothing".to_string(), |v| v.to_string());
        println!("FAIL {:<36} wrote {:>4} read {read}", r.name, r.wrote);
    }
    println!("{} of {} passed; the current sound is restored", results.len() - failed.len(), results.len());
    if !failed.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
