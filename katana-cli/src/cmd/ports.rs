use katana_proto::MidirTransport;

pub fn run() -> anyhow::Result<()> {
    for p in MidirTransport::list_ports()? {
        println!("{p}");
    }
    Ok(())
}
