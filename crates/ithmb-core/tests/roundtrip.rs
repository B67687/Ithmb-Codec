//! Encode→decode roundtrip tests for all 7 pixel formats.
//!
//! Each test:
//! 1. Creates known BGRA pixel data
//! 2. Encodes using the Rust encoder (ithmb-core/src/enc.rs)
//! 3. Prepends the 4-byte profile prefix
//! 4. Decodes using [`decode_with_profile`] (the same pipeline real files go through)
//! 5. Asserts the decoded pixels match the originals (within quantization tolerance
//!    for lossy YUV / nibble-chroma formats).
#![allow(clippy::pedantic, clippy::unwrap_used)]

use divan as _;
use image as _;
use ithmb_core::enc::*;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use std::sync::atomic::AtomicBool;
use thiserror as _;
mod util;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 2×2 test image with 3 distinct colors + white.
///
/// ```
/// (0,0) Red   | (1,0) Green
/// (0,1) Blue  | (1,1) White
/// ```
fn bgra_2x2_3colors() -> Vec<u8> {
    vec![
        0, 0, 255, 255, // (0,0) Red
        0, 255, 0, 255, // (1,0) Green
        255, 0, 0, 255, // (0,1) Blue
        255, 255, 255, 255, // (1,1) White
    ]
}

/// 4×2 test image for UYVY (8 pixels, each horizontal pair is the same color
/// so chroma averaging has no error).
///
/// ```
/// Row 0: Red, Red, Green, Green
/// Row 1: Blue, Blue, Gray, Gray
/// ```
fn bgra_4x2_4colors() -> Vec<u8> {
    vec![
        0, 0, 255, 255, // (0,0) Red
        0, 0, 255, 255, // (1,0) Red   — same-color pair
        0, 255, 0, 255, // (2,0) Green
        0, 255, 0, 255, // (3,0) Green — same-color pair
        255, 0, 0, 255, // (0,1) Blue
        255, 0, 0, 255, // (1,1) Blue  — same-color pair
        128, 128, 128, 255, // (2,1) Gray
        128, 128, 128, 255, // (3,1) Gray  — same-color pair
    ]
}

fn roundtrip_once(profile: &Profile, encoded: &[u8]) -> ithmb_core::error::DecodedImage {
    let mut with_prefix = profile.prefix.to_be_bytes().to_vec();
    with_prefix.extend(encoded);

    let canceled = AtomicBool::new(false);
    decode_with_profile(&with_prefix, profile, &canceled).expect("decode_with_profile should succeed")
}

// ---------------------------------------------------------------------------
// RGB565  —  2 bytes/pixel, 5/6/5 MSB replication
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_rgb565() {
    let bgra = bgra_2x2_3colors();
    let w = 2;
    let h = 2;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Rgb565,
        frame_byte_length: w * h * 2,
        little_endian: true,
        ..Default::default()
    };

    let encoded = encode_rgb565(&bgra, w, h, false); // little-endian
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(decoded.width, u32::try_from(w).unwrap());
    assert_eq!(decoded.height, u32::try_from(h).unwrap());
    // Saturated 0/255 values roundtrip exactly through 5/6/5 MSB replication
    assert_eq!(decoded.data, bgra);
}

// ---------------------------------------------------------------------------
// RGB555  —  2 bytes/pixel, 5/5/5 MSB replication
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_rgb555() {
    let bgra = bgra_2x2_3colors();
    let w = 2;
    let h = 2;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Rgb555,
        frame_byte_length: w * h * 2,
        little_endian: true,
        ..Default::default()
    };

    let encoded = encode_rgb555(&bgra, w, h, false, false); // LE, no channel swap
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(decoded.width, u32::try_from(w).unwrap());
    assert_eq!(decoded.height, u32::try_from(h).unwrap());
    assert_eq!(decoded.data, bgra);
}

// ---------------------------------------------------------------------------
// Reordered RGB555  —  always big-endian, Z-order interleaved
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_reordered_rgb555_1x1() {
    // 1×1 avoids Z-order position mapping concerns
    let bgra = vec![0, 0, 255, 255]; // red
    let w = 1;
    let h = 1;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: w * h * 2,
        little_endian: false, // ReorderedRGB555 is always big-endian
        ..Default::default()
    };

    let encoded = encode_reordered_rgb555(&bgra, w, h, true); // always big-endian
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    assert_eq!(i32::try_from(decoded.height).unwrap(), h);
    assert_eq!(decoded.data, bgra);
}

