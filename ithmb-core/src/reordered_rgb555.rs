//! `ReorderedRGB555` decoder — 15-bit RGB with big-endian byte order.
//!
//! This variant of RGB555 stores each 16-bit pixel in big-endian byte order
//! (high byte first, low byte second), unlike standard RGB555 which defaults to
//! little-endian. The bit layout within each 16-bit value is identical to standard
//! RGB555.
//!
//! Each pixel is 2 bytes (16 bits, MSB unused), laid out as:
//!
//! ```text
//! Default (swap_rgb_channels = false):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
//!
//! Stored in big-endian byte order:
//!   Byte 0: [|x|R4|R3|R2|R1|R0|G4|G3|]  (high byte)
//!   Byte 1: [|G2|G1|G0|B4|B3|B2|B1|B0|]  (low byte)
//!
//! BGR15  (swap_rgb_channels = true, used by iPhone 2G):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  B4 B3 B2 B1 B0 G4 G3 G2 G1 G0 R4 R3 R2 R1 R0
//! ```
//!
//! Output is BGRA 8-bit per channel.

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;

/// Decodes a `ReorderedRGB555` frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data (2 bytes per pixel, big-endian byte order).
/// * `profile` — The profile describing this frame's dimensions and flags.
///
/// # Errors
///
/// Returns [`DecodeError::InvalidFormat`] if width or height is zero or negative.
/// Returns [`DecodeError::BufferTooShort`] if `src` is smaller than `w * h * 2`.
pub fn decode(src: &[u8], profile: &Profile) -> Result<DecodedImage, DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;
    let swap = profile.swap_rgb_channels;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat("width and height must be positive".into()));
    }

    #[allow(clippy::cast_sign_loss)]
    let w = w_i32 as usize;
    #[allow(clippy::cast_sign_loss)]
    let h = h_i32 as usize;
    let total_pixels = w * h;
    let expected_bytes = total_pixels * 2;

    if src.len() < expected_bytes {
        return Err(DecodeError::BufferTooShort {
            expected: expected_bytes,
            actual: src.len(),
        });
    }

    let mut dst = vec![0u8; total_pixels * 4];
    let row_stride = src.len() / h;

    for y in 0..h {
        let row_start = y * row_stride;
        let dst_start = y * w * 4;

        for x in 0..w {
            let src_idx = row_start + x * 2;
            // ReorderedRGB555 always uses big-endian byte order:
            //   Byte 0 = high byte, Byte 1 = low byte.
            let raw = u16::from_be_bytes([src[src_idx], src[src_idx + 1]]);

            // Default layout: xRRRRRGGGGGBBBBB (R in high 5, B in low 5).
            // BGR15  (swap):  xBBBBBGGGGGRRRRR (B in high 5, R in low 5).
            let (r5, g5, b5) = if swap {
                (
                    u32::from(raw & 0x1F),
                    u32::from((raw >> 5) & 0x1F),
                    u32::from((raw >> 10) & 0x1F),
                )
            } else {
                (
                    u32::from((raw >> 10) & 0x1F),
                    u32::from((raw >> 5) & 0x1F),
                    u32::from(raw & 0x1F),
                )
            };

            let dst_idx = dst_start + x * 4;
            dst[dst_idx] = msb_replicate_5(b5);
            dst[dst_idx + 1] = msb_replicate_5(g5);
            dst[dst_idx + 2] = msb_replicate_5(r5);
            dst[dst_idx + 3] = 255;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    Ok(DecodedImage {
        data: dst,
        width: w as u32,
        height: h as u32,
    })
}

/// Replicates a 5-bit value to 8 bits: `(v << 3) | (v >> 2)`.
#[inline]
#[allow(clippy::cast_possible_truncation)]
fn msb_replicate_5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Encoding, Profile};

    fn make_profile(w: i32, h: i32, swap: bool) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::ReorderedRgb555,
            frame_byte_length: w * h * 2,
            little_endian: false,
            swap_rgb_channels: swap,
            ..Default::default()
        }
    }

    #[test]
    fn zero_dimensions_returns_err() {
        let profile = make_profile(0, 100, false);
        let result = decode(b"", &profile);
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn negative_dimension_returns_err() {
        let profile = make_profile(-1, 100, false);
        let result = decode(b"", &profile);
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn too_short_returns_buffer_too_short() {
        let profile = make_profile(100, 100, false);
        let result = decode(&[0u8; 10], &profile);
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
    }

    #[test]
    fn buffer_too_short_reports_exact_counts() {
        let profile = make_profile(10, 10, false);
        let result = decode(&[0u8; 50], &profile);
        match result {
            Err(DecodeError::BufferTooShort {
                expected: 200,
                actual: 50,
            }) => {} // ok
            other => panic!("expected BufferTooShort(200, 50), got {other:?}"),
        }
    }

    #[test]
    fn dst_allocation_matches_geometry() {
        let profile = make_profile(3, 2, false);
        let pixels = vec![0u8; 3 * 2 * 2];
        let img = decode(&pixels, &profile).unwrap();
        assert_eq!(img.data.len(), 3 * 2 * 4);
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
    }

    #[test]
    fn solid_white_pixel() {
        // RGB555 0x7FFF → R=31, G=31, B=31 → all 255 in BGRA
        // Big-endian: [0x7F, 0xFF]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x7F, 0xFF], &profile).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn solid_black_pixel() {
        // RGB555 0x0000 → all zeros
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x00, 0x00], &profile).unwrap();
        assert_eq!(img.data, vec![0, 0, 0, 255]);
    }

    #[test]
    fn solid_red_pixel() {
        // Layout xRRRRRGGGGGBBBBB, R=31 → bits 14..10 = 11111
        // Pixel = 0x7C00, big-endian: [0x7C, 0x00]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x7C, 0x00], &profile).unwrap();
        // BGRA: B=0, G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn solid_blue_pixel() {
        // Layout xRRRRRGGGGGBBBBB, B=31 → bits 4..0 = 11111
        // Pixel = 0x001F, big-endian: [0x00, 0x1F]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x00, 0x1F], &profile).unwrap();
        // BGRA: B=255, G=0, R=0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn solid_green_pixel() {
        // Layout xRRRRRGGGGGBBBBB, G=31 → bits 9..5 = 11111
        // Pixel = 0x03E0, big-endian: [0x03, 0xE0]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x03, 0xE0], &profile).unwrap();
        // BGRA: B=0, G=255, R=0
        assert_eq!(img.data, vec![0, 0xFF, 0, 255]);
    }

    #[test]
    fn decode_swap_rgb_channels() {
        // swap_rgb_channels=true: layout becomes xBBBBBGGGGGRRRRR
        // Blue=31 → bits 14..10 = 11111, pixel = 0x7C00
        // Big-endian: [0x7C, 0x00]
        // With swap, high bits = B, so B_out = 255
        let profile = make_profile(1, 1, true);
        let img = decode(&[0x7C, 0x00], &profile).unwrap();
        // Swap mode: B from high bits = 31 → 255, R from low bits = 0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn swap_mode_red_stays_low() {
        // swap_rgb_channels=true: xBBBBBGGGGGRRRRR
        // Red=31 → bits 4..0 = 11111, pixel = 0x001F
        // Big-endian: [0x00, 0x1F]
        let profile = make_profile(1, 1, true);
        let img = decode(&[0x00, 0x1F], &profile).unwrap();
        // Swap mode: R from low bits = 31 → R_out=255
        // BGRA: B=0 (from high bits), G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn two_pixel_decode() {
        // 2 white pixels in big-endian
        let profile = make_profile(2, 1, false);
        let img = decode(&[0x7F, 0xFF, 0x7F, 0xFF], &profile).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn two_pixel_different_colors() {
        // Pixel 0: red (R=31), Pixel 1: blue (B=31)
        let profile = make_profile(2, 1, false);
        let data = [
            0x7C, 0x00, // red: 0x7C00 BE
            0x00, 0x1F, // blue: 0x001F BE
        ];
        let img = decode(&data, &profile).unwrap();
        assert_eq!(
            img.data,
            vec![
                0, 0, 0xFF, 255, // BGRA: B=0, G=0, R=255
                0xFF, 0, 0, 255, // BGRA: B=255, G=0, R=0
            ]
        );
    }

    #[test]
    fn msb_replicate_clamping() {
        assert_eq!(msb_replicate_5(31), 0xFF);
        assert_eq!(msb_replicate_5(0), 0x00);
        assert_eq!(msb_replicate_5(16), 0x84);
        assert_eq!(msb_replicate_5(8), 0x42);
        assert_eq!(msb_replicate_5(1), 0x08);
    }

    #[test]
    fn output_has_correct_alpha() {
        let profile = make_profile(2, 2, false);
        let pixels = vec![0u8; 8];
        let img = decode(&pixels, &profile).unwrap();
        for i in 0..4 {
            assert_eq!(img.data[i * 4 + 3], 255);
        }
    }
}
