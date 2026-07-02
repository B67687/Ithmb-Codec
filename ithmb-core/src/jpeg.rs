//! JPEG detection and passthrough decoder for `.ithmb` files.
//!
//! T-prefix `.ithmb` files contain an embedded JPEG stream rather than raw pixel
//! data. This module detects JPEG streams by their SOI marker, decodes them
//! via the `jpeg_decoder` crate, applies EXIF orientation if present, and
//! outputs BGRA8 pixel data.

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::io::Cursor;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if `src` starts with a JPEG SOI marker (`0xFF`, `0xD8`).
#[must_use]
pub fn is_jpeg(src: &[u8]) -> bool {
    src.first().is_some_and(|&b| b == 0xFF) && src.get(1).is_some_and(|&b| b == 0xD8)
}

/// Decodes a JPEG stream to BGRA8 output.
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] if the input is shorter than the
/// SOI marker. Returns [`DecodeError::InvalidFormat`] if the input is not a
/// valid JPEG stream. Returns [`DecodeError::Jpeg`] if the underlying JPEG
/// decoder fails.
pub fn decode(src: &[u8], _profile: &Profile) -> Result<DecodedImage, DecodeError> {
    if src.len() < 2 {
        return Err(DecodeError::BufferTooShort {
            expected: 2,
            actual: src.len(),
        });
    }
    if !is_jpeg(src) {
        return Err(DecodeError::InvalidFormat("not a JPEG stream".into()));
    }

    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(src));
    let pixels = decoder.decode().map_err(|e| DecodeError::Jpeg(e.to_string()))?;

    let info = decoder
        .info()
        .ok_or_else(|| DecodeError::Jpeg("no JPEG metadata".into()))?;

    let w = u32::from(info.width);
    let h = u32::from(info.height);

    // `pixels` is RGB (3 bytes per pixel) — convert to BGRA8.
    let pixel_count = (w * h) as usize;
    let mut data = vec![0u8; pixel_count * 4];

    for (i, chunk) in pixels.chunks_exact(3).enumerate() {
        if i >= pixel_count {
            break;
        }
        let dst = i * 4;
        data[dst] = chunk[2]; // B
        data[dst + 1] = chunk[1]; // G
        data[dst + 2] = chunk[0]; // R
        data[dst + 3] = 255; // A
    }

    // Check EXIF orientation and rotate if needed.
    let orientation = extract_exif_orientation(src);
    if orientation > 1 {
        let (rotated_data, rw, rh) = rotate_bgra(&data, w, h, orientation);
        return Ok(DecodedImage {
            data: rotated_data,
            width: rw,
            height: rh,
        });
    }

    Ok(DecodedImage {
        data,
        width: w,
        height: h,
    })
}

// ---------------------------------------------------------------------------
// EXIF orientation extraction (simplified APP1 parser)
// ---------------------------------------------------------------------------

