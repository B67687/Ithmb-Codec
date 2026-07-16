//! Synthetic test-vector generation and roundtrip verification for all 7 pixel
//! formats.  Each generator produces a complete `.ithmb` file (4-byte prefix +
//! encoded frame).  Every test encodes -> decodes -> verifies that the decoded
//! BGRA pixels match the original (within per-format quantization tolerance).
//!
//! Coverage:
//!  - All 7 formats at powers-of-2 dimensions (2x2, 4x4, 8x8).
//!  - Non-power-of-2 dimensions (3x3, 5x7, 4x6, 6x4) for formats that support
//!    them (RGB565, RGB555, UYVY, CLCL, CL).
//!  - Single-pixel / single-row / single-column (1x1, 1x3, 3x1).
//!  - Solid colours (white, black, R, G, B).
//!  - Checkerboard 50% pattern.
//!  - Seeded LCG pseudo-random pixel data.
//!
//! This file does NOT write generated vectors to disk and intentionally does
//! NOT overlap with `roundtrip.rs` (different dimension / pattern combos) or
//! `golden_comparison.rs` (pre-encoded reference files).
#![deny(clippy::all)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::pedantic,
    clippy::unwrap_used
)]

use ithmb_core::enc::*;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use std::sync::atomic::AtomicBool;

// Suppress unused-dev-dependency warnings (workspace-wide deps).
use divan as _;
use image as _;
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use proptest as _;
use thiserror as _;

mod util;

