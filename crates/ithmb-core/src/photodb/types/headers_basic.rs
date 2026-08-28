//! Basic header structs: MHFD, MHSD, MHL, MHII.

use super::endian::{read_u16, read_u32};
use crate::error::DecodeError;

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

/// MHSD — section descriptor, 16 bytes.
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
