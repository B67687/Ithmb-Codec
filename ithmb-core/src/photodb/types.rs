//! PhotoDB/ArtworkDB binary chunk types: magic constants, endian-aware byte-slice
//! read helpers, and all chunk header structs.
//!
//! Ported from `IthmbCodec.PhotoDb.Types` (C#).

#![allow(missing_docs)]
use crate::error::DecodeError;

// ---------------------------------------------------------------------------
// Known chunk magics (canonical little-endian u32 values)
// ---------------------------------------------------------------------------
// Each is the u32 value of the ASCII magic string when read in the file's
// native endianness.  For a LE file, raw bytes match ASCII; for a BE file,
// raw bytes are byte-swapped but ReadU32BE gives the same canonical value.

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

// ---------------------------------------------------------------------------
// Endian-aware read helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Chunk header structs
// ---------------------------------------------------------------------------

/// MHFD — file header, always 12 bytes. Root container of the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhfdHeader {
    pub magic: u32,
    /// Always 12 (the size of this header).
    pub header_size: u32,
    /// Number of top-level MHSD sections.
    pub entry_count: u32,
}

impl MhfdHeader {
    /// Byte size of the MHFD header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhfdHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let entry_count = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            entry_count,
        })
    }
}

/// MHSD — section descriptor, 16 bytes. Describes a section containing
/// [`entry_count`](MhsdHeader::entry_count) records of type
/// [`record_type`](MhsdHeader::record_type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhsdHeader {
    pub magic: u32,
    /// Total section size including child entries.
    pub header_size: u32,
    /// Section index within parent.
    pub index: u16,
    /// Type of records: 1 = Photos, 4 = Thumbnails, etc.
    pub record_type: u16,
    /// Number of records in this section.
    pub entry_count: u32,
}

impl MhsdHeader {
    /// Byte size of the MHSD header.
    pub const SIZE: usize = 16;

    /// Parse an [`MhsdHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let index = read_u16(data, *offset + 8, little_endian);
        let record_type = read_u16(data, *offset + 10, little_endian);
        let entry_count = read_u32(data, *offset + 12, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            index,
            record_type,
            entry_count,
        })
    }
}

/// MHL — photo list entry, 12 bytes. Groups photo items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhlHeader {
    pub magic: u32,
    pub header_size: u32,
    /// Number of child items.
    pub count: u32,
}

impl MhlHeader {
    /// Byte size of the MHL header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhlHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let count = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            count,
        })
    }
}

/// MHII — photo item, 12 bytes. Identifies a single photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhiiHeader {
    pub magic: u32,
    pub header_size: u32,
    /// Unique photo identifier.
    pub photo_id: u32,
}

impl MhiiHeader {
    /// Byte size of the MHII header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhiiHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let photo_id = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            photo_id,
        })
    }
}

/// MHBA — album container, 12 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhbaHeader {
    pub magic: u32,
    pub header_size: u32,
    /// Unique album identifier.
    pub album_id: u32,
}

impl MhbaHeader {
    /// Byte size of the MHBA header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhbaHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let album_id = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            album_id,
        })
    }
}

/// MHIA — album item container, 12 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhiaHeader {
    pub magic: u32,
    pub header_size: u32,
    /// Unique artwork identifier.
    pub artwork_id: u32,
}

impl MhiaHeader {
    /// Byte size of the MHIA header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhiaHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let artwork_id = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            artwork_id,
        })
    }
}

/// MHIF — file info container, 12 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhifHeader {
    pub magic: u32,
    pub header_size: u32,
    /// Type of file info.
    pub info_type: u32,
}

impl MhifHeader {
    /// Byte size of the MHIF header.
    pub const SIZE: usize = 12;

    /// Parse an [`MhifHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let magic = read_u32(data, *offset, little_endian);
        let header_size = read_u32(data, *offset + 4, little_endian);
        let info_type = read_u32(data, *offset + 8, little_endian);
        *offset += Self::SIZE;
        Ok(Self {
            magic,
            header_size,
            info_type,
        })
    }
}

/// MHOD — variable-length data record, 4-byte header.
///
/// `tag = 1` indicates a null-terminated string (see [`MhodString`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhodHeader {
    /// 1 = `MhodString` (null-terminated UTF-16?).
    pub tag: u16,
    /// Size of the data following this header.
    pub size: u16,
}

impl MhodHeader {
    /// Byte size of the MHOD header.
    pub const SIZE: usize = 4;

    /// Parse an [`MhodHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than [`Self::SIZE`] bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < Self::SIZE {
            return Err(DecodeError::BufferTooShort {
                expected: Self::SIZE,
                actual: remaining,
            });
        }
        let tag = read_u16(data, *offset, little_endian);
        let size = read_u16(data, *offset + 2, little_endian);
        *offset += Self::SIZE;
        Ok(Self { tag, size })
    }
}

