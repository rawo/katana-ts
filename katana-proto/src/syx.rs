//! `.syx` files: SysEx messages stored back to back.

use crate::ProtoError;

/// Splits concatenated `F0 … F7` messages. Any byte outside a message, or an
/// unterminated message, is an error.
pub fn read_syx(bytes: &[u8]) -> Result<Vec<Vec<u8>>, ProtoError> {
    let mut msgs = Vec::new();
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        match (start, b) {
            (None, 0xF0) => start = Some(i),
            (None, _) | (Some(_), 0xF0) => return Err(ProtoError::BadSyx { offset: i }),
            (Some(s), 0xF7) => {
                msgs.push(bytes[s..=i].to_vec());
                start = None;
            }
            (Some(_), _) => {}
        }
    }
    match start {
        Some(s) => Err(ProtoError::BadSyx { offset: s }),
        None => Ok(msgs),
    }
}

pub fn write_syx(msgs: &[Vec<u8>]) -> Vec<u8> {
    msgs.concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, Header, ProtoError};

    const GS_RESET: [u8; 11] = [0xF0, 0x41, 0x10, 0x42, 0x12, 0x40, 0x00, 0x7F, 0x00, 0x41, 0xF7];

    #[test]
    fn syx_round_trips() {
        let m = vec![GS_RESET.to_vec(), GS_RESET.to_vec()];
        assert_eq!(read_syx(&write_syx(&m)).unwrap(), m);
    }

    #[test]
    fn syx_rejects_unterminated() {
        assert!(read_syx(&GS_RESET[..5]).is_err());
    }

    #[test]
    fn syx_rejects_bytes_outside_message() {
        let mut b = GS_RESET.to_vec();
        b.push(0x00);
        assert!(read_syx(&b).is_err());
    }

    #[test]
    fn syx_rejects_bad_checksum() {
        let mut b = GS_RESET.to_vec();
        b[9] = 0x00;
        let msgs = read_syx(&b).unwrap();
        let h = Header { device_id: 0x10, model_id: vec![0x42], addr_width: 3 };
        assert!(matches!(decode(&h, &msgs[0]), Err(ProtoError::BadChecksum { .. })));
    }
}
