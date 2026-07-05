//! PhotoDB/ArtworkDB integrity checker and binary builder.
//!
//! Ported from `IthmbCodec.PhotoDb.Serialization` (C#).
//!
//! # Integrity Check
//!
//! `integrity_check_photodb` validates the structure of a PhotoDB/ArtworkDB
//! binary, returning a list of issues (empty = clean).
//!
//! # Binary Builder
//!
//! `try_build_photodb` constructs a synthetic PhotoDB/ArtworkDB binary from
//! a list of `BuildEntry` descriptors.
#![allow(
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unnecessary_trailing_comma,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::single_match_else,
    clippy::cast_possible_wrap
)]

use crate::error::DecodeError;
use crate::photodb::types::{
    MHBA, MHFD, MHIA, MHIF, MHII, MHL, MHNI, MHOD, MHSD, is_known_magic, read_i32, read_u32, read_u32_be, read_u32_le,
};
use crate::profile::Profile;
use crate::profile_db::ProfileDb;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A single entry to be built into a synthetic PhotoDB/ArtworkDB binary.
///
/// Each entry specifies a format ID (matching a known profile) and its raw
/// pixel data.
#[derive(Debug, Clone)]
pub struct BuildEntry {
    /// Format/profile identifier (e.g. 1019, 1007).
    pub format_id: i32,
    /// Raw pixel data for this entry.
    pub data: Vec<u8>,
}

/// An MHNI entry discovered during integrity walking.
#[derive(Debug, Clone)]
struct MhniEntry {
    format_id: i32,
    ithmb_offset: i32,
    image_size: i32,
}

