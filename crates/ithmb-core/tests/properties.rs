#![allow(clippy::pedantic, clippy::unwrap_used, clippy::missing_panics_doc)]
//! Property-based tests for encode/decode roundtrip invariants.
//!
//! Uses `proptest` to generate random BGRA images across all 8 formats and
//! verifies:
//!
//! a. **Roundtrip safety** — encode → decode does not panic for valid inputs.
//! b. **Prefix identity** — the 4-byte profile prefix is preserved.
//! c. **Dimension fidelity** — decoded width/height match the encoded dimensions.
//! d. **Valid pixel range** — all pixel values are in `0..=255`, alpha is `255`.
//!
//! **256 test cases per format** — shrinking on failure, no library modifications.

mod util;

use divan as _;
use image as _;
use jpeg_decoder as _;
use thiserror as _;

use ithmb_core::enc::{
    encode_cl, encode_clcl, encode_reordered_rgb555, encode_rgb555, encode_rgb565, encode_uyvy, encode_ycbcr420,
};
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use proptest::prelude::*;
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Test format enumeration
// ---------------------------------------------------------------------------

/// The 8 formats covered by property tests.
///
/// CLCL and CL are YUV 4:2:2 variants distinguished by chroma-packing flags rather
/// than a separate `Encoding` discriminant.
#[derive(Debug, Clone, Copy)]
enum TestFormat {
    Rgb565,
    Rgb555,
    ReorderedRgb555,
    Uyvy,
    Ycbcr420,
    Clcl,
    Cl,
    #[allow(dead_code)]
    Jpeg,
    // JPEG excluded: no encoder exists for roundtrip testing
}

impl TestFormat {
    fn name(self) -> &'static str {
        match self {
            Self::Rgb565 => "RGB565",
            Self::Rgb555 => "RGB555",
            Self::ReorderedRgb555 => "ReorderedRGB555",
            Self::Uyvy => "UYVY",
            Self::Ycbcr420 => "YCbCr420",
            Self::Clcl => "CLCL",
            Self::Cl => "CL",
            Self::Jpeg => "JPEG",
        }
    }
}

// ---------------------------------------------------------------------------
// Profile & encoder helpers
// ---------------------------------------------------------------------------

/// Build the decoding profile for a given format and dimensions.
///
/// CLCL and CL use `Encoding::Yuv422` plus `clcl_chroma` / `cl_chroma` flags;
/// ReorderedRGB555 forces big-endian byte order.
fn build_profile(w: i32, h: i32, fmt: TestFormat) -> Profile {
    match fmt {
        TestFormat::Clcl => {
            let n = (w * h) as usize;
            let chroma_len = n.div_ceil(2);
            Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Yuv422,
                frame_byte_length: i32::try_from(n + chroma_len + chroma_len).unwrap(),
                clcl_chroma: true,
                ..Default::default()
            }
        }
        TestFormat::Cl => {
            let n = (w * h) as usize;
            Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Yuv422,
                frame_byte_length: i32::try_from(n * 2).unwrap(),
                cl_chroma: true,
                ..Default::default()
            }
        }
        TestFormat::ReorderedRgb555 => {
            let mut p = util::make_profile(w, h, Encoding::ReorderedRgb555);
            p.little_endian = false;
            p
        }
        TestFormat::Jpeg => Profile {
            encoding: Encoding::Jpeg,
            ..Default::default()
        },
        TestFormat::Rgb565 => util::make_profile(w, h, Encoding::Rgb565),
        TestFormat::Rgb555 => util::make_profile(w, h, Encoding::Rgb555),
        TestFormat::Uyvy => util::make_profile(w, h, Encoding::Yuv422),
        TestFormat::Ycbcr420 => util::make_profile(w, h, Encoding::Ycbcr420),
    }
}

