//! Tree-walking helpers — recursive chunk traversal for PhotoDB/ArtworkDB.

use crate::error::DecodeError;
use crate::photodb::types::*;
use std::cmp::min;

use super::{PhotoDbEntry, PhotoDbEntryKind, PhotoDbMetadata};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum recursion depth when walking the chunk tree.
const MAX_DEPTH: u32 = 64;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check whether the data at `start` (within the range [`start`, `end`))
/// appears to be a valid child chunk.
///
/// Verifies that:
/// 1. There is room for at least an 8-byte header (magic + header_size).
/// 2. The `header_size` field at `start + 4` is >= 8.
/// 3. The magic at `start` is a recognised chunk type.
#[must_use]
pub(super) fn has_child_chunks(data: &[u8], start: usize, end: usize, little_endian: bool) -> bool {
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
pub(super) fn walk_entries(
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
        let chunk_end = min(pos.saturating_add(hdr_size_usize), data.len());
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
                let child_end = min(pos.saturating_add(total_len), data.len());
                if child_end <= pos {
                    // Zero/negative-size container: no children and no
                    // advancement possible — bail out of the walk (C1).
                    break;
                }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