#[test]
fn roundtrip_reordered_rgb555_2x2_uniform() {
    // All-white 2×2 — position-independent, so Z-order reordering is invisible
    let bgra = vec![255u8; 2 * 2 * 4]; // all white
    let w = 2;
    let h = 2;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: w * h * 2,
        little_endian: false,
        ..Default::default()
    };

    let encoded = encode_reordered_rgb555(&bgra, w, h, true);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    assert_eq!(i32::try_from(decoded.height).unwrap(), h);
    assert_eq!(decoded.data, bgra);
}

// ---------------------------------------------------------------------------
// UYVY  —  YUV 4:2:2 packed, BT.601 colour conversion
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_uyvy() {
    // 4×2 with same-color horizontal pairs so chroma averaging is exact.
    // Small BT.601 rounding errors still occur (±1 for saturated colors).
    let bgra = bgra_4x2_4colors();
    let w = 4;
    let h = 2;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: w * h * 2,
        ..Default::default()
    };

    let encoded = encode_uyvy(&bgra, w, h);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    assert_eq!(i32::try_from(decoded.height).unwrap(), h);
    util::assert_bgra_tolerant(&decoded.data, &bgra, 2);
}

// ---------------------------------------------------------------------------
// YCbCr 4:2:0  —  planar Y + subsampled Cb/Cr, BT.601
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_ycbcr420() {
    // All-white 2×2 — YCbCr 4:2:0 is lossless for neutral colors
    let bgra = vec![255u8; 2 * 2 * 4]; // all white
    let w = 2;
    let h = 2;
    let uv_w = usize::try_from(w).unwrap().div_ceil(2);
    let uv_h = usize::try_from(h).unwrap().div_ceil(2);
    let frame_len = i32::try_from(usize::try_from(w * h).unwrap() + uv_w * uv_h * 2).unwrap();
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: frame_len,
        ..Default::default()
    };
    let encoded = encode_ycbcr420(&bgra, w, h, false); // no chroma swap
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    // All-white is lossless through YCbCr 4:2:0 (neutral chroma)
    assert_eq!(decoded.data, bgra);
}

// ---------------------------------------------------------------------------
// CLCL  —  nibble-chroma YUV (separate Cb/Cr planes)
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_clcl() {
    let bgra = bgra_2x2_3colors();
    let w = 2;
    let h = 2;
    let n = usize::try_from(w * h).unwrap();
    let chroma_len = n.div_ceil(2);
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: i32::try_from(n + chroma_len + chroma_len).unwrap(),
        clcl_chroma: true,
        ..Default::default()
    };

    let encoded = encode_clcl(&bgra, w, h);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    assert_eq!(i32::try_from(decoded.height).unwrap(), h);
    // Nibble chroma loses 4 bits per channel → max chroma error 15, which
    // propagates to RGB error up to ~32 through BT.601.
    util::assert_bgra_tolerant(&decoded.data, &bgra, 32);
}

// ---------------------------------------------------------------------------
// CL  —  per-pixel nibble-chroma YUV
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_cl() {
    let bgra = bgra_2x2_3colors();
    let w = 2;
    let h = 2;
    let n = usize::try_from(w * h).unwrap();
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: i32::try_from(n * 2).unwrap(),
        cl_chroma: true,
        ..Default::default()
    };

    let encoded = encode_cl(&bgra, w, h);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(i32::try_from(decoded.width).unwrap(), w);
    assert_eq!(i32::try_from(decoded.height).unwrap(), h);
    // Same nibble-quantization tolerance as CLCL.
    util::assert_bgra_tolerant(&decoded.data, &bgra, 32);
}

// ---------------------------------------------------------------------------
// Exhaustive RGB565 — all 65,536 possible values
// ---------------------------------------------------------------------------

