//! Endian-aware read helpers for binary chunk parsing.

/// Read a `u32` from `data` at `offset`, interpreting bytes as little-endian
/// or big-endian.
///
/// # Panics
///
/// Panics if `offset + 4 > data.len()`.
#[must_use]
#[inline]
pub fn read_u32(data: &[u8], offset: usize, little_endian: bool) -> u32 {
    if little_endian {
        read_u32_le(data, offset)
    } else {
        read_u32_be(data, offset)
    }
}

/// Read an `i32` from `data` at `offset`, interpreting bytes as little-endian
/// or big-endian.
///
/// # Panics
///
/// Panics if `offset + 4 > data.len()`.
#[must_use]
#[inline]
#[allow(clippy::cast_possible_wrap)]
pub fn read_i32(data: &[u8], offset: usize, little_endian: bool) -> i32 {
    read_u32(data, offset, little_endian) as i32
}

/// Read a `u16` from `data` at `offset`, interpreting bytes as little-endian
/// or big-endian.
///
/// # Panics
///
/// Panics if `offset + 2 > data.len()`.
#[must_use]
#[inline]
pub fn read_u16(data: &[u8], offset: usize, little_endian: bool) -> u16 {
    if little_endian {
        u16::from(data[offset]) | (u16::from(data[offset + 1]) << 8)
    } else {
        (u16::from(data[offset]) << 8) | u16::from(data[offset + 1])
    }
}

/// Read a `u32` from `data` at `offset` in big-endian byte order.
///
/// # Panics
///
/// Panics if `offset + 4 > data.len()`.
#[must_use]
#[inline]
pub fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset]) << 24
        | u32::from(data[offset + 1]) << 16
        | u32::from(data[offset + 2]) << 8
        | u32::from(data[offset + 3])
}

/// Read a `u32` from `data` at `offset` in little-endian byte order.
///
/// # Panics
///
/// Panics if `offset + 4 > data.len()`.
#[must_use]
#[inline]
pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset])
        | (u32::from(data[offset + 1]) << 8)
        | (u32::from(data[offset + 2]) << 16)
        | (u32::from(data[offset + 3]) << 24)
}
