#![allow(dead_code)]
//! Shared test utilities for ithmb-core integration tests.
//!
//! Provides profile builders, roundtrip helpers, pattern generators, and
//! tolerance assertions shared across multiple test files.
//!
//! Import via `mod util;` and use as `util::make_profile(...)`, etc.

pub mod rng;

use ithmb_core::DecodedImage;
use ithmb_core::enc::encode_bgra;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Profile builder
// ---------------------------------------------------------------------------

/// Build a minimal profile for the given dimensions and encoding.
///
/// The prefix defaults to `9999` (a dummy value not in the real profile DB).
/// Frame byte length is computed correctly for each encoding:
///
/// | Encoding              | Formula                           |
/// |-----------------------|-----------------------------------|
/// | `Rgb565` / `Rgb555`       | `w × h × 2`                      |
/// | `ReorderedRgb555`        | `w × h × 2`                      |
/// | Yuv422                | `ceil(w / 2) × h × 4`            |
/// | Ycbcr420              | `w × h + ceil(w/2) × ceil(h/2) × 2` |
/// | Jpeg                  | `0`                               |
///
/// Fields such as `little_endian`, `clcl_chroma`, `cl_chroma`, etc. are left
/// at [`Profile`] defaults -- override them on the returned value when needed.
#[must_use]
pub fn make_profile(w: i32, h: i32, encoding: Encoding) -> Profile {
    let wu = w.unsigned_abs() as usize;
    let hu = h.unsigned_abs() as usize;
    let frame_byte_length = match encoding {
        Encoding::Rgb565 | Encoding::Rgb555 | Encoding::ReorderedRgb555 => w * h * 2,
        Encoding::Yuv422 => {
            let pairs = wu.div_ceil(2) * hu;
            i32::try_from(pairs).unwrap() * 4
        }
        Encoding::Ycbcr420 => {
            let uv_w = wu.div_ceil(2);
            let uv_h = hu.div_ceil(2);
            i32::try_from(wu * hu + uv_w * uv_h * 2).unwrap()
        }
        Encoding::Jpeg => 0,
    };
    Profile {
        prefix: 9999,
        width: w,
        height: h,
        encoding,
        frame_byte_length,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Roundtrip helper
// ---------------------------------------------------------------------------

/// Encode BGRA pixels, prepend the 4-byte profile prefix, decode, and return
/// the decoded image.
///
/// This is a convenience wrapper that creates a default profile via
/// [`make_profile`], encodes via [`encode_bgra`], and decodes via
/// [`decode_with_profile`]. For tests that need custom profile fields (e.g.
/// `little_endian`, `clcl_chroma`), construct the profile directly and call
/// [`encode_bgra`] / [`decode_with_profile`] yourself.
///
/// # Panics
///
/// Panics if encoding or decoding fails.
#[must_use]
pub fn roundtrip_encode_decode(bgra: &[u8], w: i32, h: i32, encoding: Encoding) -> DecodedImage {
    let profile = make_profile(w, h, encoding);
    let encoded = encode_bgra(bgra, w, h, &profile);
    let prefix_bytes = profile.prefix.unsigned_abs().to_be_bytes();
    let mut with_prefix = Vec::with_capacity(4 + encoded.len());
    with_prefix.extend_from_slice(&prefix_bytes);
    with_prefix.extend_from_slice(&encoded);
    let canceled = AtomicBool::new(false);
    decode_with_profile(&with_prefix, &profile, &canceled).expect("roundtrip encode-decode should succeed")
}

// ---------------------------------------------------------------------------
// Pattern generators
// ---------------------------------------------------------------------------

/// Generate a checkerboard pattern in BGRA format.
///
/// Each cell is 1x1 pixel, alternating between black ([0, 0, 0, 255]) and
/// white ([255, 255, 255, 255]).
#[must_use]
pub fn make_bgra_checkerboard(w: u32, h: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = if (x + y) % 2 == 0 { 255u8 } else { 0u8 };
            pixels.push(v); // B
            pixels.push(v); // G
            pixels.push(v); // R
            pixels.push(255); // A
        }
    }
    pixels
}

// ---------------------------------------------------------------------------
// Tolerance assertion
// ---------------------------------------------------------------------------

/// Assert two BGRA buffers match within a per-channel tolerance.
///
/// # Panics
///
/// Panics if:
/// - The buffers have different lengths
/// - Any channel delta exceeds `tolerance`
/// - Any alpha value is not 255
pub fn assert_bgra_tolerant(actual: &[u8], expected: &[u8], tolerance: u8) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "length mismatch: {} vs {}",
        actual.len(),
        expected.len()
    );
    for (i, (a, e)) in actual.chunks_exact(4).zip(expected.chunks_exact(4)).enumerate() {
        let b_diff = (i16::from(a[0]) - i16::from(e[0])).unsigned_abs();
        let g_diff = (i16::from(a[1]) - i16::from(e[1])).unsigned_abs();
        let r_diff = (i16::from(a[2]) - i16::from(e[2])).unsigned_abs();
        assert!(
            b_diff <= u16::from(tolerance),
            "B pixel {i}: got {expected}, expected {got}",
            expected = e[0],
            got = a[0]
        );
        assert!(
            g_diff <= u16::from(tolerance),
            "G pixel {i}: got {expected}, expected {got}",
            expected = e[1],
            got = a[1]
        );
        assert!(
            r_diff <= u16::from(tolerance),
            "R pixel {i}: got {expected}, expected {got}",
            expected = e[2],
            got = a[2]
        );
        assert_eq!(a[3], 255, "alpha pixel {i}");
    }
}
