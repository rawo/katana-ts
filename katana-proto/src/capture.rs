//! Reassembles SysEx messages from captured USB-MIDI traffic.

use crate::ProtoError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    HostToAmp,
    AmpToHost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedMessage {
    pub dir: Direction,
    pub bytes: Vec<u8>,
}

/// Stateful SysEx reassembler over USB-MIDI 1.0 event packets (4 bytes each:
/// cable/CIN, then up to 3 MIDI bytes). State persists across calls, so a
/// message split over several USB transfers is still joined.
#[derive(Default)]
struct Reassembler {
    current: Option<Vec<u8>>,
}

impl Reassembler {
    fn feed(&mut self, packets: &[u8], out: &mut Vec<Vec<u8>>) {
        for p in packets.chunks_exact(4) {
            let (n, ends) = match p[0] & 0x0F {
                0x4 => (3, false),
                0x5 => (1, true),
                0x6 => (2, true),
                0x7 => (3, true),
                _ => continue,
            };
            let bytes = &p[1..1 + n];
            if bytes[0] == 0xF0 {
                self.current = Some(Vec::new());
            }
            let Some(buf) = self.current.as_mut() else { continue };
            buf.extend_from_slice(bytes);
            if ends {
                out.push(self.current.take().unwrap());
            }
        }
    }
}

/// Extracts complete SysEx messages from concatenated USB-MIDI event packets.
/// Non-SysEx packets are ignored; an unterminated trailing message is dropped.
pub fn extract_sysex(packets: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    Reassembler::default().feed(packets, &mut out);
    out
}

/// Loads `tshark -T fields -e usb.endpoint_address.direction -e usb.capdata`
/// output: one URB per line, `<0|1>\t<hex>`, hex optionally `:`-separated.
pub fn load_tshark_export(text: &str) -> Result<Vec<CapturedMessage>, ProtoError> {
    let mut out_host = Reassembler::default();
    let mut out_amp = Reassembler::default();
    let mut msgs = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bad = || ProtoError::BadExport { line: i + 1 };
        if line.trim().is_empty() {
            continue;
        }
        let (dir, hex) = line.split_once('\t').ok_or_else(bad)?;
        let hex: String = hex.trim().chars().filter(|&c| c != ':').collect();
        if hex.is_empty() {
            continue;
        }
        if hex.len() % 2 != 0 {
            return Err(bad());
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|j| u8::from_str_radix(&hex[j..j + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| bad())?;
        let (dir, r) = match dir.trim() {
            "0" => (Direction::HostToAmp, &mut out_host),
            "1" => (Direction::AmpToHost, &mut out_amp),
            _ => return Err(bad()),
        };
        let mut done = Vec::new();
        r.feed(&bytes, &mut done);
        msgs.extend(done.into_iter().map(|bytes| CapturedMessage { dir, bytes }));
    }
    Ok(msgs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtoError;

    const GS_RESET: [u8; 11] = [0xF0, 0x41, 0x10, 0x42, 0x12, 0x40, 0x00, 0x7F, 0x00, 0x41, 0xF7];
    const GS_PACKETS: [u8; 16] =
        [0x04, 0xF0, 0x41, 0x10, 0x04, 0x42, 0x12, 0x40, 0x04, 0x00, 0x7F, 0x00, 0x06, 0x41, 0xF7, 0x00];

    #[test]
    fn extracts_sysex_from_packets() {
        assert_eq!(extract_sysex(&GS_PACKETS), vec![GS_RESET.to_vec()]);
    }

    #[test]
    fn extract_ignores_non_sysex_packets() {
        let mut p = vec![0x0F, 0xFE, 0x00, 0x00, 0x0C, 0xC0, 0x05, 0x00]; // active sensing, program change
        p.extend_from_slice(&GS_PACKETS);
        assert_eq!(extract_sysex(&p), vec![GS_RESET.to_vec()]);
    }

    #[test]
    fn extract_drops_incomplete_trailing_sysex() {
        assert!(extract_sysex(&GS_PACKETS[..8]).is_empty());
    }

    #[test]
    fn loads_export_split_across_urbs() {
        let text = "0\t04:f0:41:10:04:42:12:40\n1\t\n0\t04007f000641f700";
        let msgs = load_tshark_export(text).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].bytes, GS_RESET.to_vec());
        assert!(matches!(msgs[0].dir, Direction::HostToAmp));
    }

    #[test]
    fn export_rejects_garbage_line() {
        assert!(matches!(load_tshark_export("0\tzz"), Err(ProtoError::BadExport { line: 1 })));
    }
}