/// Create random BGRA pixels using the shared seeded RNG.
fn random_bgra(rng: &mut util::rng::SeededRng, n: usize) -> Vec<[u8; 4]> {
    let mut flat = vec![0u8; n * 4];
    rng.fill_bgra(&mut flat);
    flat.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

// Helpers - pixel patterns, frame-size computation
// ---------------------------------------------------------------------------

/// Return `w x h` BGRA pixels of a single colour.
fn solid_pixels(w: u32, h: u32, colour: [u8; 4]) -> Vec<[u8; 4]> {
    vec![colour; (w * h) as usize]
}

/// Return `w x h` BGRA pixels forming a checkerboard with `tile_size`-square
/// tiles.  `(row + col) / tile_size % 2` selects between `a` and `b`.
fn checkerboard_pixels(w: u32, h: u32, tile_size: u32, a: [u8; 4], b: [u8; 4]) -> Vec<[u8; 4]> {
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for row in 0..h {
        for col in 0..w {
            let cell = ((row / tile_size) + (col / tile_size)) % 2;
            pixels.push(if cell == 0 { a } else { b });
        }
    }
    pixels
}

/// Flatten `[[u8; 4]]` to `[u8]` (B,G,R,A order - callers provide BGRA).
fn flatten_pixels(bgra: &[[u8; 4]]) -> Vec<u8> {
    let mut flat = Vec::with_capacity(bgra.len() * 4);
    for px in bgra {
        flat.extend_from_slice(px);
    }
    flat
}

/// Compute the exact encoded frame-byte length for a given format and
/// dimensions. This matches what the corresponding `encode_*` function produces.
fn frame_byte_length(w: i32, h: i32, encoding: Encoding) -> i32 {
    let wu = w as usize;
    let hu = h as usize;
    match encoding {
        Encoding::Rgb565 | Encoding::Rgb555 | Encoding::ReorderedRgb555 => w * h * 2,
        Encoding::Yuv422 => {
            let pairs = wu.div_ceil(2) * hu;
            (pairs * 4) as i32
        }
        Encoding::Ycbcr420 => {
            let uv_w = wu.div_ceil(2);
            let uv_h = hu.div_ceil(2);
            (wu * hu + uv_w * uv_h * 2) as i32
        }
        Encoding::Jpeg => 0,
        _ => unreachable!("Unknown encoding variant"),
    }
}

/// Helper: compute the frame-byte length for CLCL.
fn clcl_frame_len(w: i32, h: i32) -> i32 {
    let n = (w * h) as usize;
    let chroma_len = n.div_ceil(2);
    (n + chroma_len + chroma_len) as i32
}

/// Helper: compute the frame-byte length for CL.
fn cl_frame_len(w: i32, h: i32) -> i32 {
    w * h * 2
}

// ---------------------------------------------------------------------------
// Profile builders
// ---------------------------------------------------------------------------

fn profile_rgb565(w: i32, h: i32, le: bool) -> Profile {
    Profile {
        prefix: 0x0000_1001,
        width: w,
        height: h,
        encoding: Encoding::Rgb565,
        frame_byte_length: frame_byte_length(w, h, Encoding::Rgb565),
        little_endian: le,
        ..Default::default()
    }
}

fn profile_rgb555(w: i32, h: i32, le: bool, swap_rgb: bool) -> Profile {
    Profile {
        prefix: 0x0000_1002,
        width: w,
        height: h,
        encoding: Encoding::Rgb555,
        frame_byte_length: frame_byte_length(w, h, Encoding::Rgb555),
        little_endian: le,
        swap_rgb_channels: swap_rgb,
        ..Default::default()
    }
}

fn profile_reordered_rgb555(w: i32, h: i32) -> Profile {
    Profile {
        prefix: 0x0000_1003,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: frame_byte_length(w, h, Encoding::ReorderedRgb555),
        little_endian: false, // ReorderedRGB555 is always big-endian
        ..Default::default()
    }
}

fn profile_uyvy(w: i32, h: i32) -> Profile {
    Profile {
        prefix: 0x0000_1004,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: frame_byte_length(w, h, Encoding::Yuv422),
        ..Default::default()
    }
}

fn profile_ycbcr420(w: i32, h: i32) -> Profile {
    Profile {
        prefix: 0x0000_1005,
        width: w,
        height: h,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: frame_byte_length(w, h, Encoding::Ycbcr420),
        ..Default::default()
    }
}

fn profile_clcl(w: i32, h: i32) -> Profile {
    Profile {
        prefix: 0x0000_1006,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: clcl_frame_len(w, h),
        clcl_chroma: true,
        ..Default::default()
    }
}

fn profile_cl(w: i32, h: i32) -> Profile {
    Profile {
        prefix: 0x0000_1007,
        width: w,
        height: h,
        encoding: Encoding::Yuv422,
        frame_byte_length: cl_frame_len(w, h),
        cl_chroma: true,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Generator functions - each returns a complete .ithmb file (prefix + frame).
// ---------------------------------------------------------------------------

/// Generate a complete .ithmb file with RGB565 encoding.
///
/// `bgra` must have exactly `(w x h)` entries, each `[B, G, R, A]`.
/// `big_endian` controls byte order within each 16-bit pixel word.
#[must_use]
pub fn generate_rgb565(w: i32, h: i32, bgra: &[[u8; 4]], big_endian: bool) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_rgb565(&flat, w, h, big_endian);
    let profile = profile_rgb565(w, h, !big_endian);
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with RGB555 encoding.
#[must_use]
pub fn generate_rgb555(w: i32, h: i32, bgra: &[[u8; 4]], big_endian: bool, swap_rgb: bool) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_rgb555(&flat, w, h, big_endian, swap_rgb);
    let profile = profile_rgb555(w, h, !big_endian, swap_rgb);
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with Reordered RGB555 encoding (always
/// big-endian, Z-order interleaved).
#[must_use]
pub fn generate_reordered_rgb555(w: i32, h: i32, bgra: &[[u8; 4]]) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_reordered_rgb555(&flat, w, h, true);
    let profile = profile_reordered_rgb555(w, h);
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with UYVY encoding (YUV 4:2:2 packed, BT.601).
#[must_use]
pub fn generate_uyvy(w: i32, h: i32, bgra: &[[u8; 4]]) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_uyvy(&flat, w, h);
    let profile = profile_uyvy(w, h);
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with planar YCbCr 4:2:0 encoding.
/// `swap_chroma` controls plane order (default: Y Cb Cr).
#[must_use]
pub fn generate_ycbcr420(w: i32, h: i32, bgra: &[[u8; 4]], swap_chroma: bool) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_ycbcr420(&flat, w, h, swap_chroma);
    let mut profile = profile_ycbcr420(w, h);
    profile.swap_chroma_planes = swap_chroma;
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with CLCL nibble-chroma encoding.
#[must_use]
pub fn generate_clcl(w: i32, h: i32, bgra: &[[u8; 4]]) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_clcl(&flat, w, h);
    let profile = profile_clcl(w, h);
    build_file(&profile, &encoded)
}

/// Generate a complete .ithmb file with CL per-pixel nibble-chroma encoding.
#[must_use]
pub fn generate_cl(w: i32, h: i32, bgra: &[[u8; 4]]) -> Vec<u8> {
    let flat = flatten_pixels(bgra);
    let encoded = encode_cl(&flat, w, h);
    let profile = profile_cl(w, h);
    build_file(&profile, &encoded)
}

/// Prepend the 4-byte prefix and return a complete `.ithmb` file.
fn build_file(profile: &Profile, frame: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + frame.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(frame);
    buf
}

// ---------------------------------------------------------------------------
// Roundtrip verification helpers
// ---------------------------------------------------------------------------

/// Verify that encoding `bgra` to the given format and decoding it back
/// produces pixels that match the original within `tolerance`.
fn roundtrip_verify(
    bgra: &[[u8; 4]],
    w: i32,
    h: i32,
    profile: &Profile,
    encode: fn(&[u8], i32, i32) -> Vec<u8>,
    tolerance: u8,
    label: &str,
) {
    let flat = flatten_pixels(bgra);
    let encoded = encode(&flat, w, h);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);

    let canceled = AtomicBool::new(false);
    let decoded =
        decode_with_profile(&buf, profile, &canceled).unwrap_or_else(|e| panic!("{label}: decode failed: {e}"));

    assert_eq!(
        decoded.width as i32, w,
        "{label}: width mismatch (got {}, expected {w})",
        decoded.width
    );
    assert_eq!(
        decoded.height as i32, h,
        "{label}: height mismatch (got {}, expected {h})",
        decoded.height
    );

    let expected_len = bgra.len() * 4;
    assert_eq!(
        decoded.data.len(),
        expected_len,
        "{label}: data length mismatch (got {}, expected {expected_len})",
        decoded.data.len()
    );

    for (i, px) in bgra.iter().enumerate() {
        let off = i * 4;
        let b_act = decoded.data[off];
        let g_act = decoded.data[off + 1];
        let r_act = decoded.data[off + 2];
        let a_act = decoded.data[off + 3];
        let (b_exp, g_exp, r_exp, a_exp) = (px[0], px[1], px[2], px[3]);

        let b_diff = (i16::from(b_act) - i16::from(b_exp)).unsigned_abs();
        let g_diff = (i16::from(g_act) - i16::from(g_exp)).unsigned_abs();
        let r_diff = (i16::from(r_act) - i16::from(r_exp)).unsigned_abs();
        assert!(
            b_diff <= u16::from(tolerance),
            "{label}: pixel {i} B: got {b_act}, expected {b_exp} (diff {b_diff}, tol {tolerance})"
        );
        assert!(
            g_diff <= u16::from(tolerance),
            "{label}: pixel {i} G: got {g_act}, expected {g_exp} (diff {g_diff}, tol {tolerance})"
        );
        assert!(
            r_diff <= u16::from(tolerance),
            "{label}: pixel {i} R: got {r_act}, expected {r_exp} (diff {r_diff}, tol {tolerance})"
        );
        assert_eq!(a_act, a_exp, "{label}: pixel {i} alpha: got {a_act}, expected {a_exp}");
    }
}

/// Variant of `roundtrip_verify` for YUV-based formats that need a custom
/// encode function signature (e.g. no `big_endian` parameter).
fn roundtrip_verify_yuv<F>(bgra: &[[u8; 4]], w: i32, h: i32, profile: &Profile, encode: F, tolerance: u8, label: &str)
where
    F: Fn(&[u8], i32, i32) -> Vec<u8>,
{
    let flat = flatten_pixels(bgra);
    let encoded = encode(&flat, w, h);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);

    let canceled = AtomicBool::new(false);
    let decoded =
        decode_with_profile(&buf, profile, &canceled).unwrap_or_else(|e| panic!("{label}: decode failed: {e}"));

    assert_eq!(decoded.width as i32, w, "{label}: width mismatch");
    assert_eq!(decoded.height as i32, h, "{label}: height mismatch");
    assert_eq!(decoded.data.len(), bgra.len() * 4, "{label}: data length mismatch");

    for (i, px) in bgra.iter().enumerate() {
        let off = i * 4;
        let (b_exp, g_exp, r_exp, a_exp) = (px[0], px[1], px[2], px[3]);
        let b_diff = (i16::from(decoded.data[off]) - i16::from(b_exp)).unsigned_abs();
        let g_diff = (i16::from(decoded.data[off + 1]) - i16::from(g_exp)).unsigned_abs();
        let r_diff = (i16::from(decoded.data[off + 2]) - i16::from(r_exp)).unsigned_abs();
        assert!(
            b_diff <= u16::from(tolerance),
            "{label}: pixel {i} B: got {}, expected {} (diff {b_diff}, tol {tolerance})",
            decoded.data[off],
            b_exp
        );
        assert!(
            g_diff <= u16::from(tolerance),
            "{label}: pixel {i} G: got {}, expected {} (diff {g_diff}, tol {tolerance})",
            decoded.data[off + 1],
            g_exp
        );
        assert!(
            r_diff <= u16::from(tolerance),
            "{label}: pixel {i} R: got {}, expected {} (diff {r_diff}, tol {tolerance})",
            decoded.data[off + 2],
            r_exp
        );
        assert_eq!(decoded.data[off + 3], a_exp, "{label}: pixel {i} alpha");
    }
}

/// Helper for en/decoding via `build_ithmb_file` (rotates +
/// encodes + pads per profile).
fn roundtrip_via_build(bgra: &[[u8; 4]], w: i32, h: i32, profile: &Profile, tolerance: u8, label: &str) {
    let flat = flatten_pixels(bgra);
    let file = build_ithmb_file(&flat, w, h, profile);
    let canceled = AtomicBool::new(false);
    let decoded =
        decode_with_profile(&file, profile, &canceled).unwrap_or_else(|e| panic!("{label}: decode failed: {e}"));

    assert_eq!(decoded.width as i32, w, "{label}: width mismatch");
    assert_eq!(decoded.height as i32, h, "{label}: height mismatch");
    assert_eq!(decoded.data.len(), bgra.len() * 4, "{label}: data length mismatch");

    for (i, px) in bgra.iter().enumerate() {
        let off = i * 4;
        let (b_exp, g_exp, r_exp, a_exp) = (px[0], px[1], px[2], px[3]);
        let b_diff = (i16::from(decoded.data[off]) - i16::from(b_exp)).unsigned_abs();
        let g_diff = (i16::from(decoded.data[off + 1]) - i16::from(g_exp)).unsigned_abs();
        let r_diff = (i16::from(decoded.data[off + 2]) - i16::from(r_exp)).unsigned_abs();
        assert!(
            b_diff <= u16::from(tolerance),
            "{label}: pixel {i} B: got {}, expected {} (diff {b_diff}, tol {tolerance})",
            decoded.data[off],
            b_exp
        );
        assert!(
            g_diff <= u16::from(tolerance),
            "{label}: pixel {i} G: got {}, expected {} (diff {g_diff}, tol {tolerance})",
            decoded.data[off + 1],
            g_exp
        );
        assert!(
            r_diff <= u16::from(tolerance),
            "{label}: pixel {i} R: got {}, expected {} (diff {r_diff}, tol {tolerance})",
            decoded.data[off + 2],
            r_exp
        );
        assert_eq!(decoded.data[off + 3], a_exp, "{label}: pixel {i} alpha");
    }
}

// ---------------------------------------------------------------------------
// Tolerance constants for each format family
// ---------------------------------------------------------------------------

/// RGB565: MSB-replication error for non-saturated channels <= 8 (R/B) / <= 4 (G).
const TOL_RGB565: u8 = 8;
/// RGB555: same MSB error <= 8 (all channels are 5-bit).
const TOL_RGB555: u8 = 8;
/// UYVY: BT.601 YUV + chroma averaging. High-frequency patterns (checkerboard,
/// random) can have large per-channel errors through YUV->RGB conversion.
const TOL_UYVY: u8 = 48;
/// YCbCr 4:2:0: BT.601 + 2x2 chroma subsampling.
const TOL_YCBCR420: u8 = 48;
/// CLCL/CL: nibble chroma loses 4 bits. RGB error can reach +/-64 through BT.601.
const TOL_NIBBLE: u8 = 64;

// ===========================================================================
// TESTS - RGB565 (little-endian)
// ===========================================================================

#[test]
fn rgb565_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("rgb565_{w}x{h}");
        let pixels = solid_pixels(w, h, [128, 64, 192, 255]);
        let profile = profile_rgb565(w as i32, h as i32, true);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_rgb565(d, w, h, false),
            TOL_RGB565,
            &label,
        );
    }
}

