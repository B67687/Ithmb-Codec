//! Integration tests for the ratified divergence-catalog resolutions
//! (`docs/divergence-catalog.md`, items U1–U12).
//!
//! Covers the code-change items:
//! * U1 — profile 1044 disabled (53 active profiles; 1044 unreachable).
//! * U2 — Nano 7G alternates resolve by frame size in `decode_ithmb`.
//! * U5/U6 — CL / CLCL planar-layout PIN tests (lock the ratified Rust layout
//!   so a future fix cannot silently swap it).
//! * U8 — encoder honors `swaps_dimensions` (built-in profile 1020).
//! * U9 — reordered RGB555 encoder honors `little_endian`; byte-exact
//!   roundtrip for both endiannesses.

#![allow(clippy::pedantic, clippy::unwrap_used)]

use divan as _;
use image as _;
use ithmb_core::enc::*;
use ithmb_core::pipeline::{decode_ithmb, decode_with_profile};
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::profile_db::ProfileDb;
use jpeg_decoder as _;
#[cfg(feature = "logging")]
use log as _;
#[cfg(feature = "cache")]
use lru as _;
use proptest as _;
use std::sync::atomic::AtomicBool;
use thiserror as _;

// ---------------------------------------------------------------------------
// U1 — profile 1044 disabled
// ---------------------------------------------------------------------------

#[test]
fn u1_profile_1044_disabled() {
    let db = ProfileDb::load_builtin().unwrap();
    assert_eq!(db.len(), 53, "profile 1044 must be disabled (iOpenPod #81)");
    assert!(db.get(1044).is_none(), "profile 1044 must not be active");
}

// ---------------------------------------------------------------------------
// U2 — Nano 7G alternates (1013 → 50×50, 1015 → 58×58, 1016 → 57×57)
// ---------------------------------------------------------------------------

/// A Nano 7G cover-art frame is a small RGB565 payload under a prefix whose
/// global profile is much larger. `decode_ithmb` must resolve to the alternate
/// by frame size and decode to the alternate's dimensions.
#[test]
fn u2_nano_7g_alternates_decode_via_prefix() {
    for (prefix, w, h) in [(1013i32, 50, 50), (1015, 58, 58), (1016, 57, 57)] {
        let frame_len = usize::try_from(w * h * 2).unwrap();
        let mut src = Vec::with_capacity(4 + frame_len);
        src.extend_from_slice(&prefix.to_be_bytes());
        src.resize(4 + frame_len, 0);
        let img = decode_ithmb(&src, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, u32::try_from(w).unwrap(), "prefix {prefix} width");
        assert_eq!(img.height, u32::try_from(h).unwrap(), "prefix {prefix} height");
        assert_eq!(img.data.len(), frame_len * 2, "prefix {prefix} pixel count");
    }
}

// ---------------------------------------------------------------------------
// U5 — CL planar-layout PIN test
// ---------------------------------------------------------------------------

/// Rust-ratified planar CL layout: `[Y0..Yn][CbCr0..CbCrn]`, chroma byte is
/// `(Cr << 4) | Cb` (high nibble = Cr). Two pixels with distinct chroma make a
/// layout swap observable.
#[test]
fn u5_cl_planar_layout_pin() {
    let profile = Profile {
        prefix: 0x0000_0F05,
        width: 2,
        height: 1,
        encoding: Encoding::Yuv422,
        frame_byte_length: 4,
        cl_chroma: true,
        ..Default::default()
    };
    // px0: Y=100, Cb nibble=5, Cr nibble=10 → chroma byte (10<<4)|5 = 0xA5
    // px1: Y=200, Cb nibble=12, Cr nibble=3 → chroma byte (3<<4)|12 = 0x3C
    let mut src = profile.prefix.to_be_bytes().to_vec();
    src.extend_from_slice(&[100, 200, 0xA5, 0x3C]);

    let img = decode_with_profile(&src, &profile, &AtomicBool::new(false)).unwrap();
    let e0 = ithmb_core::yuv::yuv_to_bgra(100, 5 << 4, 10 << 4);
    let e1 = ithmb_core::yuv::yuv_to_bgra(200, 12 << 4, 3 << 4);
    assert_eq!(&img.data[0..4], &e0, "px0 CL planar layout");
    assert_eq!(&img.data[4..8], &e1, "px1 CL planar layout");
}

// ---------------------------------------------------------------------------
// U6 — CLCL planar-layout PIN test
// ---------------------------------------------------------------------------

