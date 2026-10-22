/// Roland checksum over the address and data bytes of an RQ1/DT1 message:
/// the value that brings their sum to 0 modulo 128.
pub fn roland_checksum(bytes: &[u8]) -> u8 {
    let sum: u32 = bytes.iter().map(|&b| b as u32).sum();
    ((128 - sum % 128) % 128) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    // Roland GS Reset, published by Roland: F0 41 10 42 12 40 00 7F 00 41 F7
    #[test]
    fn checksum_matches_gs_reset() {
        assert_eq!(roland_checksum(&[0x40, 0x00, 0x7F, 0x00]), 0x41);
    }

    #[test]
    fn checksum_of_zero_sum_is_zero() {
        assert_eq!(roland_checksum(&[0x00, 0x00]), 0x00);
    }
}
