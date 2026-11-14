//! Checked conversions between integer types, so no cast can truncate.

/// A count or index as `i32`, saturating.
pub(crate) fn to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// An `i64` as `i32`, saturating at both ends.
pub(crate) fn narrow(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

/// A non-negative `i32` as `usize`; negatives become zero.
pub(crate) fn to_usize(value: i32) -> usize {
    usize::try_from(value).unwrap_or(0)
}

/// A non-negative `i32` as `u32`; negatives become zero.
pub(crate) fn to_u32(value: i32) -> u32 {
    u32::try_from(value).unwrap_or(0)
}

/// A non-negative `i32` as `u64`; negatives become zero.
pub(crate) fn to_u64(value: i32) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// The low eight bits of a value known to be in `0..=255`; others give 255.
pub(crate) fn to_u8(value: i32) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}
