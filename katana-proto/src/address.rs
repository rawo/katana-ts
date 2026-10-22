//! Roland-style addresses: each byte is one base-128 digit.

use std::fmt;
use std::ops::Add;

use crate::ProtoError;

/// Linear value of a Roland address (or RQ1 size) whose wire form is
/// big-endian base-128 digits, so arithmetic carries at 0x7F.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Address(pub u32);

/// RQ1 sizes use the same 7-bit digit encoding as addresses.
pub type Size = Address;

impl Address {
    pub fn from_bytes(bytes: &[u8]) -> Result<Address, ProtoError> {
        let mut value: u32 = 0;
        for (index, &byte) in bytes.iter().enumerate() {
            if byte > 0x7F {
                return Err(ProtoError::NotSevenBit { index, byte });
            }
            value = value
                .checked_mul(128)
                .and_then(|v| v.checked_add(byte as u32))
                .ok_or(ProtoError::AddressOverflow)?;
        }
        Ok(Address(value))
    }

    pub fn to_bytes(self, width: usize) -> Result<Vec<u8>, ProtoError> {
        let mut out = vec![0u8; width];
        let mut rest = self.0;
        for slot in out.iter_mut().rev() {
            *slot = (rest % 128) as u8;
            rest /= 128;
        }
        if rest != 0 {
            return Err(ProtoError::AddressOverflow);
        }
        Ok(out)
    }
}

impl Add<u32> for Address {
    type Output = Address;

    fn add(self, rhs: u32) -> Address {
        Address(self.0 + rhs)
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.to_bytes(4).unwrap_or_else(|_| self.to_bytes(5).unwrap());
        let hex: Vec<String> = digits.iter().map(|b| format!("{b:02X}")).collect();
        write!(f, "{}", hex.join(" "))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtoError;

    #[test]
    fn address_round_trips() {
        let a = Address::from_bytes(&[0x60, 0x00, 0x12, 0x34]).unwrap();
        assert_eq!(a.to_bytes(4).unwrap(), vec![0x60, 0x00, 0x12, 0x34]);
    }

    #[test]
    fn address_carries_across_7bit_digit() {
        let a = Address::from_bytes(&[0x00, 0x00, 0x7F, 0x7F]).unwrap() + 1;
        assert_eq!(a.to_bytes(4).unwrap(), vec![0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn address_rejects_8bit_byte() {
        assert!(matches!(
            Address::from_bytes(&[0x00, 0x80]),
            Err(ProtoError::NotSevenBit { index: 1, byte: 0x80 })
        ));
    }

    #[test]
    fn address_rejects_too_wide() {
        assert!(Address(128 * 128).to_bytes(2).is_err());
    }
}
