use std::time::Duration;

use katana_proto::{all_params, format_value, read_patch, MidirTransport, ParamDef};

pub fn run(port: &str) -> anyhow::Result<()> {
    let mut t = MidirTransport::open(port)?;
    let patch = read_patch(&mut t, Duration::from_secs(2))?;
    for p in all_params() {
        let current = patch.get(p).map_or("?".to_string(), |v| current(p, v));
        println!("{:<32} {:>3}..={:<3} {current}", p.name, p.min, p.max);
    }
    Ok(())
}

/// The raw value, with its name or shown form after it when that differs.
fn current(p: &ParamDef, v: u32) -> String {
    let name = p.values.and_then(|vs| vs.iter().find(|(x, _)| *x == v)).map(|(_, n)| n.to_string());
    let shown = name.unwrap_or_else(|| format_value(p, v));
    if shown == v.to_string() {
        shown
    } else {
        format!("{v} ({shown})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use katana_proto::{find_param, Format};

    #[test]
    fn plain_values_are_not_repeated() {
        let p = all_params().iter().find(|p| p.format == Format::Plain && p.ofs == 0).unwrap();
        assert_eq!(current(p, p.max), p.max.to_string());
    }

    #[test]
    fn names_and_shown_forms_follow_the_raw_value() {
        assert_eq!(current(find_param("amp-type").unwrap(), 1), "1 (CLEAN)");
        let off = all_params().iter().find(|p| p.format == Format::OffBelowZero).unwrap();
        assert_eq!(current(off, 0), "0 (OFF)");
    }
}