/// Mutable state threaded through the integrity walk tree.
struct WalkState {
    entries: Vec<MhniEntry>,
    max_chunk_end: usize,
    issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check whether the bytes at `pos` form a valid chunk header.
///
/// A valid chunk has at least 8 bytes remaining (magic + header_size),
/// a `header_size` >= 8, and a known magic constant.
fn has_child_chunks(data: &[u8], pos: usize, end: usize, little_endian: bool) -> bool {
    if pos + 8 > end || pos + 8 > data.len() {
        return false;
    }
    let hdr_size = read_u32(data, pos + 4, little_endian);
    if hdr_size < 8 {
        return false;
    }
    let magic = read_u32(data, pos, little_endian);
    is_known_magic(magic)
}

/// Fast check: do the first 4 bytes match the ASCII "mhfd" magic in either
/// endianness representation?
fn can_open_photodb(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    // LE representation: "mhfd" = [0x6d, 0x68, 0x66, 0x64]
    // BE representation: bytes are byte-swapped version of the canonical LE
    // u32 value 0x6466686d → [0x64, 0x66, 0x68, 0x6d]
    let magic_le = read_u32_le(data, 0);
    let magic_be = read_u32_be(data, 0);
    magic_le == MHFD || magic_be == MHFD
}

/// Detect endianness of a PhotoDB binary.
///
/// Returns `Some(true)` for little-endian, `Some(false)` for big-endian,
/// and `None` if detection fails.
fn detect_endianness(data: &[u8]) -> Option<bool> {
    if data.len() < 8 {
        return None;
    }
    let le_magic = read_u32_le(data, 0);
    let be_magic = read_u32_be(data, 0);
    if le_magic == MHFD {
        // Verify the header_size field looks reasonable in LE
        let hdr_size = read_u32_le(data, 4);
        if hdr_size >= 12 && (hdr_size as usize) <= data.len() {
            return Some(true);
        }
    }
    if be_magic == MHFD {
        let hdr_size = read_u32_be(data, 4);
        if hdr_size >= 12 && (hdr_size as usize) <= data.len() {
            return Some(false);
        }
    }
    // Fallback: try to determine by checking which interpretation gives a
    // plausible header_size.
    if le_magic == MHFD {
        return Some(true);
    }
    if be_magic == MHFD {
        return Some(false);
    }
    None
}

/// Compute the total extent (end offset) of a chunk based on its magic and
/// header fields.
fn chunk_total_end(data: &[u8], pos: usize, magic: u32, _hdr_size: u32, little_endian: bool) -> usize {
    let hdr_size = read_u32(data, pos + 4, little_endian) as usize;
    match magic {
        // MHII stores its total length at offset +8.
        MHII => {
            if pos + 12 <= data.len() {
                pos + read_u32(data, pos + 8, little_endian) as usize
            } else {
                pos + 12
            }
        }
        // MHNI: use total_len at +8 if it is larger than header_size and
        // within bounds (handles our synthetic files with padding).
        // For real classic files (header_size = 36, total_len undefined),
        // falls back to header_size.
        MHNI => {
            if pos + 12 <= data.len() {
                let total_len = read_u32(data, pos + 8, little_endian) as usize;
                if total_len > hdr_size && total_len <= data.len().saturating_sub(pos) {
                    pos + total_len
                } else {
                    pos + hdr_size
                }
            } else {
                pos + hdr_size
            }
        }
        // All other types use header_size.
        _ => pos + hdr_size,
    }
}

// ---------------------------------------------------------------------------
// Integrity walk tree (recursive)
// ---------------------------------------------------------------------------

/// Recursively walk the chunk tree, validating structure and collecting MHNI
/// entries.
///
/// `offset` is the starting byte position, `end` is the exclusive upper bound
/// for this walk level (from the parent container's `header_size` or
/// `data.len()` for the root). `depth` prevents runaway recursion (max 64).
#[allow(clippy::too_many_arguments)]
fn integrity_walk_tree(
    data: &[u8],
    offset: usize,
    end: usize,
    depth: usize,
    little_endian: bool,
    state: &mut WalkState,
) {
    if depth > 64 {
        state.issues.push("Maximum chunk nesting depth (64) exceeded".into());
        return;
    }

    let mut pos = offset;
    while pos < end && pos + 8 <= data.len() {
        // Check for a valid chunk at this position.
        if !has_child_chunks(data, pos, end, little_endian) {
            // If we're still before `end` but no valid chunk, there may be
            // trailing garbage — the caller handles this after the walk.
            break;
        }

        let magic = read_u32(data, pos, little_endian);
        let hdr_size = read_u32(data, pos + 4, little_endian) as usize;
        let chunk_end = chunk_total_end(data, pos, magic, hdr_size as u32, little_endian);
        state.max_chunk_end = state.max_chunk_end.max(chunk_end);

        // Guard: hdr_size must be at least 8 (magic + header_size)
        if hdr_size < 8 {
            state
                .issues
                .push(format!("Chunk at offset {pos} has invalid header_size={hdr_size}"));
            pos = chunk_end;
            continue;
        }

        match magic {
            MHFD => {
                // Descend into MHFD children at +12 (past the 12-byte header).
                let children_start = pos + 12;
                let children_end = chunk_end.min(end);
                if children_start < children_end {
                    integrity_walk_tree(data, children_start, children_end, depth + 1, little_endian, state);
                }
            }
            MHSD => {
                // Descend into MHSD children at +16 (past the 16-byte header).
                let children_start = pos + 16;
                let children_end = chunk_end.min(end);
                if children_start < children_end {
                    integrity_walk_tree(data, children_start, children_end, depth + 1, little_endian, state);
                }
            }
            MHL => {
                // Descend into MHL children at +12 (past the 12-byte header).
                let children_start = pos + 12;
                let children_end = chunk_end.min(end);
                if children_start < children_end {
                    integrity_walk_tree(data, children_start, children_end, depth + 1, little_endian, state);
                }
            }
            MHII | MHIF | MHOD => {
                // Leaf node — total_len at +8 already used for chunk_end, or skip.
            }
            MHBA | MHIA => {
                // Descend into album/album-item children at +12.
                let children_start = pos + 12;
                let children_end = chunk_end.min(end);
                if children_start < children_end {
                    integrity_walk_tree(data, children_start, children_end, depth + 1, little_endian, state);
                }
            }
            MHNI => {
                // Collect entry data.
                if hdr_size >= 36 && pos + 28 <= data.len() {
                    let format_id = read_i32(data, pos + 16, little_endian);
                    let ithmb_offset = read_i32(data, pos + 20, little_endian);
                    let image_size = read_i32(data, pos + 24, little_endian);
                    state.entries.push(MhniEntry {
                        format_id,
                        ithmb_offset,
                        image_size,
                    });
                }
                // Also track the chunk end for boundary tracking.
            }
            _ => {
                // Unknown magic (should never happen after has_child_chunks
                // check, but handle defensively).
                state
                    .issues
                    .push(format!("Unknown chunk magic 0x{magic:08x} at offset {pos}"));
            }
        }

        // Advance to the next chunk. Use chunk_end; if it didn't advance,
        // force 1-byte to avoid infinite loop.
        let next_pos = chunk_end.max(pos + 1);
        if next_pos <= pos {
            break;
        }
        pos = next_pos;
    }
}

// ---------------------------------------------------------------------------
// LE byte-by-byte writers (no std::io::Write, no BinaryWriter, no byteorder
// crate)
// ---------------------------------------------------------------------------

fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = (value & 0xff) as u8;
    buf[offset + 1] = ((value >> 8) & 0xff) as u8;
    buf[offset + 2] = ((value >> 16) & 0xff) as u8;
    buf[offset + 3] = ((value >> 24) & 0xff) as u8;
}

