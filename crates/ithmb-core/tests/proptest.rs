#![allow(clippy::pedantic, clippy::unwrap_used, clippy::missing_panics_doc)]
//! Property-based tests for the full encode → file → decode pipeline and the
//! profile/cache serialization roundtrips.
//!
//! Complements `properties.rs` (which roundtrips raw frame data through
//! `decode_with_profile`) by exercising:
//!
//! a. **Full-pipeline roundtrip** — `build_ithmb_file` (prefix + interlace +
//!    padding) → `decode_with_profile`, for all 7 synthetic encoders. For the
//!    lossless-within-MSB-replication formats (rgb565, rgb555, reordered)
//!    pixel identity is asserted within the 5/6/5 quantization bound; the
//!    lossy YUV formats assert dimension + alpha invariants only (matching the
//!    `properties.rs` convention).
//! b. **Interlaced / padded variants** — the codec paths that reorder rows and
//!    tolerate trailing padding.
//! c. **Cache entry serialization** (`cache` feature) — a decode miss stores a
//!    serialized entry; a hit must deserialize it back to an identical image.
//! d. **Profile serialize → parse roundtrip** — a test-side serializer emits
//!    every field the production parser understands; parsing must reconstruct
//!    the profile exactly. (The crate has no production serializer today, so
//!    this pins parser fidelity against the documented schema.)
//!
//! **256 test cases per property** — shrinking on failure, no library changes.

mod util;

use divan as _;
use image as _;
use jpeg_decoder as _;
#[cfg(feature = "logging")]
use log as _;
#[cfg(feature = "cache")]
use lru as _;
use thiserror as _;

use ithmb_core::enc::build_ithmb_file;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::profile_parser::parse_profiles_json;
use proptest::prelude::*;
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Test format enumeration
// ---------------------------------------------------------------------------

/// The 7 encodable formats covered by the full-pipeline property.
#[derive(Debug, Clone, Copy)]
enum TestFormat {
    Rgb565,
    Rgb555,
    ReorderedRgb555,
    Uyvy,
    Ycbcr420,
    Clcl,
    Cl,
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
        }
    }
}

// ---------------------------------------------------------------------------
// Profile & pipeline helpers
// ---------------------------------------------------------------------------

/// Build the decoding profile for a format and dimensions.
///
/// CLCL / CL are `Encoding::Yuv422` plus a chroma-packing flag; ReorderedRGB555
/// forces big-endian byte order.
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
        TestFormat::Rgb565 => util::make_profile(w, h, Encoding::Rgb565),
        TestFormat::Rgb555 => util::make_profile(w, h, Encoding::Rgb555),
        TestFormat::Uyvy => util::make_profile(w, h, Encoding::Yuv422),
        TestFormat::Ycbcr420 => util::make_profile(w, h, Encoding::Ycbcr420),
    }
}

/// Full `build_ithmb_file` → `decode_with_profile` roundtrip check.
///
/// `tolerance == Some(t)` additionally asserts pixel identity within `t` per
/// channel (safe for the MSB-replicating RGB formats); `None` checks only the
/// dimension + alpha invariants (lossy YUV formats).
fn check_full_pipeline(fmt: TestFormat, w: i32, h: i32, bgra: &[u8], tolerance: Option<u8>) {
    let profile = build_profile(w, h, fmt);
    let file = build_ithmb_file(bgra, w, h, &profile);

    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&file, &profile, &canceled)
        .unwrap_or_else(|e| panic!("{}: full-pipeline decode failed: {e}", fmt.name()));

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
    for (i, chunk) in decoded.data.chunks_exact(4).enumerate() {
        assert_eq!(
            chunk[3],
            255,
            "{}: alpha channel is {alpha} at pixel {i}, expected 255",
            fmt.name(),
            alpha = chunk[3],
        );
    }

    if let Some(tol) = tolerance {
        util::assert_bgra_tolerant(&decoded.data, bgra, tol);
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

/// Strategy with even width (required by CLCL and the interlaced UYVY path).
fn arb_image_even_width() -> impl Strategy<Value = (i32, i32, Vec<u8>)> {
    (even_dim(), dim()).prop_flat_map(|(w, h)| {
        let len = (w * h * 4) as usize;
        (Just(w), Just(h), prop::collection::vec(any::<u8>(), len))
    })
}

/// Strategy with even width and height (required by YCbCr 4:2:0).
fn arb_image_even_both() -> impl Strategy<Value = (i32, i32, Vec<u8>)> {
    (even_dim(), even_dim()).prop_flat_map(|(w, h)| {
        let len = (w * h * 4) as usize;
        (Just(w), Just(h), prop::collection::vec(any::<u8>(), len))
    })
}

// ---------------------------------------------------------------------------
// Profile JSON helpers (test-side serializer for the parse-roundtrip property)
// ---------------------------------------------------------------------------

/// Canonical lowercase encoding name, as the production parser lowercases
/// input before matching.
fn encoding_name(e: Encoding) -> &'static str {
    match e {
        Encoding::Rgb565 => "rgb565",
        Encoding::Rgb555 => "rgb555",
        Encoding::ReorderedRgb555 => "reorderedrgb555",
        Encoding::Yuv422 => "yuv422",
        Encoding::Ycbcr420 => "ycbcr420",
        Encoding::Jpeg => "jpeg",
        // `Encoding` is `#[non_exhaustive]`; unreachable today.
        _ => unreachable!("unknown encoding variant"),
    }
}

