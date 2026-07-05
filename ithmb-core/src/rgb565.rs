//! RGB565 decoder — the most common .ithmb raw pixel format.
//!
//! Each pixel is 2 bytes (16 bits) laid out as:
//!
//! ```text
//! Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!        R4 R3 R2 R1 R0 G5 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
//! ```
//!
//! Default byte order is little-endian. Big-endian variants exist (e.g. iPod
//! Photo 4G format 1013). The `swap_rgb_channels` flag handles the BGR15
//! channel-reversed layout used by iPhone 2G (bit layout: `xBBBBBGGGGGRRRRR`).
//!
//! Output is BGRA 8-bit per channel (the native pixel format for the `ImageGlass`
//! ABI and common system framebuffers).
//!
//! ## SIMD
//!
//! This module starts with a correct scalar implementation. SIMD (SSE2/NEON/AVX-512)
//! will be added in subsequent iterations, ported from the verified C# intrinsics.

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::sync::atomic::AtomicBool;
/// Decodes an RGB565 frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data (2 bytes per pixel).
/// * `profile` — The profile describing this frame's dimensions and flags.
///
/// # Returns
///
/// `Ok(DecodedImage)` on success, or `Err(DecodeError)` with a structured reason.
///
/// # Errors
///
/// Returns [`DecodeError::InvalidFormat`] if the profile dimensions are zero or
/// negative. Returns [`DecodeError::BufferTooShort`] if `src` is shorter than
/// the expected pixel data (`width × height × 2` bytes).
pub fn decode(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    let (w, h) = crate::decoder_helpers::validate_dimensions(src, profile, "width and height must be positive", 2)?;
    let le = profile.little_endian;
    let swap = profile.swap_rgb_channels;

    let mut dst = vec![0u8; w * h * 4];
    let row_stride = src.len() / h;

    for y in 0..h {
        crate::pixel_utils::check_canceled(canceled, "rgb565 decode canceled")?;
        let row_start = y * row_stride;
        let dst_start = y * w * 4;
        let row_dst = &mut dst[dst_start..dst_start + w * 4];

        // SIMD fast path (LE + x86 with simd feature)
        #[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
        if le {
            crate::simd::rgb565_apply_row_to_bgra(&src[row_start..row_start + w * 2], row_dst);
            if swap {
                for p in row_dst.chunks_exact_mut(4) {
                    p.swap(0, 2);
                }
            }
            continue;
        }

        // Scalar fallback (handles BE endianness and swap natively)
        for x in 0..w {
            let pixel = if le {
                u16::from_le_bytes([src[row_start + x * 2], src[row_start + x * 2 + 1]])
            } else {
                u16::from_be_bytes([src[row_start + x * 2], src[row_start + x * 2 + 1]])
            };
            let r5 = u32::from((pixel >> 11) & 0x1F);
            let g6 = u32::from((pixel >> 5) & 0x3F);
            let b5 = u32::from(pixel & 0x1F);
            let dst_idx = x * 4;
            if swap {
                row_dst[dst_idx] = crate::pixel_utils::msb_replicate_5(r5);
                row_dst[dst_idx + 1] = crate::pixel_utils::msb_replicate_6(g6);
                row_dst[dst_idx + 2] = crate::pixel_utils::msb_replicate_5(b5);
            } else {
                row_dst[dst_idx] = crate::pixel_utils::msb_replicate_5(b5);
                row_dst[dst_idx + 1] = crate::pixel_utils::msb_replicate_6(g6);
                row_dst[dst_idx + 2] = crate::pixel_utils::msb_replicate_5(r5);
            }
            row_dst[dst_idx + 3] = 255;
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    Ok(DecodedImage {
        data: dst,
        width: w as u32,
        height: h as u32,
    })
}

// ---- MSB replication helpers ----

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
            encoding: Encoding::Rgb565,
            frame_byte_length: w * h * 2,
            little_endian: le,
            swap_rgb_channels: swap,
            ..Default::default()
        }
    }

    #[test]
    fn zero_dimensions_returns_invalid_format() {
        let profile = make_profile(0, 100, true, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn negative_dimensions_returns_invalid_format() {
        let profile = make_profile(-1, 100, true, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn too_short_returns_buffer_too_short() {
        let profile = make_profile(100, 100, true, false);
        let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort {
                expected: 20000,
                actual: 10
            })
        ));
    }

    #[test]
    fn decode_black_pixel() {
        // RGB565 0x0000 → all zeros
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x00, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(&img.data, &[0, 0, 0, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_white_pixel() {
        // RGB565 0xFFFF → R=0x1F, G=0x3F, B=0x1F
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0xFF, 0xFF], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(&img.data, &[0xFF, 0xFF, 0xFF, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_red_pixel() {
        // RGB565 little-endian: R=31, G=0, B=0 → 0b11111000_00000000 = 0xF8 0x00
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x00, 0xF8], &profile, &AtomicBool::new(false)).unwrap();
        // B=0, G=0, R=255
        assert_eq!(&img.data, &[0, 0, 0xFF, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_blue_pixel() {
        // RGB565 little-endian: R=0, G=0, B=31 → 0b00000000_00011111 = 0x1F 0x00
        let profile = make_profile(1, 1, true, false);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // B=255, G=0, R=0
        assert_eq!(&img.data, &[0xFF, 0, 0, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_big_endian() {
        // Big-endian RGB565: same bits but bytes swapped
        let profile = make_profile(1, 1, false, false);
        // R=31, G=0, B=0 in big-endian: 0xF8 0x00 → 0b11111000_00000000
        // Stored as bytes: [0xF8, 0x00] (big-endian, so first byte is high)
        let img = decode(&[0xF8, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(&img.data, &[0, 0, 0xFF, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_swap_rgb_channels() {
        // With swap_rgb_channels=true, R and B are swapped.
        // RGB565 storing blue: R=0, G=0, B=31 → 0x001F
        // With swap, output should put 0 in B slot and 255 in R slot.
        let profile = make_profile(1, 1, true, true);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // swap_rgb: B slot gets the R value (0), R slot gets the B value (255)
        assert_eq!(&img.data, &[0, 0, 0xFF, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn multi_pixel_decode() {
        // 2x1 image, 2 white pixels
        let profile = make_profile(2, 1, true, false);
        let img = decode(&[0xFF, 0xFF, 0xFF, 0xFF], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(&img.data, &[0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn row_stride_is_data_driven() {
        // 55-pixel padded format (like F1061): rowStride = src.len / h
        // The data is larger than w * 2 because of padding.
        let w = 55;
        let h = 55;
        let padded_data = vec![0u8; 6400]; // larger than 55*55*2 = 6050
        let profile = Profile {
            width: w,
            height: h,
            encoding: Encoding::Rgb565,
            frame_byte_length: 6400,
            little_endian: true,
            ..Default::default()
        };
        let img = decode(&padded_data, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 55);
        assert_eq!(img.height, 55);
        assert_eq!(img.data.len(), 55 * 55 * 4);
    }
}
