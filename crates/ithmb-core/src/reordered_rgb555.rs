//! `ReorderedRGB555` decoder — 15-bit RGB with little-endian byte order.
//!
//! This variant of RGB555 stores each 16-bit pixel in little-endian byte order
//! (low byte first, high byte second), matching the format Apple uses in
//! `.ithmb` files for `ReorderedRGB555` profiles.
//! RGB555.
//!
//! Each pixel is 2 bytes (16 bits, MSB unused), laid out as:
//!
//! ```text
//! Default (swap_rgb_channels = false):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
//!
//! Stored in little-endian byte order:
//!   Byte 0: [|G2|G1|G0|B4|B3|B2|B1|B0|]  (low byte)
//!   Byte 1: [|x|R4|R3|R2|R1|R0|G4|G3|]  (high byte)
//!
//! BGR15  (swap_rgb_channels = true, used by iPhone 2G):
//!   Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
//!          x  B4 B3 B2 B1 B0 G4 G3 G2 G1 G0 R4 R3 R2 R1 R0
//! ```
//!
//! Output is BGRA 8-bit per channel.

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::sync::atomic::AtomicBool;

/// Decodes a `ReorderedRGB555` frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data (2 bytes per pixel, little-endian byte order).
/// * `profile` — The profile describing this frame's dimensions and flags.
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] if `src` is smaller than `w * h * 2`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn decode(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    let (data, w, h) =
        crate::decoder_helpers::validate_dimensions(src, profile, "width and height must be positive", 2)?;
    let src = &*data;
    let le = profile.little_endian;
    let swap = profile.swap_rgb_channels;
    let total_pixels = w * h;

    let mut dst = vec![0u8; total_pixels * 4];
    let bits = std::cmp::max(w, h).next_power_of_two().trailing_zeros();

    // Stack-allocated row offset LUT: max known profile width is 480,
    // so 1024 u32s (4 KB) is more than sufficient and avoids a heap alloc.
    let mut row_offsets = [0u32; 1024];

    for y in 0..h {
        crate::pixel_utils::check_canceled(canceled, "reordered_rgb555 decode canceled")?;
        let dst_start = y * w * 4;

        // Phase 1: Build row offset table via Morton x-increment (C's approach)
        // Full morton_interleave for x=0, then 5-op increment for x=1..w-1
        let mut z = morton_interleave(0, y as u32, bits);
        for entry in &mut row_offsets[..w] {
            *entry = z;
            z = morton_inc_x(z);
        }
        // Phase 3: Standard 4-pixel SIMD/scalar path (non-AVX2 or no simd feature)
        let mut x = 0usize;
        while x + 4 <= w {
            let mut pixels = [[0u8; 2]; 4];
            for (i, pixel) in pixels.iter_mut().enumerate() {
                let src_idx = row_offsets[x + i] as usize * 2;
                *pixel = if src_idx + 1 < src.len() {
                    [src[src_idx], src[src_idx + 1]]
                } else {
                    [0, 0]
                };
                if !le {
                    pixel.swap(0, 1);
                }
            }
            let bgra = crate::simd::rgb555_pack_to_bgra(pixels, swap);
            let dst_idx = dst_start + x * 4;
            dst[dst_idx..dst_idx + 16].copy_from_slice(&bgra);
            x += 4;
        }
        for (rx, entry) in row_offsets[..w].iter().enumerate().skip(x) {
            let src_idx = *entry as usize * 2;
            let raw = if src_idx + 1 < src.len() {
                if le {
                    u16::from_le_bytes([src[src_idx], src[src_idx + 1]])
                } else {
                    u16::from_be_bytes([src[src_idx], src[src_idx + 1]])
                }
            } else {
                0x0000
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
            let off = dst_start + rx * 4;
            dst[off] = crate::pixel_utils::msb_replicate_5(b5);
            dst[off + 1] = crate::pixel_utils::msb_replicate_5(g5);
            dst[off + 2] = crate::pixel_utils::msb_replicate_5(r5);
            dst[off + 3] = 255;
        }
    }

    Ok(DecodedImage {
        data: dst,
        width: w as u32,
        height: h as u32,
    })
}

/// Interleave bits of `x` and `y` into Morton Z-order (y₀ x₀ y₁ x₁ …).
///
/// Used by `EncodeReorderedRgb555` — the encoder counterpart of the
/// reordered-RGB555 decoder.
#[inline]
pub(crate) fn morton_interleave(x: u32, y: u32, bits: u32) -> u32 {
    let mut z = 0u32;
    for i in 0..bits {
        z |= ((x >> i) & 1) << (2 * i + 1);
        z |= ((y >> i) & 1) << (2 * i);
    }
    z
}

/// Increment the x-component of a Morton Z-order code in ~5 ops.
///
/// Given `z = morton(x, y, bits)`, returns `morton(x+1, y, bits)`.
///
/// The `morton_interleave` function places x at odd bit positions (2i+1)
/// and y at even bit positions (2i). To increment x, we fill even bits
/// with 1 so carry propagates through them, then mask to odd bits.
#[inline]
fn morton_inc_x(z: u32) -> u32 {
    let x_bits = z & 0xAAAA_AAAA; // odd bits = x
    let y_bits = z & 0x5555_5555; // even bits = y
    let x_inc = ((x_bits | 0x5555_5555) + 1) & 0xAAAA_AAAA;
    y_bits | x_inc
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Encoding, Profile};
    use std::sync::atomic::AtomicBool;

    fn make_profile(w: i32, h: i32, swap: bool) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::ReorderedRgb555,
            frame_byte_length: w * h * 2,
            little_endian: true,
            swap_rgb_channels: swap,
            ..Default::default()
        }
    }

    #[test]
    fn zero_dimensions_returns_err() {
        let profile = make_profile(0, 100, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn negative_dimension_returns_err() {
        let profile = make_profile(-1, 100, false);
        let result = decode(b"", &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
    }

    #[test]
    fn too_short_returns_buffer_too_short() {
        let profile = make_profile(100, 100, false);
        let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
    }

    #[test]
    fn buffer_too_short_reports_exact_counts() {
        let profile = make_profile(14, 10, false);
        // 14*10*2 = 280 needed, deficit=270 > 256 → still BufferTooShort
        let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
        match result {
            Err(DecodeError::BufferTooShort {
                expected: 280,
                actual: 10,
            }) => {} // ok
            other => panic!("expected BufferTooShort(280, 10), got {other:?}"),
        }
    }

    #[test]
    fn dst_allocation_matches_geometry() {
        let profile = make_profile(3, 2, false);
        let pixels = vec![0u8; 3 * 2 * 2];
        let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data.len(), 3 * 2 * 4);
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
    }

    #[test]
    fn solid_white_pixel() {
        // RGB555 0x7FFF → R=31, G=31, B=31 → all 255 in BGRA
        // Big-endian: [0xFF, 0x7F]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0xFF, 0x7F], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn solid_black_pixel() {
        // RGB555 0x0000 → all zeros
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x00, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0, 0, 0, 255]);
    }

    #[test]
    fn solid_red_pixel() {
        // Layout xRRRRRGGGGGBBBBB, R=31 → bits 14..10 = 11111
        // Pixel = 0x7C00, LE: [0x00, 0x7C]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=0, G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn solid_blue_pixel() {
        // Layout xRRRRRGGGGGBBBBB, B=31 → bits 4..0 = 11111
        // Pixel = 0x001F, LE: [0x1F, 0x00]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=255, G=0, R=0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn solid_green_pixel() {
        // Layout xRRRRRGGGGGBBBBB, G=31 → bits 9..5 = 11111
        // Pixel = 0x03E0, LE: [0xE0, 0x03]
        let profile = make_profile(1, 1, false);
        let img = decode(&[0xE0, 0x03], &profile, &AtomicBool::new(false)).unwrap();
        // BGRA: B=0, G=255, R=0
        assert_eq!(img.data, vec![0, 0xFF, 0, 255]);
    }

    #[test]
    fn decode_swap_rgb_channels() {
        // swap_rgb_channels=true: layout becomes xBBBBBGGGGGRRRRR
        // Blue=31 → bits 14..10 = 11111, pixel = 0x7C00
        // Big-endian: [0x00, 0x7C]
        // With swap, high bits = B, so B_out = 255
        let profile = make_profile(1, 1, true);
        let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
        // Swap mode: B from high bits = 31 → 255, R from low bits = 0
        assert_eq!(img.data, vec![0xFF, 0, 0, 255]);
    }

    #[test]
    fn swap_mode_red_stays_low() {
        // swap_rgb_channels=true: xBBBBBGGGGGRRRRR
        // Red=31 → bits 4..0 = 11111, pixel = 0x001F
        // Big-endian: [0x1F, 0x00]
        let profile = make_profile(1, 1, true);
        let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
        // Swap mode: R from low bits = 31 → R_out=255
        // BGRA: B=0 (from high bits), G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn two_pixel_decode() {
        // 2 white pixels in Morton order: z=0→white, z=1→gap, z=2→white
        let profile = make_profile(2, 1, false);
        // Morton-ordered: pixel0 at z=0, gap at z=1, pixel1 at z=2
        let img = decode(&[0xFF, 0x7F, 0x00, 0x00, 0xFF, 0x7F], &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn two_pixel_different_colors() {
        // Morton-ordered 2×1: z=0→pixel0, z=1→gap, z=2→pixel1
        let profile = make_profile(2, 1, false);
        let data = [
            0x00, 0x7C, // z=0: red   (0x7C00 BE)
            0x00, 0x00, // z=1: gap (zero-filled)
            0x1F, 0x00, // z=2: blue  (0x001F BE)
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
    fn msb_replicate_clamping() {
        assert_eq!(crate::pixel_utils::msb_replicate_5(31), 0xFF);
        assert_eq!(crate::pixel_utils::msb_replicate_5(0), 0x00);
        assert_eq!(crate::pixel_utils::msb_replicate_5(16), 0x84);
        assert_eq!(crate::pixel_utils::msb_replicate_5(8), 0x42);
        assert_eq!(crate::pixel_utils::msb_replicate_5(1), 0x08);
    }

    #[test]
    fn output_has_correct_alpha() {
        let profile = make_profile(2, 2, false);
        let pixels = vec![0u8; 8];
        let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
        for i in 0..4 {
            assert_eq!(img.data[i * 4 + 3], 255);
        }
    }
    #[test]
    fn decode_4x4_morton_order_quadrants() {
        // 4×4 image with one color per 2×2 quadrant encoded in Z-order Morton:
        //   Red quadrant at top-left,  Green at top-right,
        //   Blue at bottom-left, White at bottom-right.
        //
        // With morton_interleave (y in even bits, x in odd bits), 4×4 Z-order is:
        //   z=0..3  → (0,0),(1,0),(0,1),(1,1) = Red    quadrant (top-left)
        //   z=4..7  → (0,2),(1,2),(0,3),(1,3) = Blue   quadrant (bottom-left)
        //   z=8..11 → (2,0),(3,0),(2,1),(3,1) = Green  quadrant (top-right)
        //   z=12..15→ (2,2),(3,2),(2,3),(3,3) = White  quadrant (bottom-right)
        let profile = make_profile(4, 4, false);

        // Morton-ordered pixel data in little-endian RGB555:
        // RED=0x7C00, GREEN=0x03E0, BLUE=0x001F, WHITE=0x7FFF
        let encoded = vec![
            0x00, 0x7C, 0x00, 0x7C, 0x00, 0x7C, 0x00, 0x7C, // z=0..3:  4 RED
            0x1F, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x1F, 0x00, // z=4..7:  4 BLUE
            0xE0, 0x03, 0xE0, 0x03, 0xE0, 0x03, 0xE0, 0x03, // z=8..11: 4 GREEN
            0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x7F, 0xFF, 0x7F, // z=12..15:4 WHITE
        ];

        let img = decode(&encoded, &profile, &AtomicBool::new(false)).unwrap();
        let data = &img.data;

        // Row 0: Red, Red, Green, Green
        assert_eq!(&data[0..4], &[0, 0, 255, 255], "(0,0)");
        assert_eq!(&data[4..8], &[0, 0, 255, 255], "(1,0)");
        assert_eq!(&data[8..12], &[0, 255, 0, 255], "(2,0)");
        assert_eq!(&data[12..16], &[0, 255, 0, 255], "(3,0)");

        // Row 1: Red, Red, Green, Green
        assert_eq!(&data[16..20], &[0, 0, 255, 255], "(0,1)");
        assert_eq!(&data[20..24], &[0, 0, 255, 255], "(1,1)");
        assert_eq!(&data[24..28], &[0, 255, 0, 255], "(2,1)");
        assert_eq!(&data[28..32], &[0, 255, 0, 255], "(3,1)");

        // Row 2: Blue, Blue, White, White
        assert_eq!(&data[32..36], &[255, 0, 0, 255], "(0,2)");
        assert_eq!(&data[36..40], &[255, 0, 0, 255], "(1,2)");
        assert_eq!(&data[40..44], &[255, 255, 255, 255], "(2,2)");
        assert_eq!(&data[44..48], &[255, 255, 255, 255], "(3,2)");

        // Row 3: Blue, Blue, White, White
        assert_eq!(&data[48..52], &[255, 0, 0, 255], "(0,3)");
        assert_eq!(&data[52..56], &[255, 0, 0, 255], "(1,3)");
        assert_eq!(&data[56..60], &[255, 255, 255, 255], "(2,3)");
        assert_eq!(&data[60..64], &[255, 255, 255, 255], "(3,3)");
    }
}
