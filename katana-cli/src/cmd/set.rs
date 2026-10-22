use anyhow::{anyhow, Context};
use katana_proto::{find_param, set_param, MidirTransport, ParamDef};

/// `value` is a number or one of the parameter's value names.
fn parse_value(p: &ParamDef, value: &str) -> anyhow::Result<u32> {
    if let Ok(n) = value.parse() {
        return Ok(n);
    }
    p.values
        .and_then(|vs| vs.iter().find(|(_, n)| n.eq_ignore_ascii_case(value)))
        .map(|&(v, _)| v)
        .ok_or_else(|| anyhow!("{value:?} is neither a number nor a value name of {}", p.name))
}

pub fn run(port: &str, name: &str, value: &str) -> anyhow::Result<()> {
    let p = find_param(name).with_context(|| format!("unknown parameter {name:?} (see `katana-cli params`)"))?;
    let v = parse_value(p, value)?;
    let mut t = MidirTransport::open(port)?;
    set_param(&mut t, p, v)?;
    println!("{} = {v}", p.name);
    Ok(())
}