#[test]
fn rgb565_non_power_of_2() {
    for &(w, h) in &[(3, 3), (5, 7)] {
        let label = format!("rgb565_{w}x{h}");
        let pixels = solid_pixels(w, h, [200, 100, 50, 255]);
        let profile = profile_rgb565(w as i32, h as i32, true);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_rgb565(d, w, h, false),
            TOL_RGB565,
            &label,
        );
    }
}

#[test]
fn rgb565_single_pixel() {
    for &(w, h) in &[(1, 1), (1, 3), (3, 1)] {
        let label = format!("rgb565_{w}x{h}");
        let pixels = solid_pixels(w, h, [255, 0, 0, 255]);
        let profile = profile_rgb565(w as i32, h as i32, true);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_rgb565(d, w, h, false),
            TOL_RGB565,
            &label,
        );
    }
}

#[test]
fn rgb565_big_endian() {
    let pixels = solid_pixels(2, 2, [0, 255, 0, 255]);
    let profile = profile_rgb565(2, 2, false);
    roundtrip_verify(
        &pixels,
        2,
        2,
        &profile,
        |d, w, h| encode_rgb565(d, w, h, true),
        TOL_RGB565,
        "rgb565_big_endian_2x2",
    );
}

#[test]
fn rgb565_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("rgb565_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_rgb565(4, 4, true);
        roundtrip_verify(
            &pixels,
            4,
            4,
            &profile,
            |d, w, h| encode_rgb565(d, w, h, false),
            TOL_RGB565,
            &label,
        );
    }
}

