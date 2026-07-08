//! RGB555 decoder — 15-bit RGB used by iPod 4G/5G and iPhone 2G.
//!
//! Each pixel is 2 bytes (16 bits, MSB unused), laid out as:
//!
//! ```text
//! Default (swap_rgb_channels = false):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
//!
//! BGR15  (swap_rgb_channels = true, used by iPhone 2G):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  B4 B3 B2 B1 B0 G4 G3 G2 G1 G0 R4 R3 R2 R1 R0
//! ```
//!
//! Default byte order is little-endian. Output is BGRA 8-bit per channel.
//!
//! ## SIMD
//!
//! SIMD implementations exist in [`crate::simd`] (SSE2/AVX2/NEON runtime dispatch).

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::sync::atomic::AtomicBool;

/// Decodes an RGB555 frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data (2 bytes per pixel).
/// * `profile` — The profile describing this frame's dimensions and flags.
///
/// # Errors
///
/// Returns [`DecodeError::InvalidFormat`] if width or height is zero or negative.
/// Returns [`DecodeError::BufferTooShort`] if `src` is smaller than `w * h * 2`.
pub fn decode(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    let (w, h) = crate::decoder_helpers::validate_dimensions(src, profile, "width and height must be positive", 2)?;
    let le = profile.little_endian;
    let swap = profile.swap_rgb_channels;
    let total_pixels = w * h;

    let mut dst = vec![0u8; total_pixels * 4];

    let row_stride = src.len() / h;

    for y in 0..h {
        crate::pixel_utils::check_canceled(canceled, "rgb555 decode canceled")?;
        let row_start = y * row_stride;
        let dst_start = y * w * 4;
        let row_dst = &mut dst[dst_start..dst_start + w * 4];

        // SIMD fast path (LE + x86 with simd feature)
        #[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
        if le {
            crate::simd::rgb555_apply_row_to_bgra(&src[row_start..row_start + w * 2], row_dst);
            if swap {
                for p in row_dst.chunks_exact_mut(4) {
                    p.swap(0, 2);
                }
            }
            continue;
        }

        // Scalar fallback (handles BE endianness and swap natively)
        let src_row = &src[row_start..row_start + w * 2];
        for (src_pixel, dst_pixel) in src_row.chunks_exact(2).zip(row_dst.chunks_exact_mut(4)) {
            let raw = if le {
                u16::from_le_bytes([src_pixel[0], src_pixel[1]])
            } else {
                u16::from_be_bytes([src_pixel[0], src_pixel[1]])
            };
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
            dst_pixel.copy_from_slice(&[
                crate::pixel_utils::msb_replicate_5(b5),
                crate::pixel_utils::msb_replicate_5(g5),
                crate::pixel_utils::msb_replicate_5(r5),
                255,
            ]);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    let out_w = w as u32;
    #[allow(clippy::cast_possible_truncation)]
    let out_h = h as u32;

    Ok(DecodedImage {
        data: dst,
        width: out_w,
        height: out_h,
    })
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Encoding, Profile};
    use std::sync::atomic::AtomicBool;

    fn make_profile(w: i32, h: i32, le: bool, swap: bool) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::Rgb555,
            frame_byte_length: w * h * 2,
            little_endian: le,
            swap_rgb_channels: swap,
            ..Default::default()
        }
    }

    #[test]
    fn zero_dimensions_returns_err() {
        let profile = make_profile(0, 100, true, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn negative_dimension_returns_err() {
        let profile = make_profile(-1, 100, true, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn too_short_returns_buffer_too_short() {
        let profile = make_profile(100, 100, true, false);
        let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
    }

    #[test]
    fn buffer_too_short_reports_exact_counts() {
        let profile = make_profile(10, 10, true, false);
        let result = decode(&[0u8; 50], &profile, &AtomicBool::new(false));
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
        // 3×2 image = 6 pixels = 24 bytes output
        let profile = make_profile(3, 2, true, false);
        let pixels = vec![0u8; 3 * 2 * 2];
        let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data.len(), 3 * 2 * 4);
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
    }

    #[test]
    fn solid_white_pixel() {
        // RGB555 0x7FFF → R=31, G=31, B=31 → all 255 in BGRA
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0xFF, 0x7F], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn solid_black_pixel() {
        // RGB555 0x0000 → all zeros
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x00, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0, 0, 0, 255]);
    }

    #[test]
    fn solid_red_pixel() {
        // Layout xRRRRRGGGGGBBBBB, R=31 → bits 14..10 = 11111
        // Remaining bits 9..0 = 00000_00000
        // Pixel = 0b_0_11111_00000_00000 = 0x7C00
        // In LE: [0x00, 0x7C]
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=0, G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn solid_blue_pixel() {
        // Layout xRRRRRGGGGGBBBBB, B=31 → bits 4..0 = 11111
        // Pixel = 0b_0_00000_00000_11111 = 0x001F
        // In LE: [0x1F, 0x00]
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=255, G=0, R=0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn solid_green_pixel() {
        // Layout xRRRRRGGGGGBBBBB, G=31 → bits 9..5 = 11111
        // Pixel = 0b_0_00000_11111_00000 = 0x03E0
        // In LE: [0xE0, 0x03]
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0xE0, 0x03], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=0, G=255, R=0
        assert_eq!(img.data, vec![0, 0xFF, 0, 255]);
    }

    #[test]
    fn decode_big_endian() {
        // Same bits as solid_red but bytes swapped
        // Layout xRRRRRGGGGGBBBBB, R=31 → pixel = 0x7C00
        // Big-endian bytes: [0x7C, 0x00]
        let profile = make_profile(1, 1, false, false);
        let img = decode(&[0x7C, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn decode_swap_rgb_channels() {
        // swap_rgb_channels=true: layout becomes xBBBBBGGGGGRRRRR
        // Blue=31 → bits 14..10 = 11111, pixel = 0x7C00
        // In LE: [0x00, 0x7C]
        // With swap, high bits = blue, so B_out = 255
        let profile = make_profile(1, 1, true, true);
        let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
        // Swap mode: B from high bits = 31 → 255, R from low bits = 0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn swap_mode_red_stays_low() {
        // swap_rgb_channels=true: xBBBBBGGGGGRRRRR
        // Red=31 → bits 4..0 = 11111, pixel = 0x001F
        // In LE: [0x1F, 0x00]
        let profile = make_profile(1, 1, true, true);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // Swap mode: R from low bits = 31 → R_out=255
        // BGRA: B=0 (from high bits), G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn multi_pixel_decode() {
        // 2×1 image, 2 white pixels
        let profile = make_profile(2, 1, true, false);
        let img = decode(&[0xFF, 0x7F, 0xFF, 0x7F], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn msb_replicate_clamping() {
        // msb_replicate_5(31) → 0xFF  (max 5-bit → max 8-bit)
        assert_eq!(crate::pixel_utils::msb_replicate_5(31), 0xFF);
        // msb_replicate_5(0) → 0x00
        assert_eq!(crate::pixel_utils::msb_replicate_5(0), 0x00);
        // msb_replicate_5(16) → 0x84  (mid-level check)
        assert_eq!(crate::pixel_utils::msb_replicate_5(16), 0x84);
        // msb_replicate_5(8) → 0x42
        assert_eq!(crate::pixel_utils::msb_replicate_5(8), 0x42);
        // msb_replicate_5(1) → 0x08
        assert_eq!(crate::pixel_utils::msb_replicate_5(1), 0x08);
    }

    #[test]
    fn two_pixel_different_colors() {
        // Pixel 0: red (R=31), Pixel 1: blue (B=31)
        let profile = make_profile(2, 1, true, false);
        let data = [
            0x00, 0x7C, // red: 0x7C00 LE
            0x1F, 0x00, // blue: 0x001F LE
        ];
        let img = decode(&data, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            img.data,
            vec![
                0, 0, 0xFF, 255, // BGRA: B=0, G=0, R=255
                0xFF, 0, 0, 255, // BGRA: B=255, G=0, R=0
            ]
        );
    }

    #[test]
    fn matches_golden_gradient_first_pixel() {
        // Golden test: gradient_4x4, first pixel 0x0010
        // Layout xRRRRRGGGGGBBBBB: R=0, G=0, B=16
        // BGRA: B=msb5(16)=0x84, G=0, R=0
        let profile = make_profile(4, 4, true, false);
        let img = decode(
            &[
                0x10, 0x00, 0x12, 0x28, 0x15, 0x54, 0x17, 0x7C, //
                0x52, 0x01, 0x55, 0x29, 0x57, 0x55, 0x5A, 0x7D, //
                0xB5, 0x02, 0xB7, 0x2A, 0xBA, 0x56, 0xBD, 0x7E, //
                0xF7, 0x03, 0xFA, 0x2B, 0xFD, 0x57, 0xFF, 0x7F, //
            ],
            &profile,
            &AtomicBool::new(false),
        )
        .unwrap();
        // Assert first pixel BGRA
        assert_eq!(img.data[0], 0x84); // B
        assert_eq!(img.data[1], 0x00); // G
        assert_eq!(img.data[2], 0x00); // R
        assert_eq!(img.data[3], 0xFF); // A
        // Assert last pixel (all max)
        let last = img.data.len() - 4;
        assert_eq!(img.data[last], 0xFF);
        assert_eq!(img.data[last + 1], 0xFF);
        assert_eq!(img.data[last + 2], 0xFF);
        assert_eq!(img.data[last + 3], 0xFF);
    }

    #[allow(clippy::cast_sign_loss)]
    #[test]
    fn row_stride_is_data_driven() {
        // 55×55 padded format: rowStride = src.len / h
        let w = 55i32;
        let h = 55i32;
        let padded_data = vec![0u8; 6400]; // larger than 55*55*2 = 6050
        let profile = Profile {
            width: w,
            height: h,
            encoding: Encoding::Rgb555,
            frame_byte_length: 6400,
            little_endian: true,
            ..Default::default()
        };
        let img = decode(&padded_data, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data.len(), (w * h * 4) as usize);
    }

    #[test]
    fn output_has_correct_alpha() {
        let profile = make_profile(2, 2, true, false);
        let pixels = vec![0u8; 8];
        let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
        for i in 0..4 {
            assert_eq!(img.data[i * 4 + 3], 255);
        }
    }
}
