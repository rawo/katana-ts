//! MIDI Universal Non-realtime Identity Request / Reply.

use crate::ProtoError;

pub const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityReply {
    pub device_id: u8,
    pub manufacturer: u8,
    pub family: [u8; 2],
    pub member: [u8; 2],
    pub version: [u8; 4],
}

/// Parses `F0 7E <dev> 06 02 <mfr> <fam×2> <mem×2> <ver×4> F7`.
pub fn parse_identity_reply(msg: &[u8]) -> Result<IdentityReply, ProtoError> {
    if msg.len() < 5 || msg[0] != 0xF0 || msg[1] != 0x7E || msg[3] != 0x06 || msg[4] != 0x02 {
        return Err(ProtoError::NotSysEx);
    }
    if msg.len() != 15 || msg[14] != 0xF7 {
        return Err(ProtoError::Truncated);
    }
    Ok(IdentityReply {
        device_id: msg[2],
        manufacturer: msg[5],
        family: [msg[6], msg[7]],
        member: [msg[8], msg[9]],
        version: [msg[10], msg[11], msg[12], msg[13]],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_reply() {
        let m = [0xF0, 0x7E, 0x10, 0x06, 0x02, 0x41, 0x33, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0xF7];
        let r = parse_identity_reply(&m).unwrap();
        assert_eq!((r.manufacturer, r.family, r.member), (0x41, [0x33, 0x03], [0x00, 0x00]));
    }
}