/// Strategy over all encoding variants.
fn arb_encoding() -> impl Strategy<Value = Encoding> {
    prop_oneof![
        Just(Encoding::Rgb565),
        Just(Encoding::Rgb555),
        Just(Encoding::ReorderedRgb555),
        Just(Encoding::Yuv422),
        Just(Encoding::Ycbcr420),
        Just(Encoding::Jpeg),
    ]
}

/// Serialize a `Profile` to the exact profiles.json schema the production
/// parser understands (every field `set_field` can assign).
fn profile_to_json(p: &Profile) -> String {
    let fallback = match &p.fallback_encodings {
        None => "null".to_string(),
        Some(encs) => {
            let items: Vec<String> = encs.iter().map(|e| format!("\"{}\"", encoding_name(*e))).collect();
            format!("[{}]", items.join(","))
        }
    };
    format!(
        "[{{\"prefix\":{},\"width\":{},\"height\":{},\"encoding\":\"{}\",\"frame_byte_length\":{},\
          \"swaps_dimensions\":{},\"little_endian\":{},\"is_padded\":{},\"is_interlaced\":{},\
          \"clcl_chroma\":{},\"swap_chroma_planes\":{},\"cl_chroma\":{},\"swap_rgb_channels\":{},\
          \"rotation\":{},\"crop_x\":{},\"crop_y\":{},\"crop_width\":{},\"crop_height\":{},\
          \"slot_size\":{},\"use_mhni_dimensions\":{},\"fallback_encodings\":{}}}]",
        p.prefix,
        p.width,
        p.height,
        encoding_name(p.encoding),
        p.frame_byte_length,
        p.swaps_dimensions,
        p.little_endian,
        p.is_padded,
        p.is_interlaced,
        p.clcl_chroma,
        p.swap_chroma_planes,
        p.cl_chroma,
        p.swap_rgb_channels,
        p.rotation,
        p.crop_x,
        p.crop_y,
        p.crop_width,
        p.crop_height,
        p.slot_size,
        p.use_mhni_dimensions,
        fallback,
    )
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn prop_full_pipeline_rgb565((w, h, bgra) in arb_image()) {
        check_full_pipeline(TestFormat::Rgb565, w, h, &bgra, Some(8));
    }

    #[test]
    fn prop_full_pipeline_rgb555((w, h, bgra) in arb_image()) {
        check_full_pipeline(TestFormat::Rgb555, w, h, &bgra, Some(8));
    }

    #[test]
    fn prop_full_pipeline_reordered_rgb555((w, h, bgra) in arb_image()) {
        check_full_pipeline(TestFormat::ReorderedRgb555, w, h, &bgra, Some(8));
    }

    #[test]
    fn prop_full_pipeline_uyvy((w, h, bgra) in arb_image()) {
        check_full_pipeline(TestFormat::Uyvy, w, h, &bgra, None);
    }

    #[test]
    fn prop_full_pipeline_ycbcr420((w, h, bgra) in arb_image_even_both()) {
        check_full_pipeline(TestFormat::Ycbcr420, w, h, &bgra, None);
    }

    #[test]
    fn prop_full_pipeline_clcl((w, h, bgra) in arb_image_even_width()) {
        check_full_pipeline(TestFormat::Clcl, w, h, &bgra, None);
    }

    #[test]
    fn prop_full_pipeline_cl((w, h, bgra) in arb_image()) {
        check_full_pipeline(TestFormat::Cl, w, h, &bgra, None);
    }

    /// Interlaced UYVY — `build_ithmb_file` interlaces field rows; the decoder
    /// must reverse it. Row order changes, so only dims + alpha are asserted.
    #[test]
    fn prop_interlaced_uyvy((w, h, bgra) in arb_image_even_both()) {
        let profile = Profile {
            is_interlaced: true,
            ..util::make_profile(w, h, Encoding::Yuv422)
        };
        let file = build_ithmb_file(&bgra, w, h, &profile);
        let canceled = AtomicBool::new(false);
        let decoded = decode_with_profile(&file, &profile, &canceled)
            .unwrap_or_else(|e| panic!("interlaced UYVY decode failed: {e}"));
        assert_eq!(decoded.width, w as u32);
        assert_eq!(decoded.height, h as u32);
        assert!(decoded.data.chunks_exact(4).all(|c| c[3] == 255), "interlaced alpha != 255");
    }

    /// Padded RGB565 — a frame with trailing zero padding beyond its payload
    /// must still decode to the correct dimensions without error. Pixel
    /// fidelity is NOT asserted here: the rgb565 decoder derives its row
    /// stride from the input length (`src.len() / h`, a data-driven stride
    /// for the F1061-style row-padded format), so arbitrary trailing padding
    /// is only pixel-safe when it is shorter than one row. The crate's own
    /// `row_stride_is_data_driven` test likewise asserts dims only.
    #[test]
    fn prop_padded_rgb565((w, h, bgra) in arb_image()) {
        let profile = Profile {
            is_padded: true,
            frame_byte_length: w * h * 2 + 64,
            slot_size: w * h * 2 + 64,
            ..util::make_profile(w, h, Encoding::Rgb565)
        };
        let file = build_ithmb_file(&bgra, w, h, &profile);
        let canceled = AtomicBool::new(false);
        let decoded = decode_with_profile(&file, &profile, &canceled)
            .unwrap_or_else(|e| panic!("padded RGB565 decode failed: {e}"));
        assert_eq!(decoded.width, w as u32);
        assert_eq!(decoded.height, h as u32);
        assert!(decoded.data.chunks_exact(4).all(|c| c[3] == 255), "padded alpha != 255");
    }

    /// Profile serialize → parse roundtrip: any profile expressible in the
    /// profiles.json schema must survive the parser unchanged.
    #[test]
    fn prop_profile_serialize_parse_roundtrip(
        prefix in any::<i32>(),
        width in any::<i32>(),
        height in any::<i32>(),
        encoding in arb_encoding(),
        frame_byte_length in any::<i32>(),
        swaps_dimensions in any::<bool>(),
        little_endian in any::<bool>(),
        is_padded in any::<bool>(),
        is_interlaced in any::<bool>(),
        clcl_chroma in any::<bool>(),
        swap_chroma_planes in any::<bool>(),
        cl_chroma in any::<bool>(),
        swap_rgb_channels in any::<bool>(),
        rotation in any::<i32>(),
        crop_x in any::<i32>(),
        crop_y in any::<i32>(),
        crop_width in any::<i32>(),
        crop_height in any::<i32>(),
        slot_size in any::<i32>(),
        use_mhni_dimensions in any::<bool>(),
        fallback in prop::option::of(prop::collection::vec(arb_encoding(), 0..4)),
    ) {
        let profile = Profile {
            prefix,
            width,
            height,
            encoding,
            frame_byte_length,
            swaps_dimensions,
            little_endian,
            is_padded,
            is_interlaced,
            clcl_chroma,
            swap_chroma_planes,
            cl_chroma,
            swap_rgb_channels,
            rotation,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            slot_size,
            use_mhni_dimensions,
            fallback_encodings: fallback,
        };
        let json = profile_to_json(&profile);
        let parsed = parse_profiles_json(&json)
            .expect("test-side serializer output must roundtrip through the parser");
        assert_eq!(parsed.as_slice(), &[profile], "parse(serialize(p)) != p");
    }
}

