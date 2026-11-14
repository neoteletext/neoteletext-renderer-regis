//! The two checksums a PNG file needs.

/// CRC-32 as PNG uses it (PNG specification section 5.5): the reflected
/// polynomial 0xEDB88320, initial value and final inversion of all ones.
pub(crate) fn crc32(parts: &[&[u8]]) -> u32 {
    const POLYNOMIAL: u32 = 0xEDB8_8320;
    let mut crc = u32::MAX;
    for byte in parts.iter().flat_map(|part| part.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ POLYNOMIAL } else { crc >> 1 };
        }
    }
    !crc
}

/// Adler-32 (RFC 1950 section 8.2).
pub(crate) fn adler32(data: &[u8]) -> u32 {
    /// The largest prime below 2^16.
    const MODULUS: u32 = 65_521;
    let (mut low, mut high) = (1u32, 0u32);
    for byte in data {
        low = low.wrapping_add(u32::from(*byte)).checked_rem(MODULUS).unwrap_or(0);
        high = high.wrapping_add(low).checked_rem(MODULUS).unwrap_or(0);
    }
    (high << 16) | low
}

#[cfg(test)]
mod tests {
    use super::{adler32, crc32};

    #[test]
    fn the_standard_check_values_hold() {
        // The CRC-32 and Adler-32 of "123456789" and of "Wikipedia".
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }
}
