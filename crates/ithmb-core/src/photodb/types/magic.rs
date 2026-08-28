//! Known chunk magics (canonical little-endian u32 values).
//!
//! Each is the u32 value of the ASCII magic string when read in the file's
//! native endianness.

/// `"mhfd"` as a little-endian u32.
pub const MHFD: u32 = 0x6466_686d;
/// `"mhsd"` as a little-endian u32.
pub const MHSD: u32 = 0x6473_686d;
/// `"mhli"` as a little-endian u32 (four-character padded magic for MHL).
pub const MHL: u32 = 0x696c_686d;
/// `"mhii"` as a little-endian u32.
pub const MHII: u32 = 0x6969_686d;
/// `"mhni"` as a little-endian u32.
pub const MHNI: u32 = 0x696e_686d;
/// `"mhba"` as a little-endian u32.
pub const MHBA: u32 = 0x6162_686d;
/// `"mhia"` as a little-endian u32.
pub const MHIA: u32 = 0x6169_686d;
/// `"mhif"` as a little-endian u32.
pub const MHIF: u32 = 0x6669_686d;
/// `"mhod"` as a little-endian u32.
pub const MHOD: u32 = 0x646f_686d;

/// Returns `true` if `magic` is a known PhotoDB/ArtworkDB chunk magic (LE u32).
#[must_use]
#[inline]
pub fn is_known_magic(magic: u32) -> bool {
    matches!(magic, MHFD | MHSD | MHL | MHII | MHNI | MHBA | MHIA | MHIF | MHOD)
}
