//! PhotoDB/ArtworkDB tree walker — extracts `.ithmb` thumbnail entries from the
//! binary chunk container format.
//!
//! Ported from `IthmbCodec.PhotoDb.Core` (C#).
#![allow(
    clippy::wildcard_imports,
    clippy::doc_markdown,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::error::DecodeError;
use crate::photodb::types::*;
use crate::profile_db::ProfileDb;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum recursion depth when walking the chunk tree.
const MAX_DEPTH: u32 = 64;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Describes how a PhotoDB entry carries its pixel data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoDbEntryKind {
    /// Pixel data is embedded inline in [`PhotoDbEntry::data`].
    Inline,
    /// Entry references an external `.ithmb` file
    /// (`ithmb_offset == -1`, `image_size > 0`).
    ExternalReference,
    /// No pixel data is available for this entry.
    NoData,
}

/// Optional metadata extracted from MHOD/MHIF chunks during tree-walking.
#[derive(Debug, Clone, Default)]
pub struct PhotoDbMetadata {
    /// Decoded MHOD null-terminated strings (e.g. album names, photo dates).
    pub mhod_strings: Vec<String>,
    /// Raw bytes of the last-seen MHIF file-info data.
    pub mhif_data: Vec<u8>,
    /// Info type from the MHIF header, if any MHIF was seen.
    pub mhif_info_type: Option<u32>,
}

/// A single thumbnail entry extracted from a PhotoDB/ArtworkDB binary file.
#[derive(Debug, Clone)]
pub struct PhotoDbEntry {
    /// Format identifier matching a profile prefix (e.g. 1019).
    pub format_id: i32,
    /// Raw thumbnail pixel data (empty for external `.ithmb` references).
    pub data: Vec<u8>,
    /// Byte offset of the pixel data within the `.ithmb` file.
    /// `-1` for external (Apple TV / Animal) entries.
    pub ithmb_offset: i32,
    /// Byte size of the image data.
    pub image_size: i32,
    /// Image width in pixels.
    pub width: i32,
    /// Image height in pixels.
    pub height: i32,
    /// How this entry carries its pixel data.
    pub kind: PhotoDbEntryKind,
    /// Path to the external `.ithmb` file (only for ExternalReference entries).
    pub ithmb_path: String,
    /// Metadata captured from parent MHOD/MHIF chunks.
    pub metadata: PhotoDbMetadata,
}

// ---------------------------------------------------------------------------
// Endianness detection
// ---------------------------------------------------------------------------

/// Detect the endianness of a PhotoDB/ArtworkDB file by examining the first 4
/// raw bytes.
///
/// Returns `Some(true)` for little-endian (`"mhfd"`), `Some(false)` for
/// big-endian (`"dfhm"`), and `None` if the prefix matches neither pattern.
#[must_use]
pub fn detect_endianness(data: &[u8]) -> Option<bool> {
    if data.len() < 4 {
        return None;
    }
    // LE file: raw bytes are "mhfd" = [0x6d, 0x68, 0x66, 0x64].
    if data[0] == 0x6d && data[1] == 0x68 && data[2] == 0x66 && data[3] == 0x64 {
        return Some(true);
    }
    // BE file: raw bytes are "dfhm" = [0x64, 0x66, 0x68, 0x6d].
    if data[0] == 0x64 && data[1] == 0x66 && data[2] == 0x68 && data[3] == 0x6d {
        return Some(false);
    }
    None
}

/// Quick magic check — returns `true` if `data` starts with a valid PhotoDB
/// magic prefix in either endianness.
#[must_use]
pub fn can_open_photodb(data: &[u8]) -> bool {
    detect_endianness(data).is_some()
}

// ---------------------------------------------------------------------------
// Main parse entry-point
// ---------------------------------------------------------------------------