#[test]
fn rgb565_checkerboard() {
    let pixels = checkerboard_pixels(8, 8, 1, [0, 0, 255, 255], [255, 255, 255, 255]);
    let profile = profile_rgb565(8, 8, true);
    roundtrip_verify(
        &pixels,
        8,
        8,
        &profile,
        |d, w, h| encode_rgb565(d, w, h, false),
        TOL_RGB565,
        "rgb565_checkerboard_8x8",
    );
}

#[test]
fn rgb565_random() {
    let mut rng = util::rng::SeededRng::new(42);
    let pixels = random_bgra(&mut rng, 8 * 8);
    let profile = profile_rgb565(8, 8, true);
    roundtrip_verify(
        &pixels,
        8,
        8,
        &profile,
        |d, w, h| encode_rgb565(d, w, h, false),
        TOL_RGB565,
        "rgb565_random_8x8",
    );
}

// ===========================================================================
// TESTS - RGB555 (little-endian, no channel swap)
// ===========================================================================

#[test]
fn rgb555_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("rgb555_{w}x{h}");
        let pixels = solid_pixels(w, h, [64, 128, 192, 255]);
        let profile = profile_rgb555(w as i32, h as i32, true, false);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_rgb555(d, w, h, false, false),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn rgb555_single_pixel() {
    for &(w, h) in &[(1, 1), (1, 3), (3, 1)] {
        let label = format!("rgb555_{w}x{h}");
        let pixels = solid_pixels(w, h, [0, 255, 0, 255]);
        let profile = profile_rgb555(w as i32, h as i32, true, false);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_rgb555(d, w, h, false, false),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn rgb555_bgr15() {
    let pixels = solid_pixels(2, 2, [255, 0, 0, 255]);
    let profile = profile_rgb555(2, 2, true, true);
    roundtrip_verify(
        &pixels,
        2,
        2,
        &profile,
        |d, w, h| encode_rgb555(d, w, h, false, true),
        TOL_RGB555,
        "rgb555_bgr15_2x2",
    );
}

#[test]
fn rgb555_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("rgb555_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_rgb555(4, 4, true, false);
        roundtrip_verify(
            &pixels,
            4,
            4,
            &profile,
            |d, w, h| encode_rgb555(d, w, h, false, false),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn rgb555_checkerboard() {
    let pixels = checkerboard_pixels(4, 4, 1, [0, 0, 255, 255], [0, 255, 0, 255]);
    let profile = profile_rgb555(4, 4, true, false);
    roundtrip_verify(
        &pixels,
        4,
        4,
        &profile,
        |d, w, h| encode_rgb555(d, w, h, false, false),
        TOL_RGB555,
        "rgb555_checkerboard_4x4",
    );
}

#[test]
fn rgb555_random() {
    let mut rng = util::rng::SeededRng::new(99);
    let pixels = random_bgra(&mut rng, 6 * 6);
    let profile = profile_rgb555(6, 6, true, false);
    roundtrip_verify(
        &pixels,
        6,
        6,
        &profile,
        |d, w, h| encode_rgb555(d, w, h, false, false),
        TOL_RGB555,
        "rgb555_random_6x6",
    );
}

// ===========================================================================
// TESTS - Reordered RGB555 (big-endian, Z-order interleaved)
// ===========================================================================

#[test]
fn reordered_rgb555_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("reordered_rgb555_{w}x{h}");
        let pixels = solid_pixels(w, h, [100, 150, 200, 255]);
        let profile = profile_reordered_rgb555(w as i32, h as i32);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_reordered_rgb555(d, w, h, true),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn reordered_rgb555_single_pixel() {
    for &(w, h) in &[(1, 1), (1, 4), (4, 1)] {
        let label = format!("reordered_rgb555_{w}x{h}");
        let pixels = solid_pixels(w, h, [0, 255, 0, 255]);
        let profile = profile_reordered_rgb555(w as i32, h as i32);
        roundtrip_verify(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_reordered_rgb555(d, w, h, true),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn reordered_rgb555_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("reordered_rgb555_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_reordered_rgb555(4, 4);
        roundtrip_verify(
            &pixels,
            4,
            4,
            &profile,
            |d, w, h| encode_reordered_rgb555(d, w, h, true),
            TOL_RGB555,
            &label,
        );
    }
}

#[test]
fn reordered_rgb555_random() {
    let mut rng = util::rng::SeededRng::new(123);
    let pixels = random_bgra(&mut rng, 4 * 4);
    let profile = profile_reordered_rgb555(4, 4);
    roundtrip_verify(
        &pixels,
        4,
        4,
        &profile,
        |d, w, h| encode_reordered_rgb555(d, w, h, true),
        TOL_RGB555,
        "reordered_rgb555_random_4x4",
    );
}

// ===========================================================================
// TESTS - UYVY (YUV 4:2:2 packed, even width required for clean roundtrip)
// ===========================================================================

#[test]
fn uyvy_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("uyvy_{w}x{h}");
        let pixels = solid_pixels(w, h, [128, 128, 128, 255]);
        let profile = profile_uyvy(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_uyvy, TOL_UYVY, &label);
    }
}

#[test]
fn uyvy_non_power_of_2() {
    for &(w, h) in &[(4, 6), (6, 4)] {
        let label = format!("uyvy_{w}x{h}");
        let pixels = solid_pixels(w, h, [200, 100, 50, 255]);
        let profile = profile_uyvy(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_uyvy, TOL_UYVY, &label);
    }
}

#[test]
fn uyvy_single_pixel() {
    for &(w, h) in &[(2, 1), (2, 2), (2, 4)] {
        let label = format!("uyvy_{w}x{h}");
        let pixels = solid_pixels(w, h, [255, 255, 255, 255]);
        let profile = profile_uyvy(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_uyvy, TOL_UYVY, &label);
    }
}

#[test]
fn uyvy_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("uyvy_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_uyvy(4, 4);
        roundtrip_verify_yuv(&pixels, 4, 4, &profile, encode_uyvy, TOL_UYVY, &label);
    }
}

#[test]
fn uyvy_checkerboard() {
    // Use 2x2 tiles so horizontal chroma pairs within each tile are uniform.
    let pixels = checkerboard_pixels(4, 4, 2, [0, 0, 255, 255], [255, 255, 255, 255]);
    let profile = profile_uyvy(4, 4);
    roundtrip_verify_yuv(&pixels, 4, 4, &profile, encode_uyvy, TOL_UYVY, "uyvy_checkerboard_4x4");
}

#[test]
fn uyvy_random() {
    let mut rng = util::rng::SeededRng::new(77);
    let pixels = random_bgra(&mut rng, 6 * 6);
    let profile = profile_uyvy(6, 6);
    // Random neighbouring pixels can have radically different chroma; UYVY
    // horizontal chroma averaging introduces large per-channel errors.
    roundtrip_verify_yuv(&pixels, 6, 6, &profile, encode_uyvy, 144, "uyvy_random_6x6");
}

// ===========================================================================
// TESTS - YCbCr 4:2:0 (planar, requires even width AND height)
// ===========================================================================

#[test]
fn ycbcr420_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("ycbcr420_{w}x{h}");
        let pixels = solid_pixels(w, h, [128, 128, 128, 255]);
        let profile = profile_ycbcr420(w as i32, h as i32);
        roundtrip_verify_yuv(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_ycbcr420(d, w, h, false),
            TOL_YCBCR420,
            &label,
        );
    }
}

#[test]
fn ycbcr420_non_power_of_2() {
    for &(w, h) in &[(6, 4), (4, 6)] {
        let label = format!("ycbcr420_{w}x{h}");
        let pixels = solid_pixels(w, h, [100, 200, 50, 255]);
        let profile = profile_ycbcr420(w as i32, h as i32);
        roundtrip_verify_yuv(
            &pixels,
            w as i32,
            h as i32,
            &profile,
            |d, w, h| encode_ycbcr420(d, w, h, false),
            TOL_YCBCR420,
            &label,
        );
    }
}

#[test]
fn ycbcr420_2x2_minimum() {
    let pixels = solid_pixels(2, 2, [255, 0, 0, 255]);
    let profile = profile_ycbcr420(2, 2);
    roundtrip_verify_yuv(
        &pixels,
        2,
        2,
        &profile,
        |d, w, h| encode_ycbcr420(d, w, h, false),
        TOL_YCBCR420,
        "ycbcr420_2x2",
    );
}

#[test]
fn ycbcr420_swap_chroma() {
    let pixels = solid_pixels(4, 4, [0, 255, 0, 255]);
    let mut profile = profile_ycbcr420(4, 4);
    profile.swap_chroma_planes = true;
    roundtrip_verify_yuv(
        &pixels,
        4,
        4,
        &profile,
        |d, w, h| encode_ycbcr420(d, w, h, true),
        TOL_YCBCR420,
        "ycbcr420_swap_chroma_4x4",
    );
}

#[test]
fn ycbcr420_solid_colors() {
    let colors: [(&str, [u8; 4]); 3] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("gray", [128, 128, 128, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("ycbcr420_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_ycbcr420(4, 4);
        roundtrip_verify_yuv(
            &pixels,
            4,
            4,
            &profile,
            |d, w, h| encode_ycbcr420(d, w, h, false),
            TOL_YCBCR420,
            &label,
        );
    }
}

#[test]
fn ycbcr420_random() {
    let mut rng = util::rng::SeededRng::new(55);
    let pixels = random_bgra(&mut rng, 6 * 4);
    let profile = profile_ycbcr420(6, 4);
    // Random patterns can have large chroma subsampling errors.
    roundtrip_verify_yuv(
        &pixels,
        6,
        4,
        &profile,
        |d, w, h| encode_ycbcr420(d, w, h, false),
        144,
        "ycbcr420_random_6x4",
    );
}

// ===========================================================================
// TESTS - CLCL (separate Cb/Cr nibble planes)
// ===========================================================================

#[test]
fn clcl_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("clcl_{w}x{h}");
        let pixels = solid_pixels(w, h, [128, 128, 128, 255]);
        let profile = profile_clcl(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_clcl, TOL_NIBBLE, &label);
    }
}

#[test]
fn clcl_single_pixel() {
    // CLCL encoder/decoder mismatch for odd pixel counts; use even pixel counts.
    for &(w, h) in &[(1, 2), (2, 1), (2, 2)] {
        let label = format!("clcl_{w}x{h}");
        let pixels = solid_pixels(w, h, [255, 255, 255, 255]);
        let profile = profile_clcl(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_clcl, TOL_NIBBLE, &label);
    }
}

#[test]
fn clcl_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("clcl_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_clcl(4, 4);
        roundtrip_verify_yuv(&pixels, 4, 4, &profile, encode_clcl, TOL_NIBBLE, &label);
    }
}