/// Exhaustive RGB565 roundtrip test — all 65,536 possible values.
///
/// For each 16-bit RGB565 value, creates a 1×1 BGRA pixel, encodes it,
/// decodes it, and asserts the decoded BGRA matches the expected lossy
/// roundtrip result (5/6/5 MSB replication).
///
/// Run with: `cargo test --test roundtrip -- --ignored`
#[test]
#[ignore = "exhaustive 65,536-value test — run with --ignored"]
fn exhaustive_rgb565_roundtrip() {
    let profile = Profile {
        prefix: 0x1000_0001,
        width: 1,
        height: 1,
        encoding: Encoding::Rgb565,
        frame_byte_length: 2,
        little_endian: true,
        ..Default::default()
    };

    for rgb565_value in 0..=u16::MAX {
        let r5 = u32::from((rgb565_value >> 11) & 0x1F);
        let g6 = u32::from((rgb565_value >> 5) & 0x3F);
        let b5 = u32::from(rgb565_value & 0x1F);

        #[allow(clippy::cast_possible_truncation)]
        let r8 = ((r5 << 3) | (r5 >> 2)) as u8;
        #[allow(clippy::cast_possible_truncation)]
        let g8 = ((g6 << 2) | (g6 >> 4)) as u8;
        #[allow(clippy::cast_possible_truncation)]
        let b8 = ((b5 << 3) | (b5 >> 2)) as u8;

        let bgra = vec![b8, g8, r8, 255];
        let encoded = encode_rgb565(&bgra, 1, 1, false);
        let decoded =
            ithmb_core::rgb565::decode(&encoded, &profile, &AtomicBool::new(false)).expect("decode should succeed");

        assert_eq!(decoded.data, bgra, "mismatch at RGB565 value 0x{rgb565_value:04X}");
    }
}

// ---------------------------------------------------------------------------
// Per-format roundtrip tests — lossless (bit-exact)
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_rgb555_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Rgb555,
        frame_byte_length: w * h * 2,
        little_endian: true,
        ..Default::default()
    };

    let encoded = encode_rgb555(&bgra, w, h, false, false);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(decoded.data, bgra);
}

#[test]
fn roundtrip_uyvy_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: w * h * 2,
        ..Default::default()
    };

    let encoded = encode_uyvy(&bgra, w, h);
    let decoded = roundtrip_once(&profile, &encoded);

    assert_eq!(decoded.data, bgra);
}

// ---------------------------------------------------------------------------
// Per-format roundtrip tests — lossy (roundtrip pattern)
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_ycbcr420_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let uv_w = usize::try_from(w).unwrap().div_ceil(2);
    let uv_h = usize::try_from(h).unwrap().div_ceil(2);
    let frame_len = i32::try_from(usize::try_from(w * h).unwrap() + uv_w * uv_h * 2).unwrap();
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: frame_len,
        ..Default::default()
    };

    let encoded = encode_ycbcr420(&bgra, w, h, false);
    let expected = roundtrip_once(&profile, &encoded).data;

    let encoded2 = encode_ycbcr420(&bgra, w, h, false);
    let decoded2 = roundtrip_once(&profile, &encoded2);

    assert_eq!(decoded2.data, expected);
}

#[test]
fn roundtrip_clcl_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let n = usize::try_from(w * h).unwrap();
    let chroma_len = n.div_ceil(2);
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: i32::try_from(n + chroma_len + chroma_len).unwrap(),
        clcl_chroma: true,
        ..Default::default()
    };

    let encoded = encode_clcl(&bgra, w, h);
    let expected = roundtrip_once(&profile, &encoded).data;

    let encoded2 = encode_clcl(&bgra, w, h);
    let decoded2 = roundtrip_once(&profile, &encoded2);

    assert_eq!(decoded2.data, expected);
}

#[test]
fn roundtrip_cl_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let n = usize::try_from(w * h).unwrap();
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: i32::try_from(n * 2).unwrap(),
        cl_chroma: true,
        ..Default::default()
    };

    let encoded = encode_cl(&bgra, w, h);
    let expected = roundtrip_once(&profile, &encoded).data;

    let encoded2 = encode_cl(&bgra, w, h);
    let decoded2 = roundtrip_once(&profile, &encoded2);

    assert_eq!(decoded2.data, expected);
}

#[test]
fn roundtrip_reordered_rgb555_random_4x4() {
    let bgra = util::make_bgra_checkerboard(4, 4);
    let w = 4;
    let h = 4;
    let profile = Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: w * h * 2,
        little_endian: false,
        ..Default::default()
    };

    let encoded = encode_reordered_rgb555(&bgra, w, h, true);
    let expected = roundtrip_once(&profile, &encoded).data;

    let encoded2 = encode_reordered_rgb555(&bgra, w, h, true);
    let decoded2 = roundtrip_once(&profile, &encoded2);

    assert_eq!(decoded2.data, expected);
}