/// MHNI — thumbnail info entry, 36 bytes (iPod Classic) or 76 bytes (Apple
/// TV/Animal).
///
/// This is the critical record that maps a [`format_id`](MhniHeader::format_id)
/// to a byte range within the corresponding `.ithmb` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhniHeader {
    pub magic: u32,
    /// 36 (classic) or 76 (Apple TV/Animal).
    pub header_size: u32,
    /// Matches profile keys (e.g. 1019).
    pub format_id: i32,
    /// Byte count of the `.ithmb` data blob.
    pub image_size: i32,
    /// Byte offset into the `.ithmb` file. `-1` for external (Apple TV/Animal).
    pub ithmb_offset: i32,
    /// Image width in pixels.
    pub width: i32,
    /// Image height in pixels.
    pub height: i32,
    /// Horizontal padding (alignment).
    pub h_padding: i32,
    /// Vertical padding (alignment).
    pub v_padding: i32,
}

impl MhniHeader {
    /// Byte size of the classic (iPod Classic) MHNI header.
    pub const SIZE_CLASSIC: usize = 36;
    /// Byte size of the extended (Apple TV/Animal) MHNI header.
    pub const SIZE_EXTENDED: usize = 76;

    /// Parse an [`MhniHeader`] from `data` at `offset`, advancing `offset` past
    /// the header.
    ///
    /// Detects the inline (iPod Classic) vs. external (Apple TV/Animal) layout
    /// variant automatically.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than 28 bytes are
    /// available from `offset` (the minimum needed for variant detection), or
    /// if fewer bytes are available than the detected variant requires.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn parse(data: &[u8], offset: &mut usize, little_endian: bool) -> Result<Self, DecodeError> {
        let start = *offset;
        let remaining = data.len().saturating_sub(start);
        // Need at least 28 bytes to read the common prefix (Magic + HeaderSize
        // + 8 bytes padding + FormatId + IthmbOffset + ImageSize).
        if remaining < 28 {
            return Err(DecodeError::BufferTooShort {
                expected: 28,
                actual: remaining,
            });
        }

        let magic = read_u32(data, start, little_endian);
        let header_size = read_u32(data, start + 4, little_endian);
        let format_id = read_i32(data, start + 16, little_endian);
        let ithmb_off = read_i32(data, start + 20, little_endian);
        let img_size = read_i32(data, start + 24, little_endian);

        // Detect variant: if IthmbOffset at +20 is reasonable (< data length)
        // and ImageSize at +24 is > 0, treat as inline (iPod Classic).
        // Otherwise mark as external (Apple TV / Animal).
        let is_inline = ithmb_off >= 0
            && img_size > 0
            && (i64::from(ithmb_off)).wrapping_add(i64::from(img_size)) <= data.len() as i64;

        if is_inline {
            // iPod Classic 6G/7G — inline data in .ithmb files.
            if remaining < Self::SIZE_CLASSIC {
                return Err(DecodeError::BufferTooShort {
                    expected: Self::SIZE_CLASSIC,
                    actual: remaining,
                });
            }
            // Width at +34 (u16), Height at +32 (u16).
            let width = i32::from(read_u16(data, start + 34, little_endian));
            let height = i32::from(read_u16(data, start + 32, little_endian));
            *offset = start + Self::SIZE_CLASSIC;
            Ok(Self {
                magic,
                header_size,
                format_id,
                image_size: img_size,
                ithmb_offset: ithmb_off,
                width,
                height,
                h_padding: 0,
                v_padding: 0,
            })
        } else {
            // Apple TV / Animal — external .ithmb files or no data.
            // Width/Height packed at +20: low 16 bits = width, high 16 bits = height.
            let packed = read_i32(data, start + 20, little_endian);
            let packed_bits = packed as u32;
            let width = (packed_bits & 0xFFFF) as i32;
            let height = ((packed_bits >> 16) & 0xFFFF) as i32;
            *offset = start + Self::SIZE_EXTENDED;
            Ok(Self {
                magic,
                header_size,
                format_id,
                image_size: 0,
                ithmb_offset: -1,
                width,
                height,
                h_padding: 0,
                v_padding: 0,
            })
        }
    }
}

/// A null-terminated string payload carried by an MHOD chunk with tag = 1.
///
/// The raw bytes following the 4-byte [`MhodHeader`] are treated as
/// null-terminated data (typically UTF-16, but exposed as raw bytes here
/// so the caller can decode as appropriate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MhodString {
    /// The raw bytes of the string payload (including the null terminator if
    /// present in the source).
    pub raw: Vec<u8>,
}