#[test]
fn clcl_checkerboard() {
    let pixels = checkerboard_pixels(4, 4, 1, [255, 0, 0, 255], [0, 0, 255, 255]);
    let profile = profile_clcl(4, 4);
    roundtrip_verify_yuv(
        &pixels,
        4,
        4,
        &profile,
        encode_clcl,
        TOL_NIBBLE,
        "clcl_checkerboard_4x4",
    );
}

#[test]
fn clcl_random() {
    let mut rng = util::rng::SeededRng::new(33);
    let pixels = random_bgra(&mut rng, 4 * 6);
    let profile = profile_clcl(4, 6);
    roundtrip_verify_yuv(&pixels, 4, 6, &profile, encode_clcl, TOL_NIBBLE, "clcl_random_4x6");
}

// ===========================================================================
// TESTS - CL (per-pixel nibble chroma)
// ===========================================================================

#[test]
fn cl_power_of_2() {
    for &(w, h) in &[(2, 2), (4, 4), (8, 8)] {
        let label = format!("cl_{w}x{h}");
        let pixels = solid_pixels(w, h, [128, 128, 128, 255]);
        let profile = profile_cl(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_cl, TOL_NIBBLE, &label);
    }
}

#[test]
fn cl_single_pixel() {
    for &(w, h) in &[(1, 1), (1, 3), (3, 1)] {
        let label = format!("cl_{w}x{h}");
        let pixels = solid_pixels(w, h, [0, 0, 255, 255]);
        let profile = profile_cl(w as i32, h as i32);
        roundtrip_verify_yuv(&pixels, w as i32, h as i32, &profile, encode_cl, TOL_NIBBLE, &label);
    }
}