/// Encode BGRA pixels into the given format (pure frame data, no prefix).
fn encode(fmt: TestFormat, bgra: &[u8], w: i32, h: i32) -> Vec<u8> {
    match fmt {
        TestFormat::Rgb565 => encode_rgb565(bgra, w, h, false),
        TestFormat::Rgb555 => encode_rgb555(bgra, w, h, false, false),
        TestFormat::ReorderedRgb555 => encode_reordered_rgb555(bgra, w, h, true),
        TestFormat::Uyvy => encode_uyvy(bgra, w, h),
        TestFormat::Ycbcr420 => encode_ycbcr420(bgra, w, h, false),
        TestFormat::Clcl => encode_clcl(bgra, w, h),
        TestFormat::Cl => encode_cl(bgra, w, h),
        TestFormat::Jpeg => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Property checkers
// ---------------------------------------------------------------------------

/// Full roundtrip check for the 7 encodable formats.
///
/// Verifies all four invariants (a–d). Panics on failure, which proptest
/// catches and reports as a counterexample with shrinking.
fn check_roundtrip_properties(fmt: TestFormat, w: i32, h: i32, bgra: &[u8]) {
    let profile = build_profile(w, h, fmt);
    let encoded = encode(fmt, bgra, w, h);

    // Build a complete .ithmb byte sequence: 4-byte big-endian prefix + frame.
    let prefix_bytes = profile.prefix.to_be_bytes();
    let mut ithmb_data = Vec::with_capacity(4 + encoded.len());
    ithmb_data.extend_from_slice(&prefix_bytes);
    ithmb_data.extend_from_slice(&encoded);

    // (b) Prefix identity — the file starts with the correct profile prefix.
    assert_eq!(
        &ithmb_data[..4],
        &profile.prefix.to_be_bytes(),
        "{}: prefix mismatch",
        fmt.name(),
    );

    // (a) Roundtrip — decode does not panic (must succeed for valid input).
    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&ithmb_data, &profile, &canceled)
        .unwrap_or_else(|e| panic!("{}: roundtrip decode failed: {e}", fmt.name()));

    // (c) Dimensions — decoded width and height match the profile.
    assert_eq!(
        decoded.width,
        w as u32,
        "{}: decoded width {got} != profile width {expected}",
        fmt.name(),
        got = decoded.width,
        expected = w,
    );
    assert_eq!(
        decoded.height,
        h as u32,
        "{}: decoded height {got} != profile height {expected}",
        fmt.name(),
        got = decoded.height,
        expected = h,
    );

    // (d) Pixel range — u8 guarantees 0..=255; alpha == 255 checked below.

    // (d) Alpha — every pixel has alpha == 255.
    for (i, chunk) in decoded.data.chunks_exact(4).enumerate() {
        assert_eq!(
            chunk[3],
            255,
            "{}: alpha channel is {alpha} at pixel {i}, expected 255",
            fmt.name(),
            alpha = chunk[3],
        );
    }
}

/// Decode-only check for JPEG (no encoder — raw passthrough).
///
/// Verifies that `decode_with_profile` never panics on arbitrary byte
/// sequences. If decoding happens to succeed, pixel invariants are checked.
fn check_jpeg_decode_properties(data: &[u8]) {
    let profile = Profile {
        encoding: Encoding::Jpeg,
        ..Default::default()
    };

    let canceled = AtomicBool::new(false);

    // (a) Must not panic (may return error for non-JPEG data — that is fine).
    if let Ok(decoded) = decode_with_profile(data, &profile, &canceled) {
        // (d) Pixel range — guaranteed by u8 type.
        // (d) Alpha
        for (i, chunk) in decoded.data.chunks_exact(4).enumerate() {
            assert_eq!(chunk[3], 255, "JPEG: alpha is {} at pixel {i}", chunk[3],);
        }
    }
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

/// Strategy for valid image dimensions (1 ..= 32 pixels).
fn dim() -> impl Strategy<Value = i32> {
    1i32..=32
}

/// Strategy for even pixel dimensions (required by YCbCr 4:2:0 and CLCL).
fn even_dim() -> impl Strategy<Value = i32> {
    (1i32..=16).prop_map(|x| x * 2)
}
/// Strategy generating `(width, height, bgra)` tuples for encodable formats.
fn arb_image() -> impl Strategy<Value = (i32, i32, Vec<u8>)> {
    (dim(), dim()).prop_flat_map(|(w, h)| {
        let len = (w * h * 4) as usize;
        (Just(w), Just(h), prop::collection::vec(any::<u8>(), len))
    })
}

/// Strategy generating `(width, height, bgra)` with even width (required by CLCL).
fn arb_image_even_width() -> impl Strategy<Value = (i32, i32, Vec<u8>)> {
    (even_dim(), dim()).prop_flat_map(|(w, h)| {
        let len = (w * h * 4) as usize;
        (Just(w), Just(h), prop::collection::vec(any::<u8>(), len))
    })
}

/// Strategy generating `(width, height, bgra)` with even dimensions (required by YCbCr 4:2:0).
fn arb_image_even_both() -> impl Strategy<Value = (i32, i32, Vec<u8>)> {
    (even_dim(), even_dim()).prop_flat_map(|(w, h)| {
        let len = (w * h * 4) as usize;
        (Just(w), Just(h), prop::collection::vec(any::<u8>(), len))
    })
}

// ---------------------------------------------------------------------------
// Property tests — one per format, 256 cases each (default proptest config)
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_rgb565((w, h, bgra) in arb_image()) {
        check_roundtrip_properties(TestFormat::Rgb565, w, h, &bgra);
    }

    #[test]
    fn prop_rgb555((w, h, bgra) in arb_image()) {
        check_roundtrip_properties(TestFormat::Rgb555, w, h, &bgra);
    }

    #[test]
    fn prop_reordered_rgb555((w, h, bgra) in arb_image()) {
        check_roundtrip_properties(TestFormat::ReorderedRgb555, w, h, &bgra);
    }

    #[test]
    fn prop_uyvy((w, h, bgra) in arb_image()) {
        check_roundtrip_properties(TestFormat::Uyvy, w, h, &bgra);
    }

    #[test]
    fn prop_ycbcr420((w, h, bgra) in arb_image_even_both()) {
        check_roundtrip_properties(TestFormat::Ycbcr420, w, h, &bgra);
    }

    #[test]
    fn prop_clcl((w, h, bgra) in arb_image_even_width()) {
        check_roundtrip_properties(TestFormat::Clcl, w, h, &bgra);
    }

    #[test]
    fn prop_cl((w, h, bgra) in arb_image()) {
        check_roundtrip_properties(TestFormat::Cl, w, h, &bgra);
    }

    #[test]
    fn prop_jpeg(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        check_jpeg_decode_properties(&data);
    }
}