/// Rust-ratified planar CLCL layout: `[Y0..Yn][Cb bytes][Cr bytes]`, 2 pixels
/// per byte, odd pixel in the high nibble. Distinct per-pixel chroma makes a
/// layout swap observable.
#[test]
fn u6_clcl_planar_layout_pin() {
    let profile = Profile {
        prefix: 0x0000_0F06,
        width: 2,
        height: 1,
        encoding: Encoding::Yuv422,
        frame_byte_length: 4,
        clcl_chroma: true,
        ..Default::default()
    };
    // px0: Y=100, Cb=5, Cr=10 ; px1: Y=200, Cb=12, Cr=3
    // Cb byte = (12<<4)|5 = 0xC5 ; Cr byte = (3<<4)|10 = 0x3A
    let mut src = profile.prefix.to_be_bytes().to_vec();
    src.extend_from_slice(&[100, 200, 0xC5, 0x3A]);

    let img = decode_with_profile(&src, &profile, &AtomicBool::new(false)).unwrap();
    let e0 = ithmb_core::yuv::yuv_to_bgra(100, 5 << 4, 10 << 4);
    let e1 = ithmb_core::yuv::yuv_to_bgra(200, 12 << 4, 3 << 4);
    assert_eq!(&img.data[0..4], &e0, "px0 CLCL planar layout");
    assert_eq!(&img.data[4..8], &e1, "px1 CLCL planar layout");
}

// ---------------------------------------------------------------------------
// U8 — encoder honors `swaps_dimensions` (built-in profile 1020)
// ---------------------------------------------------------------------------

/// Profile 1020 is `width=176, height=220`, `swaps_dimensions=true`, BE.
/// The encoder must encode at swapped dims (fw=h, fh=w), and the full
/// encode→decode pipeline must surface swapped display dimensions.
#[test]
fn u8_encoder_honors_swaps_dimensions_profile_1020() {
    let db = ProfileDb::load_builtin().unwrap();
    let profile = db.get(1020).expect("built-in profile 1020").clone();
    assert!(profile.swaps_dimensions, "1020 must be a swaps_dimensions profile");
    assert!(!profile.little_endian, "1020 is big-endian");

    let w = usize::try_from(profile.width).unwrap(); // 176
    let h = usize::try_from(profile.height).unwrap(); // 220

    // Saturated checkerboard so the RGB565 roundtrip is byte-exact.
    let mut bgra = vec![0u8; w * h * 4];
    for i in 0..w * h {
        let base = i * 4;
        if (i / w + i % w) % 2 == 0 {
            bgra[base..base + 4].copy_from_slice(&[255, 255, 255, 255]);
        } else {
            bgra[base..base + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }

    let file = build_ithmb_file(&bgra, profile.width, profile.height, &profile);

    // Full pipeline: decode applies swaps_dimensions → display dims swapped.
    let img = decode_ithmb(&file, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        img.width,
        u32::try_from(profile.height).unwrap(),
        "display width swapped"
    );
    assert_eq!(
        img.height,
        u32::try_from(profile.width).unwrap(),
        "display height swapped"
    );
    // Encode→decode is identity for this saturated pattern.
    assert_eq!(img.data, bgra, "roundtrip must reproduce the original image");

    // The encoder must actually encode at swapped dims (fw=h, fh=w) — this is
    // the distinguishing check vs the pre-fix encoder.
    let expected_frame = encode_rgb565(&bgra, profile.height, profile.width, true);
    assert_eq!(&file[4..], &expected_frame, "encoder must encode at swapped dims");
}

// ---------------------------------------------------------------------------
// U9 — reordered RGB555 encoder honors `little_endian`
// ---------------------------------------------------------------------------

fn u9_reordered_roundtrip(prefix: i32, little_endian: bool) {
    let w = 2;
    let h = 2;
    let bgra = vec![
        0, 0, 255, 255, // red
        0, 255, 0, 255, // green
        255, 0, 0, 255, // blue
        255, 255, 255, 255, // white
    ];
    let profile = Profile {
        prefix,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: w * h * 2,
        little_endian,
        ..Default::default()
    };
    let encoded = encode_bgra(&bgra, w, h, &profile);
    let mut src = prefix.to_be_bytes().to_vec();
    src.extend_from_slice(&encoded);
    let img = decode_with_profile(&src, &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, bgra, "reordered roundtrip must be byte-exact");
}

#[test]
fn u9_reordered_roundtrip_byte_exact_le() {
    u9_reordered_roundtrip(0x0000_3001, true);
}

#[test]
fn u9_reordered_roundtrip_byte_exact_be() {
    u9_reordered_roundtrip(0x0000_3002, false);
}
