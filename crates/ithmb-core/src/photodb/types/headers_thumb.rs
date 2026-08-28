//! Thumbnail-related header structs: MHNI, `MhodString`.

use super::endian::{read_i32, read_u16, read_u32};
use crate::error::DecodeError;

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

        let is_inline = ithmb_off >= 0
            && img_size > 0
            && (i64::from(ithmb_off)).wrapping_add(i64::from(img_size)) <= data.len() as i64;

        if is_inline {
            if remaining < Self::SIZE_CLASSIC {
                return Err(DecodeError::BufferTooShort {
                    expected: Self::SIZE_CLASSIC,
                    actual: remaining,
                });
            }
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
            let packed = read_i32(data, start + 20, little_endian);
            let packed_bits = packed as u32;
            let width = (packed_bits & 0xFFFF) as i32;
            let height = ((packed_bits >> 16) & 0xFFFF) as i32;
            *offset = start + Self::SIZE_EXTENDED;
            Ok(Self {
                magic,
                header_size,
                format_id,
                image_size: img_size,
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
/// The raw bytes following the 4-byte [`MhodHeader`](super::MhodHeader) are
/// treated as null-terminated data (typically UTF-16, but exposed as raw bytes
/// here so the caller can decode as appropriate).
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