impl MhodString {
    /// Parse an [`MhodString`] by reading `byte_count` bytes from `data` at
    /// `offset`, then advancing `offset` past those bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BufferTooShort`] if fewer than `byte_count` bytes
    /// are available from `offset`.
    pub fn parse(data: &[u8], offset: &mut usize, byte_count: usize) -> Result<Self, DecodeError> {
        let remaining = data.len().saturating_sub(*offset);
        if remaining < byte_count {
            return Err(DecodeError::BufferTooShort {
                expected: byte_count,
                actual: remaining,
            });
        }
        let raw = data[*offset..*offset + byte_count].to_vec();
        *offset += byte_count;
        Ok(Self { raw })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_magic() {
        assert!(is_known_magic(MHFD));
        assert!(is_known_magic(MHSD));
        assert!(is_known_magic(MHL));
        assert!(is_known_magic(MHII));
        assert!(is_known_magic(MHNI));
        assert!(is_known_magic(MHBA));
        assert!(is_known_magic(MHIA));
        assert!(is_known_magic(MHIF));
        assert!(is_known_magic(MHOD));
        assert!(!is_known_magic(0xDEAD_BEEF));
        assert!(!is_known_magic(0));
    }

    #[test]
    fn test_read_u32_le() {
        let buf = [0x6d, 0x68, 0x66, 0x64, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(read_u32_le(&buf, 0), 0x6466_686d);
        assert_eq!(read_u32(&buf, 0, true), 0x6466_686d);
    }

    #[test]
    fn test_read_u32_be() {
        let buf = [0x4d, 0x48, 0x46, 0x44, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(read_u32_be(&buf, 0), 0x4d48_4644);
        assert_eq!(read_u32(&buf, 0, false), 0x4d48_4644);
    }

    #[test]
    fn test_read_i32() {
        let buf = [0xff, 0xff, 0xff, 0xff];
        assert_eq!(read_i32(&buf, 0, true), -1);
    }

    #[test]
    fn test_read_u16_le() {
        let buf = [0x34, 0x12];
        assert_eq!(read_u16(&buf, 0, true), 0x1234);
    }

    #[test]
    fn test_read_u16_be() {
        let buf = [0x12, 0x34];
        assert_eq!(read_u16(&buf, 0, false), 0x1234);
    }

    #[test]
    fn test_read_u32_convenience_equivalence() {
        let buf = [0x6d, 0x68, 0x66, 0x64];
        assert_eq!(read_u32_le(&buf, 0), read_u32(&buf, 0, true));
        assert_eq!(read_u32_be(&buf, 0), read_u32(&buf, 0, false));
    }

    #[test]
    fn test_mhfd_parse_le() {
        let data: &[u8] = b"mhfd\x0c\x00\x00\x00\x02\x00\x00\x00extra";
        let mut offset = 0;
        let hdr = MhfdHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHFD);
        assert_eq!(hdr.header_size, 12);
        assert_eq!(hdr.entry_count, 2);
        assert_eq!(offset, 12);
    }

    #[test]
    fn test_mhfd_parse_be() {
        // In a BE file, magic bytes are the BE representation of the canonical
        // LE magic value, so ReadU32BE recovers the canonical constant.
        // MHFD = 0x6466686d -> BE bytes: [0x64, 0x66, 0x68, 0x6d].
        let data: &[u8] = &[
            0x64, 0x66, 0x68, 0x6d, // magic (BE of 0x6466_686d)
            0x00, 0x00, 0x00, 0x0c, // header_size = 12 (BE)
            0x00, 0x00, 0x00, 0x02, // entry_count = 2 (BE)
        ];
        let mut offset = 0;
        let hdr = MhfdHeader::parse(data, &mut offset, false).unwrap();
        // BE read_u32 on BE bytes recovers the canonical LE magic.
        assert_eq!(hdr.magic, MHFD);
        assert_eq!(hdr.header_size, 12);
        assert_eq!(hdr.entry_count, 2);
        assert_eq!(offset, 12);
    }

    #[test]
    fn test_mhsd_parse_le() {
        let data: &[u8] = b"mhsd\x10\x00\x00\x00\x01\x00\x04\x00\x03\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhsdHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHSD);
        assert_eq!(hdr.header_size, 16);
        assert_eq!(hdr.index, 1);
        assert_eq!(hdr.record_type, 4);
        assert_eq!(hdr.entry_count, 3);
        assert_eq!(offset, 16);
    }

    #[test]
    fn test_mhl_parse_le() {
        let data: &[u8] = b"mhli\x0c\x00\x00\x00\x03\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhlHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHL);
        assert_eq!(hdr.count, 3);
    }

    #[test]
    fn test_mhii_parse_le() {
        let data: &[u8] = b"mhii\x0c\x00\x00\x00\x2a\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhiiHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHII);
        assert_eq!(hdr.photo_id, 42);
    }

    #[test]
    fn test_mh_common_parse_le() {
        let ba: &[u8] = b"mhba\x0c\x00\x00\x00\x05\x00\x00\x00";
        let mut offset = 0;
        let ba_hdr = MhbaHeader::parse(ba, &mut offset, true).unwrap();
        assert_eq!(ba_hdr.magic, MHBA);
        assert_eq!(ba_hdr.album_id, 5);

        let ia: &[u8] = b"mhia\x0c\x00\x00\x00\x07\x00\x00\x00";
        let mut offset = 0;
        let ia_hdr = MhiaHeader::parse(ia, &mut offset, true).unwrap();
        assert_eq!(ia_hdr.magic, MHIA);
        assert_eq!(ia_hdr.artwork_id, 7);

        let mhif: &[u8] = b"mhif\x0c\x00\x00\x00\x01\x00\x00\x00";
        let mut offset = 0;
        let mhif_hdr = MhifHeader::parse(mhif, &mut offset, true).unwrap();
        assert_eq!(mhif_hdr.magic, MHIF);
        assert_eq!(mhif_hdr.info_type, 1);
    }

    #[test]
    fn test_mhod_parse_le() {
        let data: &[u8] = &[0x01, 0x00, 0x0a, 0x00, 0xff, 0xff];
        let mut offset = 0;
        let hdr = MhodHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.tag, 1);
        assert_eq!(hdr.size, 10);
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_mhni_parse_inline_le() {
        // Classic 36-byte layout with inline data (iPod Classic).
        let mut data = vec![0u8; 5000];
        data[0..4].copy_from_slice(b"mhni");
        data[4..8].copy_from_slice(&[36, 0, 0, 0]); // header_size = 36 LE
        data[16..20].copy_from_slice(&[0xfb, 0x03, 0, 0]); // format_id = 1019 LE
        data[20..24].copy_from_slice(&[0x80, 0, 0, 0]); // ithmb_offset = 128 LE
        data[24..28].copy_from_slice(&[0x00, 0x10, 0, 0]); // image_size = 4096 LE
        data[32..34].copy_from_slice(&[0xe0, 0x01]); // height = 480 LE (u16)
        data[34..36].copy_from_slice(&[0xd0, 0x02]); // width = 720 LE (u16)

        let mut offset = 0;
        let hdr = MhniHeader::parse(&data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHNI);
        assert_eq!(hdr.header_size, 36);
        assert_eq!(hdr.format_id, 1019);
        assert_eq!(hdr.ithmb_offset, 128);
        assert_eq!(hdr.image_size, 4096);
        assert_eq!(hdr.width, 720);
        assert_eq!(hdr.height, 480);
        assert_eq!(hdr.h_padding, 0);
        assert_eq!(hdr.v_padding, 0);
        assert_eq!(offset, 36);
    }

    #[test]
    fn test_mhni_parse_external_le() {
        // Extended 76-byte layout with external data (Apple TV / Animal).
        let mut data = vec![0u8; 76];
        data[0..4].copy_from_slice(b"mhni");
        data[4..8].copy_from_slice(&[76, 0, 0, 0]); // header_size = 76 LE
        data[16..20].copy_from_slice(&[0xfb, 0x03, 0, 0]); // format_id = 1019 LE
        data[20..24].copy_from_slice(&[0xe0, 0x01, 0x2c, 0x01]); // packed: width=480, height=300 LE

        let mut offset = 0;
        let hdr = MhniHeader::parse(&data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHNI);
        assert_eq!(hdr.header_size, 76);
        assert_eq!(hdr.format_id, 1019);
        assert_eq!(hdr.ithmb_offset, -1);
        assert_eq!(hdr.image_size, 0);
        // Width in low 16 bits, height in high 16 bits of packed field at +20.
        assert_eq!(hdr.width, 0x01e0);
        assert_eq!(hdr.height, 0x012c);
    }

    #[test]
    fn test_mhfd_buffer_too_short() {
        let data: &[u8] = b"mhfd\x0c\x00"; // only 6 bytes
        let mut offset = 0;
        let result = MhfdHeader::parse(data, &mut offset, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_mhod_string() {
        let data: &[u8] = b"Hello\x00world";
        let mut offset = 0;
        let s = MhodString::parse(data, &mut offset, 6).unwrap();
        assert_eq!(s.raw, b"Hello\x00");
        assert_eq!(offset, 6);
    }

    #[test]
    fn test_parse_magic_equivalence() {
        // Verify that parsing a magic as LE vs BE yields different u32 values
        // from the same raw bytes, but magic constants compare correctly.
        let buf = [0x6d, 0x68, 0x66, 0x64]; // "mhfd" in raw bytes
        let le = read_u32(&buf, 0, true);
        let be = read_u32(&buf, 0, false);
        assert_eq!(le, MHFD);
        assert_ne!(le, be);
        assert_ne!(be, MHFD);
    }
}
