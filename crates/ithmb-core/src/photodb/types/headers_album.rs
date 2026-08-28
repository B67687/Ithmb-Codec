//! Album-related header structs: MHBA, MHIA, MHIF, MHOD.

use super::endian::{read_u16, read_u32};
use crate::error::DecodeError;

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
/// `tag = 1` indicates a null-terminated string (see [`MhodString`](super::MhodString)).
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
