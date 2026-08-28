//! EXIF orientation extraction and BGRA rotation helpers.

/// Extracts the EXIF orientation tag (0x0112) from a JPEG stream.
///
/// Returns `1` (normal) if no EXIF data is found or if parsing fails.
pub(crate) fn extract_exif_orientation(src: &[u8]) -> u8 {
    // Minimum valid structure: SOI(2) + APP1(2) + len(2) + "Exif\0\0"(6) + TIFF(8) = 20
    if src.len() < 20 {
        return 1;
    }

    // Check SOI marker.
    if src[0] != 0xFF || src[1] != 0xD8 {
        return 1;
    }

    // Scan for APP1 marker (FF E1). In JPEG, after SOI there may be other
    // markers before APP1, so we walk forward through segment headers.
    let mut pos = 2usize;
    loop {
        if pos + 4 > src.len() {
            return 1;
        }
        if src[pos] != 0xFF {
            return 1;
        }
        let marker = src[pos + 1];
        if marker == 0xE1 {
            // APP1 found — check for Exif identifier.
            let seg_len = usize::from(u16::from_be_bytes([src[pos + 2], src[pos + 3]]));
            if pos + 2 + seg_len > src.len() {
                return 1;
            }
            let exif_start = pos + 4; // skip marker(2) + length(2)
            if seg_len < 6 + 8 {
                return 1;
            }
            if &src[exif_start..exif_start + 6] != b"Exif\x00\x00" {
                return 1;
            }
            return parse_tiff_orientation(&src[exif_start + 6..], seg_len - 6);
        }
        if marker == 0xD9 {
            // EOI — no APP1 found.
            return 1;
        }
        // Skip over any other marker segment. Marker types FFE0–FFEF and
        // FFC0–FFDF are segment markers with a length field; standalone
        // markers (FFD0–FFD7, FFD8, FFD9, FF01) have no length.
        #[allow(clippy::match_same_arms)]
        match marker {
            // Standalone markers (no segment data).
            0x00 | 0xD0..=0xD7 | 0xD8 | 0xD9 | 0x01 => {
                pos += 2;
            }
            // Markers with segment data: all others.
            _ => {
                if pos + 4 > src.len() {
                    return 1;
                }
                let seg_len = usize::from(u16::from_be_bytes([src[pos + 2], src[pos + 3]]));
                pos += 2 + seg_len;
            }
        }
    }
}

/// Parses the TIFF header and walks IFD0 to find orientation tag 0x0112.
fn parse_tiff_orientation(tiff: &[u8], _remaining: usize) -> u8 {
    if tiff.len() < 8 {
        return 1;
    }

    let le = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return 1,
    };

    // Magic 0x002A.
    let magic = if le {
        u16::from_le_bytes([tiff[2], tiff[3]])
    } else {
        u16::from_be_bytes([tiff[2], tiff[3]])
    };
    if magic != 0x002A {
        return 1;
    }

    // Offset to IFD0 from start of TIFF header.
    let ifd0_offset = if le {
        u32::from_le_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
    } else {
        u32::from_be_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
    } as usize;

    if ifd0_offset + 2 > tiff.len() {
        return 1;
    }

    let entry_count = if le {
        u16::from_le_bytes([tiff[ifd0_offset], tiff[ifd0_offset + 1]])
    } else {
        u16::from_be_bytes([tiff[ifd0_offset], tiff[ifd0_offset + 1]])
    } as usize;

    // Each IFD entry is 12 bytes: tag(2), type(2), count(4), value/offset(4).
    for i in 0..entry_count {
        let entry_start = ifd0_offset + 2 + i * 12;
        if entry_start + 12 > tiff.len() {
            break;
        }
        let tag = if le {
            u16::from_le_bytes([tiff[entry_start], tiff[entry_start + 1]])
        } else {
            u16::from_be_bytes([tiff[entry_start], tiff[entry_start + 1]])
        };
        if tag == 0x0112 {
            // Orientation value is in bytes 8..10 of the entry (value/offset field).
            let val = if le {
                u16::from_le_bytes([tiff[entry_start + 8], tiff[entry_start + 9]])
            } else {
                u16::from_be_bytes([tiff[entry_start + 8], tiff[entry_start + 9]])
            };
            return val.min(8) as u8;
        }
    }

    1
}

// ---------------------------------------------------------------------------
// BGRA rotation helpers
// ---------------------------------------------------------------------------

/// Applies EXIF orientation rotation to a BGRA8 pixel buffer.
///
/// Returns `(rotated_data, new_width, new_height)`.
///
/// Supports orientations:
/// - 3: 180° rotation
/// - 6: 90° clockwise rotation (dimensions swap)
/// - 8: 270° clockwise / 90° counter-clockwise rotation (dimensions swap)
///
/// Delegates to the shared [`crate::pixel_utils::rotate_pixels`] helper;
/// the mapping is identical to the historic inline implementation.
pub(crate) fn rotate_bgra(data: &[u8], w: u32, h: u32, orientation: u8) -> (Vec<u8>, u32, u32) {
    let rotation = match orientation {
        3 => 180,
        6 => 90,
        8 => 270,
        _ => 0,
    };
    crate::pixel_utils::rotate_pixels(data, w, h, rotation)
}