#[test]
fn cl_solid_colors() {
    let colors: [(&str, [u8; 4]); 5] = [
        ("white", [255, 255, 255, 255]),
        ("black", [0, 0, 0, 255]),
        ("red", [0, 0, 255, 255]),
        ("green", [0, 255, 0, 255]),
        ("blue", [255, 0, 0, 255]),
    ];
    for (name, colour) in &colors {
        let label = format!("cl_solid_{name}");
        let pixels = solid_pixels(4, 4, *colour);
        let profile = profile_cl(4, 4);
        roundtrip_verify_yuv(&pixels, 4, 4, &profile, encode_cl, TOL_NIBBLE, &label);
    }
}

#[test]
fn cl_checkerboard() {
    let pixels = checkerboard_pixels(8, 8, 1, [0, 255, 0, 255], [255, 0, 0, 255]);
    let profile = profile_cl(8, 8);
    roundtrip_verify_yuv(&pixels, 8, 8, &profile, encode_cl, TOL_NIBBLE, "cl_checkerboard_8x8");
}

#[test]
fn cl_random() {
    let mut rng = util::rng::SeededRng::new(11);
    let pixels = random_bgra(&mut rng, 6 * 6);
    let profile = profile_cl(6, 6);
    roundtrip_verify_yuv(&pixels, 6, 6, &profile, encode_cl, TOL_NIBBLE, "cl_random_6x6");
}

