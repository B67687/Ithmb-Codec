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
    let (data, w, h) =
        crate::decoder_helpers::validate_dimensions(src, profile, "width and height must be positive", 2)?;
    let src = &*data;
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

        // SIMD fast path (LE + x86 with SSE2/AVX2 runtime dispatch)
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
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
    fn decode_big_endian() {
        // Same bits as solid_red but bytes swapped
        // Layout xRRRRRGGGGGBBBBB, R=31 → pixel = 0x7C00
        // Big-endian bytes: [0x7C, 0x00]
        let profile = make_profile(1, 1, false, false);
        let img = decode(&[0x7C, 0x00], &profile, &AtomicBool::new(false)).unwrap();
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
    fn shared_decode_suite() {
        // Historic per-decoder unit tests (14 fns) collapsed into one shared
        // suite — byte-for-byte identical assertions, see src/test_support.rs.
        crate::test_support::run_rgb555_family_suite(decode, |w, h, swap| make_profile(w, h, true, swap));
    }
}