/// Extracts the EXIF orientation tag (0x0112) from a JPEG stream.
///
/// Returns `1` (normal) if no EXIF data is found or if parsing fails.
fn extract_exif_orientation(src: &[u8]) -> u8 {
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
fn rotate_bgra(data: &[u8], w: u32, h: u32, orientation: u8) -> (Vec<u8>, u32, u32) {
    let total = (w * h * 4) as usize;
    // The output buffer is always the same size (w * h * 4 == h * w * 4).
    let mut rotated = vec![0u8; total];
    let wu = w as usize;
    let hu = h as usize;

    match orientation {
        3 => {
            // Rotate 180°: reverse pixel order.
            for i in 0..(wu * hu) {
                let src_idx = i * 4;
                let dst_idx = (wu * hu - 1 - i) * 4;
                rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
            }
            (rotated, w, h)
        }
        6 => {
            // Rotate 90° CW: old (ix, iy) → new (h-1-iy, ix).
            // Output dimensions: (h, w).
            for iy in 0..hu {
                for ix in 0..wu {
                    let src_idx = (iy * wu + ix) * 4;
                    let ox = hu - 1 - iy;
                    let oy = ix;
                    let dst_idx = (oy * hu + ox) * 4;
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
                }
            }
            (rotated, h, w)
        }
        8 => {
            // Rotate 270° CW (90° CCW): output(x, y) = input(y, w - 1 - x).
            // Output dimensions: (h, w).
            let mut rotated = vec![0u8; total];
            for iy in 0..hu {
                for ix in 0..wu {
                    let src_idx = (iy * wu + ix) * 4;
                    // 270° CW: old (ix, iy) → new (iy, w-1-ix)
                    let ox = iy;
                    #[allow(clippy::cast_possible_truncation)]
                    let oy = wu - 1 - ix;
                    let dst_idx = (oy * hu + ox) * 4;
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
                }
            }
            (rotated, h, w)
        }
        _ => (data.to_vec(), w, h),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid 2×2 black JPEG generated by ffmpeg.
    const TEST_JPEG: &[u8] = &[
        0xff, 0xd8, 0xff, 0xfe, 0x00, 0x0f, 0x4c, 0x61, 0x76, 0x63, 0x36, 0x31, 0x2e, 0x33, 0x2e, 0x31, 0x30, 0x30,
        0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
        0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x07, 0x07, 0x07, 0x08, 0x08,
        0x08, 0x07, 0x07, 0x07, 0x06, 0x06, 0x07, 0x07, 0x08, 0x08, 0x08, 0x08, 0x09, 0x09, 0x09, 0x08, 0x08, 0x08,
        0x08, 0x09, 0x09, 0x0a, 0x0a, 0x0a, 0x0c, 0x0c, 0x0b, 0x0b, 0x0e, 0x0e, 0x0e, 0x11, 0x11, 0x14, 0xff, 0xc4,
        0x00, 0x4b, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x02, 0x03, 0x01, 0x12, 0x00, 0x02, 0x12,
        0x00, 0x03, 0x12, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
        0x9f, 0xc0, 0x07, 0xff, 0xd9,
    ];

    /// A JPEG with EXIF orientation tag = 6 (rotate 90° CW).
    const TEST_JPEG_EXIF6: &[u8] = &[
        0xff, 0xd8, 0xff, 0xe1, 0x00, 0x20, 0x45, 0x78, 0x69, 0x66, 0x00, 0x00, 0x49, 0x49, 0x2a, 0x00, 0x08, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xfe, 0x00, 0x0f, 0x4c, 0x61, 0x76, 0x63, 0x36, 0x31, 0x2e, 0x33, 0x2e, 0x31, 0x30, 0x30, 0x00, 0xff,
        0xdb, 0x00, 0x43, 0x00, 0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x06, 0x06,
        0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x07, 0x07, 0x07, 0x08, 0x08, 0x08, 0x07,
        0x07, 0x07, 0x06, 0x06, 0x07, 0x07, 0x08, 0x08, 0x08, 0x08, 0x09, 0x09, 0x09, 0x08, 0x08, 0x08, 0x08, 0x09,
        0x09, 0x0a, 0x0a, 0x0a, 0x0c, 0x0c, 0x0b, 0x0b, 0x0e, 0x0e, 0x0e, 0x11, 0x11, 0x14, 0xff, 0xc4, 0x00, 0x4b,
        0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x02, 0x03, 0x01, 0x12, 0x00, 0x02, 0x12, 0x00, 0x03,
        0x12, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0x9f, 0xc0,
        0x07, 0xff, 0xd9,
    ];

    #[test]
    fn is_jpeg_detects_soi() {
        assert!(is_jpeg(&[0xFF, 0xD8]));
        assert!(is_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_jpeg(TEST_JPEG));
    }

    #[test]
    fn is_jpeg_rejects_non_jpeg() {
        assert!(!is_jpeg(&[0x00, 0x00]));
        assert!(!is_jpeg(&[0xFF, 0x00]));
        assert!(!is_jpeg(&[0x00, 0xD8]));
    }

    #[test]
    fn is_jpeg_rejects_short_input() {
        assert!(!is_jpeg(&[]));
        assert!(!is_jpeg(&[0xFF]));
    }

    #[test]
    fn decode_short_input_returns_buffer_too_short() {
        let profile = Profile::default();
        let result = decode(&[], &profile);
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 2, actual: 0 })
        ));

        let result = decode(&[0xFF], &profile);
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 2, actual: 1 })
        ));
    }

    #[test]
    fn decode_non_jpeg_returns_invalid_format() {
        let profile = Profile::default();
        let result = decode(&[0x00, 0x00, 0x00, 0x00], &profile);
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn decode_invalid_jpeg_returns_jpeg_error() {
        // SOI marker present but no valid JPEG structure after it.
        let profile = Profile::default();
        let result = decode(&[0xFF, 0xD8, 0xFF, 0xD9], &profile);
        assert!(matches!(result, Err(DecodeError::Jpeg(..))));
    }

    #[test]
    fn decode_valid_jpeg() {
        let profile = Profile::default();
        let result = decode(TEST_JPEG, &profile);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let img = result.unwrap();
        // 2×2 image, 16 bytes of BGRA data.
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 2 * 2 * 4);
    }

    #[test]
    fn decode_bgra_output_format() {
        let profile = Profile::default();
        let img = decode(TEST_JPEG, &profile).unwrap();
        // Every pixel should be 4 bytes with alpha = 255.
        for chunk in img.data.chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }
        // Data length should be width * height * 4.
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    #[test]
    fn extract_exif_orientation_normal_when_no_exif() {
        // JPEG without EXIF data should return orientation 1.
        assert_eq!(extract_exif_orientation(TEST_JPEG), 1);
    }

    #[test]
    fn extract_exif_orientation_found() {
        // TEST_JPEG_EXIF6 has orientation = 6.
        assert_eq!(extract_exif_orientation(TEST_JPEG_EXIF6), 6);
    }

    #[test]
    fn extract_exif_orientation_short_input() {
        assert_eq!(extract_exif_orientation(&[]), 1);
        assert_eq!(extract_exif_orientation(&[0xFF, 0xD8]), 1);
    }

    #[test]
    fn exif_rotation_180() {
        // Create 2×2 pixel data: top-left=red, top-right=green,
        // bottom-left=blue, bottom-right=white.
        let w = 2u32;
        let h = 2u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // Pixel layout: [R, G, B, A] in BGRA format.
        // Top-left (0,0): red → B=0, G=0, R=255, A=255
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // Top-right (1,0): green → B=0, G=255, R=0, A=255
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // Bottom-left (0,1): blue → B=255, G=0, R=0, A=255
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // Bottom-right (1,1): white → B=255, G=255, R=255, A=255
        data[12..16].copy_from_slice(&[255, 255, 255, 255]);

        // After 180° rotation, pixel mapping:
        // (0,0) → (1,1), (1,0) → (0,1), (0,1) → (1,0), (1,1) → (0,0)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 3);
        assert_eq!(rw, 2);
        assert_eq!(rh, 2);
        // (1,1) should be red
        assert_eq!(&rotated[12..16], &[0, 0, 255, 255]);
        // (0,1) should be green
        assert_eq!(&rotated[8..12], &[0, 255, 0, 255]);
        // (1,0) should be blue
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]);
        // (0,0) should be white
        assert_eq!(&rotated[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn exif_rotation_90cw() {
        // 2×3 pixel data for 90° CW rotation test.
        // Using a 2-wide, 3-tall image to make dimension swap obvious.
        let w = 2u32;
        let h = 3u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // Fill with distinct colors: row-major order.
        // (0,0): red
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // (1,0): green
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // (0,1): blue
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // (1,1): yellow (R+G)
        data[12..16].copy_from_slice(&[0, 255, 255, 255]);
        // (0,2): cyan (G+B)
        data[16..20].copy_from_slice(&[255, 255, 0, 255]);
        // (1,2): magenta (R+B)
        data[20..24].copy_from_slice(&[255, 0, 255, 255]);

        // 90° CW: old (x, y) → new (h-1-y, x)
        // (0,0)→(2,0), (1,0)→(2,1), (0,1)→(1,0), (1,1)→(1,1), (0,2)→(0,0), (1,2)→(0,1)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 6);
        // Dimensions swap: new width = h = 3, new height = w = 2
        assert_eq!(rw, 3);
        assert_eq!(rh, 2);

        // Output row-major layout (width=3, height=2):
        // (0,0) = old(0,2) = cyan
        assert_eq!(&rotated[0..4], &[255, 255, 0, 255]);
        // (1,0) = old(0,1) = blue
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]);
        // (2,0) = old(0,0) = red
        assert_eq!(&rotated[8..12], &[0, 0, 255, 255]);
        // (0,1) = old(1,2) = magenta
        assert_eq!(&rotated[12..16], &[255, 0, 255, 255]);
        // (1,1) = old(1,1) = yellow
        assert_eq!(&rotated[16..20], &[0, 255, 255, 255]);
        // (2,1) = old(1,0) = green
        assert_eq!(&rotated[20..24], &[0, 255, 0, 255]);
    }

    #[test]
    fn exif_rotation_270cw() {
        let w = 2u32;
        let h = 3u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // (0,0): red
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // (1,0): green
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // (0,1): blue
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // (1,1): yellow
        data[12..16].copy_from_slice(&[0, 255, 255, 255]);
        // (0,2): cyan
        data[16..20].copy_from_slice(&[255, 255, 0, 255]);
        // (1,2): magenta
        data[20..24].copy_from_slice(&[255, 0, 255, 255]);

        // 270° CW: old (x, y) → new (y, w-1-x)
        // (0,0)→(0,1), (1,0)→(0,0), (0,1)→(1,1), (1,1)→(1,0), (0,2)→(2,1), (1,2)→(2,0)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 8);
        assert_eq!(rw, 3);
        assert_eq!(rh, 2);

        // Output row-major (width=3, height=2):
        // (0,0) = old(1,0) = green
        assert_eq!(&rotated[0..4], &[0, 255, 0, 255]);
        // (1,0) = old(1,1) = yellow
        assert_eq!(&rotated[4..8], &[0, 255, 255, 255]);
        // (2,0) = old(1,2) = magenta
        assert_eq!(&rotated[8..12], &[255, 0, 255, 255]);
        // (0,1) = old(0,0) = red
        assert_eq!(&rotated[12..16], &[0, 0, 255, 255]);
        // (1,1) = old(0,1) = blue
        assert_eq!(&rotated[16..20], &[255, 0, 0, 255]);
        // (2,1) = old(0,2) = cyan
        assert_eq!(&rotated[20..24], &[255, 255, 0, 255]);
    }

    #[test]
    fn decode_with_exif_rotation() {
        let profile = Profile::default();
        let result = decode(TEST_JPEG_EXIF6, &profile);
        // The JPEG has EXIF orientation = 6, so dimensions should be swapped
        // (2×2 → still 2×2 since it's square, but rotation logic applies).
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
    }

    #[test]
    fn rotate_bgra_noop_for_normal_orientation() {
        let data = vec![0u8; 16];
        let (rotated, rw, rh) = rotate_bgra(&data, 2, 2, 1);
        assert_eq!(rotated, data);
        assert_eq!(rw, 2);
        assert_eq!(rh, 2);
    }
}