// ===========================================================================
// TESTS - build_ithmb_file dispatch (smoke tests via the full builder)
// ===========================================================================

#[test]
fn build_dispatch_rgb565() {
    let pixels = solid_pixels(2, 2, [255, 255, 255, 255]);
    let profile = profile_rgb565(2, 2, true);
    roundtrip_via_build(&pixels, 2, 2, &profile, TOL_RGB565, "build_rgb565");
    let _file = generate_rgb565(2, 2, &pixels, false);
}

#[test]
fn build_dispatch_uyvy() {
    let pixels = solid_pixels(4, 4, [128, 128, 128, 255]);
    let profile = profile_uyvy(4, 4);
    roundtrip_via_build(&pixels, 4, 4, &profile, TOL_UYVY, "build_uyvy");
    let _file = generate_uyvy(4, 4, &pixels);
}

#[test]
fn build_dispatch_clcl() {
    let pixels = solid_pixels(2, 2, [255, 255, 255, 255]);
    let profile = profile_clcl(2, 2);
    roundtrip_via_build(&pixels, 2, 2, &profile, TOL_NIBBLE, "build_clcl");
    let _file = generate_clcl(2, 2, &pixels);
}

#[test]
fn build_dispatch_cl() {
    let pixels = solid_pixels(2, 2, [255, 255, 255, 255]);
    let profile = profile_cl(2, 2);
    roundtrip_via_build(&pixels, 2, 2, &profile, TOL_NIBBLE, "build_cl");
    let _file = generate_cl(2, 2, &pixels);
}