fn write_i32_le(buf: &mut [u8], offset: usize, value: i32) {
    write_u32_le(buf, offset, value as u32);
}

fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = (value & 0xff) as u8;
    buf[offset + 1] = ((value >> 8) & 0xff) as u8;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate the structure of a PhotoDB/ArtworkDB binary.
///
/// Returns a list of human-readable issue strings. An empty `Vec` means the
/// binary appears structurally sound.
///
/// # Checks performed
///
/// 1. Minimum size (at least 4 bytes)
/// 2. Magic signature (must start with MHFD in either endianness)
/// 3. Endianness detection
/// 4. Try full parse (profile DB load) — note failure but continue
/// 5. Validate MHFD header (size >= 12, within bounds)
/// 6. Walk chunk tree: collect MHNI entries, track max boundary
/// 7. Validate known format IDs against profile DB
/// 8. Check overlapping MHNI ithmb offset ranges
/// 9. Check trailing garbage after last known chunk boundary
///
/// This function has no external I/O. All operations are on the supplied byte
/// slice.
#[must_use]
pub fn integrity_check_photodb(data: &[u8]) -> Vec<String> {
    let mut issues: Vec<String> = Vec::new();

    // 1. Minimum size check.
    if data.len() < 4 {
        issues.push(format!("Data too short: {} bytes (need at least 4)", data.len()));
        return issues;
    }

    // 2. Magic check.
    if !can_open_photodb(data) {
        issues.push("File does not start with valid MHFD magic".into());
        return issues;
    }

    // 3. Endianness detection.
    let little_endian = match detect_endianness(data) {
        Some(le) => le,
        None => {
            issues.push("Cannot determine endianness from MHFD header".into());
            return issues;
        }
    };

    // 4. Try full parse — try to load profiles, note failure but continue.
    let profile_db = match ProfileDb::load_builtin() {
        Ok(db) => Some(db),
        Err(e) => {
            issues.push(format!(
                "Cannot load profile DB (continuing with limited validation): {e}"
            ));
            None
        }
    };

    // 5. Validate MHFD header.
    if data.len() < 12 {
        issues.push(format!(
            "Data too short for MHFD header: {} bytes (need at least 12)",
            data.len()
        ));
        return issues;
    }
    let mhfd_hdr_size = read_u32(data, 4, little_endian) as usize;
    if mhfd_hdr_size < 12 {
        issues.push(format!("MHFD header_size ({mhfd_hdr_size}) is less than minimum 12"));
    }
    if mhfd_hdr_size > data.len() {
        issues.push(format!(
            "MHFD header_size ({mhfd_hdr_size}) exceeds data length ({})",
            data.len()
        ));
    }
    // entry_count validation (informational)
    let mhfd_entry_count = if data.len() >= 12 {
        read_u32(data, 8, little_endian)
    } else {
        0
    };

    // 6. Walk chunk tree.
    let mut state = WalkState {
        entries: Vec::new(),
        max_chunk_end: mhfd_hdr_size, // start from MHFD's claimed extent
        issues: Vec::new(),
    };

    // Walk starts at offset 0 (the root MHFD) and covers the entire data
    // length. Individual sections bound their children via their own
    // header_size fields.
    integrity_walk_tree(data, 0, data.len(), 0, little_endian, &mut state);

    // Collect walker-reported issues.
    issues.append(&mut state.issues);

    // 7. Validate known format IDs against profile DB.
    if let Some(ref db) = profile_db {
        for (idx, entry) in state.entries.iter().enumerate() {
            if db.get(entry.format_id).is_none() {
                issues.push(format!(
                    "Entry {idx}: unknown format ID {} (no matching profile)",
                    entry.format_id
                ));
            }
        }
    }

    // 8. Check overlapping MHNI ithmb offset ranges.
    for i in 0..state.entries.len() {
        for j in (i + 1)..state.entries.len() {
            let a = &state.entries[i];
            let b = &state.entries[j];
            // Two ranges overlap if a starts before b ends and b starts before
            // a ends.
            if a.ithmb_offset >= 0 && b.ithmb_offset >= 0 && a.image_size > 0 && b.image_size > 0 {
                let a_start = a.ithmb_offset as usize;
                let a_end = a_start + a.image_size as usize;
                let b_start = b.ithmb_offset as usize;
                let b_end = b_start + b.image_size as usize;

                if a_start < b_end && b_start < a_end {
                    issues.push(format!(
                        "Entries {i} and {j} have overlapping ithmb offset ranges \
                         (entry {i}: [{a_start}..{a_end}), entry {j}: [{b_start}..{b_end}))"
                    ));
                }
            }
        }
    }

    // 9. Check trailing garbage after last known chunk boundary.
    let effective_end = state.max_chunk_end.max(mhfd_hdr_size);
    if effective_end < data.len() {
        let garbage_bytes = data.len() - effective_end;
        // Allow up to 3 bytes of slop (padding / alignment).
        if garbage_bytes > 3 {
            issues.push(format!(
                "Trailing garbage after last known chunk boundary: \
                 {garbage_bytes} bytes at offset {effective_end} (file length: {})",
                data.len()
            ));
        }
    }

    // Also report the number of MHSD sections and entries found, unless
    // issues already exist.
    if issues.is_empty() {
        issues.push(format!(
            "PhotoDB appears valid: {mhfd_entry_count} section(s), {} entry(s)",
            state.entries.len()
        ));
    }

    issues
}