/// Parse a PhotoDB/ArtworkDB binary buffer and extract all MHNI thumbnail
/// entries into `entries`.
///
/// # Errors
///
/// Returns [`DecodeError::InvalidFormat`] if the data does not start with a
/// recognised MHFD magic, or if a required chunk header is truncated beyond
/// the buffer boundary.
pub fn try_parse_photodb(data: &[u8], entries: &mut Vec<PhotoDbEntry>) -> Result<(), DecodeError> {
    let little_endian = detect_endianness(data)
        .ok_or_else(|| DecodeError::InvalidFormat("not a valid PhotoDB/ArtworkDB file".into()))?;

    // Parse the MHFD root header.
    let mut offset = 0usize;
    let mhfd = MhfdHeader::parse(data, &mut offset, little_endian)?;
    if mhfd.magic != MHFD {
        return Err(DecodeError::InvalidFormat("MHFD header has wrong magic".into()));
    }
    // `offset` is now 12 (end of the 12-byte MHFD header). Children begin here
    // and extend to the end of the data buffer.

    walk_entries(data, offset, data.len(), little_endian, entries, 0)?;

    // Post-process: trim JPEG entries that have no matching profile.
    let db = ProfileDb::load_builtin().ok();
    for entry in entries.iter_mut() {
        let has_profile = db.as_ref().and_then(|d| d.get(entry.format_id)).is_some();
        if !has_profile && entry.data.len() >= 2 && entry.data[0] == 0xFF && entry.data[1] == 0xD8 {
            trim_jpeg(&mut entry.data);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tree-walking helpers
// ---------------------------------------------------------------------------

/// Check whether the data at `start` (within the range [`start`, `end`))
/// appears to be a valid child chunk.
///
/// Verifies that:
/// 1. There is room for at least an 8-byte header (magic + header_size).
/// 2. The `header_size` field at `start + 4` is >= 8.
/// 3. The magic at `start` is a recognised chunk type.
#[must_use]
fn has_child_chunks(data: &[u8], start: usize, end: usize, little_endian: bool) -> bool {
    if start + 8 > end || start + 8 > data.len() {
        return false;
    }
    let hdr_size = read_u32(data, start + 4, little_endian);
    if hdr_size < 8 {
        return false;
    }
    let magic = read_u32(data, start, little_endian);
    is_known_magic(magic)
}

/// Recursively walk the chunk tree within [`start`, `end`), collecting MHNI
/// entries into `entries`.
///
/// Stops silently when `depth` exceeds [`MAX_DEPTH`] to guard against
/// pathological or cyclic chunk graphs.
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] if a chunk header declares a size
/// that extends beyond the data buffer, or if a required header cannot be
/// parsed.
#[allow(clippy::too_many_lines)]
fn walk_entries(
    data: &[u8],
    start: usize,
    end: usize,
    little_endian: bool,
    entries: &mut Vec<PhotoDbEntry>,
    depth: u32,
) -> Result<(), DecodeError> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    if start >= end || start >= data.len() {
        return Ok(());
    }

    let mut pos = start;
    while pos < end && pos < data.len() {
        // Every chunk needs at least 8 bytes (magic + header_size).
        if pos + 8 > data.len() || pos + 8 > end {
            break;
        }

        let magic = read_u32(data, pos, little_endian);
        let hdr_size = read_u32(data, pos + 4, little_endian);

        // Validate: must be a known magic with a reasonable header size.
        if !is_known_magic(magic) || hdr_size < 8 {
            break;
        }

        // Total span of this chunk, including its header and all children.
        let hdr_size_usize = hdr_size as usize;
        let chunk_end = pos.saturating_add(hdr_size_usize).min(data.len());
        if chunk_end <= pos {
            break;
        }
        // Default next_pos equals chunk_end. Handlers (e.g. MHII) may
        // override to advance past their total_len instead of just hdr_size.
        let mut next_pos = chunk_end;
        match magic {
            MHFD => {
                // Root file header. Parse to validate and advance, then walk
                // children past the 12-byte header.
                let mut hdr_pos = pos;
                let _ = MhfdHeader::parse(data, &mut hdr_pos, little_endian)?;
                walk_entries(data, hdr_pos, chunk_end, little_endian, entries, depth + 1)?;
            }

            MHSD => {
                // Section descriptor. Children start after the 16-byte header.
                let mut hdr_pos = pos;
                let _ = MhsdHeader::parse(data, &mut hdr_pos, little_endian)?;
                let child_start = pos + MhsdHeader::SIZE;
                if child_start < chunk_end && has_child_chunks(data, child_start, chunk_end, little_endian) {
                    walk_entries(data, child_start, chunk_end, little_endian, entries, depth + 1)?;
                }
            }

            MHL => {
                // Photo list. Children start after the 12-byte header.
                let mut hdr_pos = pos;
                let _ = MhlHeader::parse(data, &mut hdr_pos, little_endian)?;
                let child_start = pos + MhlHeader::SIZE;
                if child_start < chunk_end {
                    walk_entries(data, child_start, chunk_end, little_endian, entries, depth + 1)?;
                }
            }

            MHII => {
                // Photo item container. Header is 12 bytes, but the total
                // extent (including children) is the u32 value at `pos + 8`.
                let mut hdr_pos = pos;
                let _ = MhiiHeader::parse(data, &mut hdr_pos, little_endian)?;
                let total_len = read_u32(data, pos + 8, little_endian) as usize;
                let child_start = pos + MhiiHeader::SIZE;
                let child_end = pos.saturating_add(total_len).min(data.len());
                if child_start < child_end {
                    walk_entries(data, child_start, child_end, little_endian, entries, depth + 1)?;
                }
                // Advance pos past total_len, not hdr_size, so the outer
                // loop doesn't re-visit children as siblings.
                next_pos = child_end;
            }

            MHNI => {
                // Thumbnail info entry — leaf node. Parse the header and
                // extract the inline data if present.
                let mut mhni_pos = pos;
                let mhni = MhniHeader::parse(data, &mut mhni_pos, little_endian)?;

                let (entry_data, kind) = if mhni.ithmb_offset >= 0 && mhni.image_size > 0 {
                    let off = mhni.ithmb_offset as usize;
                    let sz = mhni.image_size as usize;
                    if off.saturating_add(sz) <= data.len() {
                        (data[off..off + sz].to_vec(), PhotoDbEntryKind::Inline)
                    } else {
                        (Vec::new(), PhotoDbEntryKind::NoData)
                    }
                } else if mhni.ithmb_offset == -1 && mhni.image_size > 0 {
                    (Vec::new(), PhotoDbEntryKind::ExternalReference)
                } else {
                    (Vec::new(), PhotoDbEntryKind::NoData)
                };

                entries.push(PhotoDbEntry {
                    format_id: mhni.format_id,
                    data: entry_data,
                    ithmb_offset: mhni.ithmb_offset,
                    image_size: mhni.image_size,
                    width: mhni.width,
                    height: mhni.height,
                    kind,
                    ithmb_path: String::new(),
                    metadata: PhotoDbMetadata::default(),
                });
            }

            MHBA => {
                // Album container. Children start after the 12-byte header.
                let mut hdr_pos = pos;
                let _ = MhbaHeader::parse(data, &mut hdr_pos, little_endian)?;
                let child_start = pos + MhbaHeader::SIZE;
                if child_start < chunk_end {
                    walk_entries(data, child_start, chunk_end, little_endian, entries, depth + 1)?;
                }
            }

            MHIA => {
                // Album item container. Children start after the 12-byte header.
                let mut hdr_pos = pos;
                let _ = MhiaHeader::parse(data, &mut hdr_pos, little_endian)?;
                let child_start = pos + MhiaHeader::SIZE;
                if child_start < chunk_end {
                    walk_entries(data, child_start, chunk_end, little_endian, entries, depth + 1)?;
                }
            }

            MHIF | MHOD => {
                // File info and metadata records — attach to the most
                // recently pushed entry, if any. These chunks always follow
                // their associated MHNI within the same container.
                if let Some(last) = entries.last_mut() {
                    if magic == MHOD {
                        // MHOD: 4-byte header (tag + size) then raw data.
                        let mhod_start = pos + 8;
                        let mut mhod_pos = mhod_start;
                        if mhod_pos + MhodHeader::SIZE <= chunk_end {
                            let mhod_hdr = MhodHeader::parse(data, &mut mhod_pos, little_endian)?;
                            if mhod_hdr.tag == 1 && mhod_hdr.size > 0 {
                                let mhod_s = MhodString::parse(data, &mut mhod_pos, mhod_hdr.size as usize)?;
                                // Decode as UTF-8, stripping trailing nulls.
                                let trimmed = mhod_s.raw.iter().take_while(|&&b| b != 0).copied().collect::<Vec<_>>();
                                if let Ok(s) = String::from_utf8(trimmed) {
                                    // Use as ithmb_path for ExternalReference entries.
                                    if last.kind == PhotoDbEntryKind::ExternalReference && last.ithmb_path.is_empty() {
                                        last.ithmb_path.clone_from(&s);
                                    }
                                    last.metadata.mhod_strings.push(s);
                                }
                            }
                        }
                    } else {
                        // MHIF: info_type at pos+8, data starts at pos+12 (past
                        // the 12-byte magic+header_size+info_type header).
                        let if_data_start = pos + MhifHeader::SIZE;
                        if if_data_start <= chunk_end {
                            let info_type = read_u32(data, pos + 8, little_endian);
                            last.metadata.mhif_info_type = Some(info_type);
                            if if_data_start < chunk_end {
                                last.metadata.mhif_data = data[if_data_start..chunk_end].to_vec();
                            }
                        }
                    }
                }
            }

            _ => {
                // Unreachable in practice because we validated `is_known_magic`
                // above, but break defensively.
                break;
            }
        }

        pos = next_pos;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// JPEG trimming
// ---------------------------------------------------------------------------

/// Trim a JPEG byte buffer at the first EOI marker (`0xFF`, `0xD9`) searching
/// backwards from the end.
///
/// JPEG streams are self-delimiting by their EOI marker. If raw pixel data
/// follows the JPEG stream (common in PhotoDB inline entries where the
/// `image_size` includes both the JPEG and any padding), this removes the
/// trailing garbage.
fn trim_jpeg(data: &mut Vec<u8>) {
    if data.len() < 2 {
        return;
    }
    // Search backwards from the end for the EOI marker 0xFF, 0xD9.
    let mut i = data.len().saturating_sub(2);
    loop {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            data.truncate(i + 2);
            return;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
}

// ---------------------------------------------------------------------------
// Format ID naming
// ---------------------------------------------------------------------------

/// Return a human-readable display name for the given `format_id` by looking
/// it up in the built-in profile database.
///
/// When the format ID is known, the name includes the profile prefix,
/// dimensions, and encoding (e.g. `"F1019.720x480 rgb565"`). Unknown IDs
/// are labelled with a fallback string.
#[must_use]
pub fn get_format_id_name(format_id: i32) -> String {
    match ProfileDb::load_builtin() {
        Ok(db) => match db.get(format_id) {
            Some(profile) => {
                format!(
                    "F{}.{}x{} {}",
                    profile.prefix,
                    profile.width,
                    profile.height,
                    format!("{:?}", profile.encoding).to_lowercase(),
                )
            }
            None => format!("F{format_id} (unknown)"),
        },
        Err(_) => format!("F{format_id} (no profile db)"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // -- detect_endianness ---------------------------------------------------

    #[test]
    fn detect_endianness_le() {
        let data = b"mhfd\x0c\x00\x00\x00";
        assert_eq!(detect_endianness(data), Some(true));
    }

    #[test]
    fn detect_endianness_be() {
        let data: &[u8] = &[0x64, 0x66, 0x68, 0x6d, 0x00, 0x00, 0x00, 0x0c];
        assert_eq!(detect_endianness(data), Some(false));
    }

    #[test]
    fn detect_endianness_invalid() {
        assert_eq!(detect_endianness(b"xxxx"), None);
    }

    #[test]
    fn detect_endianness_too_short() {
        assert_eq!(detect_endianness(b"abc"), None);
        assert_eq!(detect_endianness(b""), None);
    }

    // -- can_open_photodb ----------------------------------------------------

    #[test]
    fn can_open_photodb_le() {
        assert!(can_open_photodb(b"mhfd..."));
    }

    #[test]
    fn can_open_photodb_be() {
        let data: &[u8] = &[0x64, 0x66, 0x68, 0x6d];
        assert!(can_open_photodb(data));
    }

    #[test]
    fn can_open_photodb_invalid() {
        assert!(!can_open_photodb(b"xxxx"));
        assert!(!can_open_photodb(b""));
    }

    // -- has_child_chunks ----------------------------------------------------

    #[test]
    fn has_child_chunks_recognises_mhsd() {
        // Simulate a valid MHSD header at `start`.
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(b"mhsd");
        // header_size at +4 = 16
        data[4..8].copy_from_slice(&[16, 0, 0, 0]);
        assert!(has_child_chunks(&data, 0, 32, true));
    }

    #[test]
    fn has_child_chunks_rejects_short_buffer() {
        let data = b"mhsd";
        assert!(!has_child_chunks(data, 0, 4, true));
    }

    #[test]
    fn has_child_chunks_rejects_unknown_magic() {
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(b"xxxx");
        data[4..8].copy_from_slice(&[16, 0, 0, 0]);
        assert!(!has_child_chunks(&data, 0, 16, true));
    }

    #[test]
    fn has_child_chunks_rejects_tiny_header_size() {
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(b"mhsd");
        data[4..8].copy_from_slice(&[4, 0, 0, 0]); // hdr_size < 8
        assert!(!has_child_chunks(&data, 0, 16, true));
    }

    // -- try_parse_photodb / walk_entries ------------------------------------

    /// Build a minimal LE PhotoDB with one MHSD section containing one MHL
    /// containing one MHII containing one MHNI (classic inline).
    fn build_minimal_photodb_le() -> Vec<u8> {
        // Layout:
        //   MHFD  (12 bytes)
        //   MHSD  (16 bytes)
        //   MHL   (12 bytes, magic "mhli")
        //   MHII  (12 bytes)
        //   MHNI  (36 bytes classic inline)
        // We place the inline pixel data right after the chunk tree.
        // The MHNI's ithmb_offset points to that trailing data.
        let tree_size = 12 + 16 + 12 + 12 + 36;
        let pixel_data: &[u8] = &[
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01,
            0x00, 0x00, 0xFF, 0xD9, // JPEG EOI
            0xCC, 0xCC, 0xCC, 0xCC,
        ]; // trailing garbage
        let pixel_offset = tree_size;

        let mut data = vec![0u8; tree_size + pixel_data.len()];

        let mut off = 0usize;

        // MHFD: magic(4) + header_size(4, 12) + entry_count(4, 1)
        data[off..off + 4].copy_from_slice(b"mhfd");
        data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]); // hdr_size = 12
        data[off + 8..off + 12].copy_from_slice(&[1, 0, 0, 0]); // entry_count = 1
        off += 12;

        // MHSD: magic(4) + hdr_size(4, 16) + index(2, 0) + rec_type(2, 4) +
        //        entry_count(4, 1)
        // total section size = 16 + 12 + 12 + 36 = 76
        let mhsd_total: u32 = 16 + 12 + 12 + 36;
        data[off..off + 4].copy_from_slice(b"mhsd");
        data[off + 4..off + 8].copy_from_slice(&mhsd_total.to_le_bytes());
        data[off + 8..off + 10].copy_from_slice(&[0, 0]); // index = 0
        data[off + 10..off + 12].copy_from_slice(&[4, 0]); // record_type = 4
        data[off + 12..off + 16].copy_from_slice(&[1, 0, 0, 0]); // entry_count = 1
        off += 16;

        // MHL: magic "mhli"(4) + hdr_size(4, 12) + count(4, 1)
        data[off..off + 4].copy_from_slice(b"mhli");
        data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]); // hdr_size = 12
        data[off + 8..off + 12].copy_from_slice(&[1, 0, 0, 0]); // count = 1
        off += 12;

        // MHII: magic(4) + hdr_size(4, 12) + total_len(4, 12 + 36 = 48)
        let mhii_total: u32 = 12 + 36;
        data[off..off + 4].copy_from_slice(b"mhii");
        data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]); // hdr_size = 12
        data[off + 8..off + 12].copy_from_slice(&mhii_total.to_le_bytes()); // total_len = 48
        off += 12;

        // MHNI: classic inline, 36 bytes
        // format_id at +16 = 1019, ithmb_offset at +20 = pixel_offset,
        // image_size at +24 = 22 (JPEG until EOI), width at +34 = 16,
        // height at +32 = 16
        let img_size = 22i32; // just the JPEG SOI..EOI part
        data[off..off + 4].copy_from_slice(b"mhni");
        data[off + 4..off + 8].copy_from_slice(&[36, 0, 0, 0]); // hdr_size = 36
        // +8..+16 padding (zeros)
        data[off + 16..off + 20].copy_from_slice(&[0xFB, 0x03, 0, 0]); // format_id = 1019 LE
        data[off + 20..off + 24].copy_from_slice(&i32::try_from(pixel_offset).unwrap().to_le_bytes()); // ithmb_offset
        data[off + 24..off + 28].copy_from_slice(&img_size.to_le_bytes()); // image_size
        // +28..+32 reserved / padding
        data[off + 32..off + 34].copy_from_slice(&[16, 0]); // height = 16 LE u16
        data[off + 34..off + 36].copy_from_slice(&[16, 0]); // width = 16 LE u16
        off += 36;

        // Pixel data follows the chunk tree.
        data[off..off + pixel_data.len()].copy_from_slice(pixel_data);

        data
    }

    #[test]
    fn try_parse_photodb_extracts_inline_mhni() {
        let photodb = build_minimal_photodb_le();
        let mut entries = Vec::new();
        try_parse_photodb(&photodb, &mut entries).unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.format_id, 1019);
        assert_eq!(entry.ithmb_offset, 88);
        // off after tree (12+16+12+12+36 = 88)
        let pixel_offset: usize = 12 + 16 + 12 + 12 + 36;
        assert_eq!(entry.ithmb_offset as usize, pixel_offset);
        // image_size is 22, but JPEG trimming with no profile will cut at EOI
        // profile 1019 exists in the built-in DB, so trimming should NOT happen.
        assert_eq!(entry.image_size, 22);
        assert_eq!(entry.width, 16);
        assert_eq!(entry.height, 16);
        // Since 1019 exists in profiles, data should NOT be trimmed
        assert_eq!(entry.data.len(), 22);
    }

    #[test]
    fn try_parse_photodb_invalid_magic() {
        let data = b"xxxx";
        let mut entries = Vec::new();
        let result = try_parse_photodb(data, &mut entries);
        assert!(result.is_err());
    }

    #[test]
    fn try_parse_photodb_empty() {
        let data = b"";
        let mut entries = Vec::new();
        let result = try_parse_photodb(data, &mut entries);
        assert!(result.is_err());
    }

    #[test]
    fn try_parse_photodb_trims_jpeg_for_unknown_profile() {
        // Build a minimal PhotoDB where the MHNI uses an unknown format_id
        // (e.g. 9999) and the data starts with JPEG SOI. The post-process
        // should trim trailing garbage at EOI.
        let mut photodb = build_minimal_photodb_le();
        // Overwrite the format_id to 9999 (unknown).
        // MHNI is at offset 12 + 16 + 12 + 12 = 52
        let mhni_offset = 52usize;
        photodb[mhni_offset + 16..mhni_offset + 20].copy_from_slice(&[0x0F, 0x27, 0, 0]); // 9999 LE

        let mut entries = Vec::new();
        try_parse_photodb(&photodb, &mut entries).unwrap();

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.format_id, 9999);
        // Data should be trimmed to just JPEG SOI..EOI (22 bytes)
        assert_eq!(entry.data.len(), 22);
        assert_eq!(&entry.data[..2], &[0xFF, 0xD8]);
        assert_eq!(&entry.data[entry.data.len() - 2..], &[0xFF, 0xD9]);
    }

    // -- has_child_chunks (additional) ---------------------------------------

    #[test]
    fn has_child_chunks_outside_end_range() {
        let mut data = vec![0u8; 16];
        data[0..4].copy_from_slice(b"mhsd");
        data[4..8].copy_from_slice(&[16, 0, 0, 0]);
        // `end` is before the header
        assert!(!has_child_chunks(&data, 0, 7, true));
    }

    // -- Depth limit ---------------------------------------------------------

    #[test]
    fn walk_entries_depth_limit_returns_early() {
        // Depth > MAX_DEPTH should return Ok(()) without processing.
        let data = b"mhfd\x0c\x00\x00\x00\x00\x00\x00\x00";
        let mut entries = Vec::new();
        let result = walk_entries(data, 0, data.len(), true, &mut entries, MAX_DEPTH + 1);
        assert!(result.is_ok());
        assert!(entries.is_empty());
    }

    #[test]
    fn walk_entries_empty_range() {
        let data = b"";
        let mut entries = Vec::new();
        let result = walk_entries(data, 0, 0, true, &mut entries, 0);
        assert!(result.is_ok());
        assert!(entries.is_empty());
    }

    // -- get_format_id_name --------------------------------------------------

    #[test]
    fn get_format_id_name_known() {
        let name = get_format_id_name(1007);
        assert!(name.contains("1007"));
        assert!(name.contains("480"));
        assert!(name.contains("864"));
    }

    #[test]
    fn get_format_id_name_unknown() {
        let name = get_format_id_name(9999);
        assert!(name.contains("unknown") || name.contains("9999"));
    }

    // -- trim_jpeg -----------------------------------------------------------

    #[test]
    fn trim_jpeg_finds_eoi() {
        let mut data = vec![0xFF, 0xD8, 0x00, 0x00, 0xFF, 0xD9, 0xCC, 0xCC, 0xCC];
        trim_jpeg(&mut data);
        assert_eq!(data.len(), 6);
        assert_eq!(&data[4..6], &[0xFF, 0xD9]);
    }

    #[test]
    fn trim_jpeg_no_eoi() {
        let mut data = vec![0xFF, 0xD8, 0x00, 0x00, 0x00, 0x00];
        trim_jpeg(&mut data);
        assert_eq!(data.len(), 6); // unchanged
    }

    #[test]
    fn trim_jpeg_short_buffer() {
        let mut data = vec![0xFF];
        trim_jpeg(&mut data);
        assert_eq!(data.len(), 1); // unchanged
    }
}