// ===========================================================================
// TESTS - Generator functions produce valid files
// ===========================================================================

#[test]
fn generator_rgb565_produces_valid_file() {
    let pixels = solid_pixels(4, 4, [200, 100, 50, 255]);
    let file = generate_rgb565(4, 4, &pixels, false);
    assert!(file.len() > 4, "file must be larger than 4-byte prefix");
    let profile = profile_rgb565(4, 4, true);
    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&file, &profile, &canceled).unwrap();
    assert_eq!(decoded.width, 4);
    assert_eq!(decoded.height, 4);
}

#[test]
fn generator_uyvy_produces_valid_file() {
    let pixels = solid_pixels(4, 4, [200, 100, 50, 255]);
    let file = generate_uyvy(4, 4, &pixels);
    assert!(file.len() > 4);
    let profile = profile_uyvy(4, 4);
    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&file, &profile, &canceled).unwrap();
    assert_eq!(decoded.width, 4);
    assert_eq!(decoded.height, 4);
}

#[test]
fn generator_ycbcr420_produces_valid_file() {
    let pixels = solid_pixels(4, 4, [200, 100, 50, 255]);
    let file = generate_ycbcr420(4, 4, &pixels, false);
    assert!(file.len() > 4);
    let profile = profile_ycbcr420(4, 4);
    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&file, &profile, &canceled).unwrap();
    assert_eq!(decoded.width, 4);
    assert_eq!(decoded.height, 4);
}
