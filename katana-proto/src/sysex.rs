//! Roland "Exclusive" envelope: RQ1 (data request) and DT1 (data set).
//!
//! `F0 41 <dev> <model-id…> <cmd> <addr> <data | size> <checksum> F7`

use crate::{roland_checksum, Address, ProtoError, Size};

const SOX: u8 = 0xF0;
const EOX: u8 = 0xF7;
const ROLAND: u8 = 0x41;
const CMD_RQ1: u8 = 0x11;
const CMD_DT1: u8 = 0x12;

/// Per-device framing parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub device_id: u8,
    pub model_id: Vec<u8>,
    pub addr_width: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Rq1 { addr: Address, size: Size },
    Dt1 { addr: Address, data: Vec<u8> },
}

pub fn encode(h: &Header, c: &Command) -> Result<Vec<u8>, ProtoError> {
    let (cmd, body) = match c {
        Command::Rq1 { addr, size } => {
            let mut body = addr.to_bytes(h.addr_width)?;
            body.extend(size.to_bytes(h.addr_width)?);
            (CMD_RQ1, body)
        }
        Command::Dt1 { addr, data } => {
            if let Some((index, &byte)) = data.iter().enumerate().find(|(_, &b)| b > 0x7F) {
                return Err(ProtoError::NotSevenBit { index, byte });
            }
            let mut body = addr.to_bytes(h.addr_width)?;
            body.extend_from_slice(data);
            (CMD_DT1, body)
        }
    };
    let mut msg = vec![SOX, ROLAND, h.device_id];
    msg.extend_from_slice(&h.model_id);
    msg.push(cmd);
    msg.extend_from_slice(&body);
    msg.push(roland_checksum(&body));
    msg.push(EOX);
    Ok(msg)
}

/// Decodes one complete SysEx message. The device ID byte is not checked,
/// so replies from a unit with a non-default ID still decode.
pub fn decode(h: &Header, msg: &[u8]) -> Result<Command, ProtoError> {
    if msg.first() != Some(&SOX) {
        return Err(ProtoError::NotSysEx);
    }
    let prefix = 3 + h.model_id.len() + 1;
    // prefix + address + at least one data byte + checksum + EOX
    if msg.len() < prefix + h.addr_width + 3 || msg.last() != Some(&EOX) {
        return Err(ProtoError::Truncated);
    }
    if msg[1] != ROLAND {
        return Err(ProtoError::WrongManufacturer(msg[1]));
    }
    let model = &msg[3..3 + h.model_id.len()];
    if model != h.model_id.as_slice() {
        return Err(ProtoError::WrongModel { expected: h.model_id.clone(), got: model.to_vec() });
    }
    let cmd = msg[prefix - 1];
    let body = &msg[prefix..msg.len() - 2];
    let got = msg[msg.len() - 2];
    let expected = roland_checksum(body);
    if got != expected {
        return Err(ProtoError::BadChecksum { expected, got });
    }
    if let Some((index, &byte)) = body.iter().enumerate().find(|(_, &b)| b > 0x7F) {
        return Err(ProtoError::NotSevenBit { index: prefix + index, byte });
    }
    let addr = Address::from_bytes(&body[..h.addr_width])?;
    let rest = &body[h.addr_width..];
    match cmd {
        CMD_DT1 => Ok(Command::Dt1 { addr, data: rest.to_vec() }),
        CMD_RQ1 if rest.len() == h.addr_width => Ok(Command::Rq1 { addr, size: Address::from_bytes(rest)? }),
        CMD_RQ1 => Err(ProtoError::Truncated),
        other => Err(ProtoError::UnknownCommand(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Address, ProtoError};

    fn gs() -> Header {
        Header { device_id: 0x10, model_id: vec![0x42], addr_width: 3 }
    }
    const GS_RESET: [u8; 11] = [0xF0, 0x41, 0x10, 0x42, 0x12, 0x40, 0x00, 0x7F, 0x00, 0x41, 0xF7];

    #[test]
    fn encodes_gs_reset() {
        let c = Command::Dt1 { addr: Address::from_bytes(&[0x40, 0x00, 0x7F]).unwrap(), data: vec![0x00] };
        assert_eq!(encode(&gs(), &c).unwrap(), GS_RESET.to_vec());
    }

    #[test]
    fn decodes_gs_reset() {
        assert_eq!(
            decode(&gs(), &GS_RESET).unwrap(),
            Command::Dt1 { addr: Address::from_bytes(&[0x40, 0x00, 0x7F]).unwrap(), data: vec![0x00] }
        );
    }

    #[test]
    fn rq1_round_trips() {
        let h = Header { device_id: 0x10, model_id: vec![0x00, 0x00, 0x00, 0x33], addr_width: 4 };
        let c = Command::Rq1 { addr: Address::from_bytes(&[0x60, 0, 0, 0]).unwrap(), size: Address(0x200) };
        assert_eq!(decode(&h, &encode(&h, &c).unwrap()).unwrap(), c);
    }

    #[test]
    fn rejects_bad_checksum() {
        let mut m = GS_RESET;
        m[9] = 0x42;
        assert!(matches!(decode(&gs(), &m), Err(ProtoError::BadChecksum { expected: 0x41, got: 0x42 })));
    }

    #[test]
    fn rejects_truncated() {
        assert!(matches!(decode(&gs(), &GS_RESET[..6]), Err(ProtoError::Truncated)));
    }

    #[test]
    fn rejects_wrong_model() {
        let h = Header { model_id: vec![0x43], ..gs() };
        assert!(matches!(decode(&h, &GS_RESET), Err(ProtoError::WrongModel { .. })));
    }

    #[test]
    fn encode_rejects_8bit_data() {
        let c = Command::Dt1 { addr: Address(0), data: vec![0x80] };
        assert!(encode(&gs(), &c).is_err());
    }
}