// ---------------------------------------------------------------------------
// Cache entry serialization roundtrip (cache feature)
// ---------------------------------------------------------------------------

/// Property version of the cache store→retrieve roundtrip: a miss serializes
/// the decoded image into the LRU; a hit must deserialize it back to the
/// identical image. Runs only under `--features cache` (or `--all-features`).
#[cfg(feature = "cache")]
mod cache_proptests {
    use super::*;
    use ithmb_core::cache::CachedDecoder;

    proptest! {
        #[test]
        fn prop_cache_entry_serialization_roundtrip((w, h, bgra) in arb_image()) {
            let profile = build_profile(w, h, TestFormat::Rgb565);
            let file = build_ithmb_file(&bgra, w, h, &profile);
            let decoder = CachedDecoder::new();
            let canceled = AtomicBool::new(false);

            // Miss: decodes through the pipeline and stores a serialized entry.
            let miss = decoder
                .decode_with_cache(&profile, &file, &canceled)
                .expect("cache miss decode should succeed");
            assert_eq!(miss.width, w as u32);
            assert_eq!(miss.height, h as u32);
            assert_eq!(decoder.len(), 1);

            // Hit: the stored entry must deserialize to the identical image.
            let hit = decoder
                .decode_with_cache(&profile, &file, &canceled)
                .expect("cache hit should succeed");
            assert_eq!(hit, miss, "cache serialization roundtrip must be identity");
            assert_eq!(decoder.len(), 1, "a hit must not add a second entry");
        }
    }
}