/// Build a synthetic PhotoDB/ArtworkDB binary from a list of entries.
///
/// The resulting binary has the layout:
///
/// ```text
/// [MHFD 12] [MHSD 16] [MHNI(0) .. MHNI(N-1)] [pixels(0) .. pixels(N-1)]
/// ```
///
/// # Errors
///
/// Returns [`DecodeError::InvalidFormat`] if:
/// - `entries` is empty
/// - Any entry's `format_id` is not found in the built-in profile DB
/// - Any entry's `data` length does not match the profile's
///   `frame_byte_length`
///
/// # Panics
///
/// Panics if arithmetic overflows (this should never happen with real-world
/// inputs since the max file size is well under 2³¹ bytes).
pub fn try_build_photodb(
    entries: &[BuildEntry],
    mhni_header_size: i32,
    mhni_padding_size: i32,
) -> Result<Vec<u8>, DecodeError> {
    // 1. Validate inputs.
    if entries.is_empty() {
        return Err(DecodeError::InvalidFormat(
            "Cannot build PhotoDB with zero entries".into(),
        ));
    }

    let db = ProfileDb::load_builtin().map_err(|e| DecodeError::Profile(format!("Failed to load profile DB: {e}")))?;

    let mhni_total_len = mhni_header_size + mhni_padding_size;
    let mhni_total_len_usize = mhni_total_len as usize;

    // Resolve profiles and validate data lengths.
    let mut profiles: Vec<&Profile> = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let profile = db
            .get(entry.format_id)
            .ok_or_else(|| DecodeError::InvalidFormat(format!("Entry {idx}: unknown format ID {}", entry.format_id)))?;
        let expected_len = profile.frame_byte_length as usize;
        if entry.data.len() != expected_len {
            return Err(DecodeError::InvalidFormat(format!(
                "Entry {idx}: format ID {} has data length {}, expected {} (frame_byte_length)",
                entry.format_id,
                entry.data.len(),
                expected_len,
            )));
        }
        profiles.push(profile);
    }

    // 2. Calculate layout.
    let n = entries.len();
    let mhsd_header_size = 16 + (n * mhni_total_len_usize) + entries.iter().map(|e| e.data.len()).sum::<usize>();
    let total_size = 12 + mhsd_header_size;

    let mut buf = vec![0u8; total_size];

    // 3. Write MHFD header (12 bytes).
    //    magic = "mhfd", header_size = 12, entry_count = 1
    buf[0..4].copy_from_slice(b"mhfd");
    write_u32_le(&mut buf, 4, 12);
    write_u32_le(&mut buf, 8, 1); // one MHSD section

    // 4. Write MHSD header (16 bytes).
    //    magic = "mhsd", header_size = 16 + N*mhniTotalLen + totalPixelData,
    //    index = 0, recordType = 4 (thumbnails), entryCount = N
    let mhsd_offset = 12;
    buf[mhsd_offset..mhsd_offset + 4].copy_from_slice(b"mhsd");
    write_u32_le(&mut buf, mhsd_offset + 4, mhsd_header_size as u32);
    write_u16_le(&mut buf, mhsd_offset + 8, 0); // index
    write_u16_le(&mut buf, mhsd_offset + 10, 4); // recordType = 4 (thumbnails)
    write_u32_le(&mut buf, mhsd_offset + 12, n as u32); // entryCount

    // 5. Write MHNI entries.
    let mhni_start = mhsd_offset + 16; // past MHSD header
    let pixel_data_start = mhni_start + n * mhni_total_len_usize;

    // Calculate running pixel data offset for each entry.
    let mut current_pixel_offset = pixel_data_start;

    for (i, (entry, profile)) in entries.iter().zip(profiles.iter()).enumerate() {
        let mhni_pos = mhni_start + i * mhni_total_len_usize;

        // magic = "mhni"
        buf[mhni_pos..mhni_pos + 4].copy_from_slice(b"mhni");
        // headerSize
        write_u32_le(&mut buf, mhni_pos + 4, mhni_header_size as u32);
        // totalLen
        write_u32_le(&mut buf, mhni_pos + 8, mhni_total_len as u32);
        // entryIndex = 1
        write_u32_le(&mut buf, mhni_pos + 12, 1);
        // formatId
        write_i32_le(&mut buf, mhni_pos + 16, entry.format_id);
        // ithmbOffset — points to this entry's pixel data
        write_i32_le(&mut buf, mhni_pos + 20, current_pixel_offset as i32);
        // imageSize
        let image_size = entry.data.len() as i32;
        write_i32_le(&mut buf, mhni_pos + 24, image_size);
        // padding u32 = 0
        write_u32_le(&mut buf, mhni_pos + 28, 0);
        // height (i16) — from profile
        let height = profile.height as u16;
        write_u16_le(&mut buf, mhni_pos + 32, height);
        // width (i16) — from profile
        let width = profile.width as u16;
        write_u16_le(&mut buf, mhni_pos + 34, width);
        // padding zeros (mhni_padding_size bytes)
        // buf is already zero-filled, so this is already covered.
        // But ensure the range is explicitly zeroed for clarity.
        let pad_start = mhni_pos + 36;
        let pad_end = mhni_pos + mhni_total_len_usize;
        for b in &mut buf[pad_start..pad_end] {
            *b = 0;
        }

        // Advance pixel offset for the next entry.
        current_pixel_offset += entry.data.len();
    }

    // 6. Write pixel data blocks.
    let mut pixel_write_pos = pixel_data_start;
    for entry in entries {
        let data_len = entry.data.len();
        buf[pixel_write_pos..pixel_write_pos + data_len].copy_from_slice(&entry.data);
        pixel_write_pos += data_len;
    }

    Ok(buf)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a simple pixel data buffer of the given length with
    /// known byte pattern.
    fn make_pixel_data(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i & 0xFF) as u8).collect()
    }

    /// Helper: build and roundtrip-check a minimal PhotoDB.
    fn build_minimal_photodb(entries: &[BuildEntry], mhni_header_size: i32, mhni_padding_size: i32) -> Vec<u8> {
        try_build_photodb(entries, mhni_header_size, mhni_padding_size).expect("build should succeed")
    }

    // -----------------------------------------------------------------------
    // Builder tests
    // -----------------------------------------------------------------------

    #[test]
    fn builder_empty_entries_fails() {
        let result = try_build_photodb(&[], 36, 40);
        assert!(result.is_err());
    }

    #[test]
    fn builder_unknown_format_id_fails() {
        let entry = BuildEntry {
            format_id: 9999,
            data: vec![0u8; 100],
        };
        let result = try_build_photodb(&[entry], 36, 40);
        assert!(result.is_err());
    }

    #[test]
    fn builder_data_length_mismatch_fails() {
        // Profile 1016 has frame_byte_length = 39200 (140x140 RGB565)
        let entry = BuildEntry {
            format_id: 1016,
            data: vec![0u8; 100], // wrong size
        };
        let result = try_build_photodb(&[entry], 36, 40);
        assert!(result.is_err());
    }

    #[test]
    fn builder_single_entry_succeeds() {
        // Profile 1016: 140x140 RGB565, frame_byte_length = 39200
        let entry = BuildEntry {
            format_id: 1016,
            data: make_pixel_data(39200),
        };
        let result = try_build_photodb(&[entry], 36, 40).unwrap();

        // Verify size
        let expected_size = 12 + 16 + 76 + 39200;
        assert_eq!(result.len(), expected_size);

        // Verify MHFD magic
        assert_eq!(&result[0..4], b"mhfd");
        assert_eq!(read_u32_le(&result, 4), 12); // header_size
        assert_eq!(read_u32_le(&result, 8), 1); // entry_count

        // Verify MHSD header
        let mhsd_hdr_size: u32 = read_u32_le(&result, 12 + 4);
        assert!(mhsd_hdr_size >= 16);
        assert_eq!(read_u16_le(&result, 12 + 8), 0); // index
        assert_eq!(read_u16_le(&result, 12 + 10), 4); // recordType

        // Verify MHNI entry
        assert_eq!(&result[28..32], b"mhni");
        assert_eq!(read_u32_le(&result, 28 + 4), 36); // headerSize
        assert_eq!(read_u32_le(&result, 28 + 8), 76); // totalLen
        assert_eq!(read_u32_le(&result, 28 + 12), 1); // entryIndex
        assert_eq!(read_i32_le(&result, 28 + 16), 1016); // formatId
        assert_eq!(read_i32_le(&result, 28 + 20), 28 + 76); // ithmbOffset
        assert_eq!(read_i32_le(&result, 28 + 24), 39200); // imageSize

        // Verify pixel data
        let pixel_start = 28 + 76;
        assert_eq!(&result[pixel_start..pixel_start + 39200], &make_pixel_data(39200));
    }

    #[test]
    fn builder_multiple_entries_succeeds() {
        let entries = vec![
            BuildEntry {
                format_id: 1016,
                data: make_pixel_data(39200),
            },
            BuildEntry {
                format_id: 3004,
                data: make_pixel_data(6160),
            },
        ];
        let result = try_build_photodb(&entries, 36, 40).unwrap();

        let expected_size = 12 + 16 + 2 * 76 + 39200 + 6160;
        assert_eq!(result.len(), expected_size);

        // First entry ithmb_offset
        let pixel_data_start = 12 + 16 + 2 * 76;
        assert_eq!(read_i32_le(&result, 28 + 20), pixel_data_start as i32);
        assert_eq!(read_i32_le(&result, 28 + 24), 39200);

        // Second entry ithmb_offset
        let second_mhni = 28 + 76;
        assert_eq!(
            read_i32_le(&result, second_mhni + 20) as usize,
            pixel_data_start + 39200
        );
        assert_eq!(read_i32_le(&result, second_mhni + 24), 6160);

        // Verify pixel data
        let second_pixel = pixel_data_start + 39200;
        assert_eq!(
            &result[pixel_data_start..pixel_data_start + 39200],
            &make_pixel_data(39200)
        );
        assert_eq!(&result[second_pixel..second_pixel + 6160], &make_pixel_data(6160));
    }

    #[test]
    fn builder_custom_mhni_sizes() {
        // Use non-default header/padding sizes.
        let entry = BuildEntry {
            format_id: 1016,
            data: make_pixel_data(39200),
        };
        let result = try_build_photodb(&[entry], 40, 50).unwrap();

        let _ = 40 + 50;
        assert_eq!(read_u32_le(&result, 28 + 4), 40); // headerSize
        assert_eq!(read_u32_le(&result, 28 + 8), 90); // totalLen
        // ithmbOffset should use the custom totalLen
        let expected_off = 12 + 16 + 90; // MHFD + MHSD + one MHNI
        assert_eq!(read_i32_le(&result, 28 + 20), expected_off);
    }

    // -----------------------------------------------------------------------
    // Integrity check tests
    // -----------------------------------------------------------------------

    #[test]
    fn integrity_empty_data() {
        let issues = integrity_check_photodb(b"");
        assert!(!issues.is_empty());
        assert!(issues[0].contains("too short"));
    }

    #[test]
    fn integrity_too_short() {
        let issues = integrity_check_photodb(b"mhf");
        assert!(!issues.is_empty());
    }

    #[test]
    fn integrity_bad_magic() {
        let issues = integrity_check_photodb(b"XXXX");
        assert!(!issues.is_empty());
        assert!(issues[0].contains("magic"));
    }

    #[test]
    fn integrity_valid_built_file() {
        let entry = BuildEntry {
            format_id: 1016,
            data: make_pixel_data(39200),
        };
        let data = build_minimal_photodb(&[entry], 36, 40);
        let issues = integrity_check_photodb(&data);
        // Should be clean or have the informational "appears valid" message
        let has_clean = issues.iter().any(|i| i.contains("appears valid"));
        assert!(has_clean, "Expected clean result, got: {issues:?}");
    }

    #[test]
    fn integrity_valid_two_entries() {
        let entries = vec![
            BuildEntry {
                format_id: 1016,
                data: make_pixel_data(39200),
            },
            BuildEntry {
                format_id: 3004,
                data: make_pixel_data(6160),
            },
        ];
        let data = build_minimal_photodb(&entries, 36, 40);
        let issues = integrity_check_photodb(&data);
        let has_clean = issues.iter().any(|i| i.contains("appears valid"));
        assert!(has_clean, "Expected clean result, got: {issues:?}");
    }

    #[test]
    fn integrity_unknown_format_id() {
        // Tamper a valid file's format ID to an unknown value.
        let entry = BuildEntry {
            format_id: 1016,
            data: make_pixel_data(39200),
        };
        let mut data = build_minimal_photodb(&[entry], 36, 40);
        // Overwrite format_id at offset 28+16=44 with an unknown value
        write_i32_le(&mut data, 44, 9999);
        let issues = integrity_check_photodb(&data);
        let has_unknown = issues.iter().any(|i| i.contains("unknown format ID"));
        assert!(has_unknown, "Expected unknown format ID issue, got: {issues:?}");
    }

    #[test]
    fn integrity_trailing_garbage() {
        let entry = BuildEntry {
            format_id: 1016,
            data: make_pixel_data(39200),
        };
        let mut data = build_minimal_photodb(&[entry], 36, 40);
        // Append garbage bytes
        data.extend_from_slice(b"GARBAGE_AFTER_END");
        let issues = integrity_check_photodb(&data);
        let has_garbage = issues.iter().any(|i| i.contains("garbage"));
        assert!(has_garbage, "Expected trailing garbage issue, got: {issues:?}");
    }

    #[test]
    fn integrity_overlapping_offsets() {
        // Build two entries, then tamper the second offset to overlap with
        // the first.
        let entries = vec![
            BuildEntry {
                format_id: 1016,
                data: make_pixel_data(39200),
            },
            BuildEntry {
                format_id: 3004,
                data: make_pixel_data(6160),
            },
        ];
        let mut data = build_minimal_photodb(&entries, 36, 40);
        // Second MHNI is at 28+76=104. Its ithmbOffset at +20=124.
        // Tamper it to point into the first entry's data.
        let first_entry_off = read_i32_le(&data, 28 + 20);
        write_i32_le(&mut data, 104 + 20, first_entry_off + 100);
        let issues = integrity_check_photodb(&data);
        let has_overlap = issues.iter().any(|i| i.contains("overlapping"));
        assert!(has_overlap, "Expected overlap issue, got: {issues:?}");
    }

    // -----------------------------------------------------------------------
    // Roundtrip: build → integrity-check
    // -----------------------------------------------------------------------

    #[test]
    fn build_then_integrity_check_roundtrip() {
        let entries = vec![
            BuildEntry {
                format_id: 1007,
                data: make_pixel_data(829_440),
            },
            BuildEntry {
                format_id: 3004,
                data: make_pixel_data(6160),
            },
        ];
        let data = build_minimal_photodb(&entries, 36, 40);
        let issues = integrity_check_photodb(&data);
        let has_clean = issues.iter().any(|i| i.contains("appears valid"));
        assert!(
            has_clean,
            "Roundtrip should produce a valid PhotoDB. Issues: {issues:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Helper test: read helpers for the builder output
    // -----------------------------------------------------------------------

    fn read_u16_le(data: &[u8], offset: usize) -> u16 {
        u16::from(data[offset]) | (u16::from(data[offset + 1]) << 8)
    }

    fn read_i32_le(data: &[u8], offset: usize) -> i32 {
        let v = u32::from(data[offset])
            | (u32::from(data[offset + 1]) << 8)
            | (u32::from(data[offset + 2]) << 16)
            | (u32::from(data[offset + 3]) << 24);
        v as i32
    }
}
