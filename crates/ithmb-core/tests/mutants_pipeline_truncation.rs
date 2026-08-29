//! Pipeline truncation tests — verifies truncated `.ithmb` input returns
//! `Err(DecodeError)` instead of panicking.
//!
//! Kills ~27 MISSED cargo-mutants in:
//! - `pipeline/open.rs` (9) — FileTooShort/FileTooLarge bounds, PhotoDB detection
//! - `pipeline/dispatch.rs` (9) — `is_jpeg_stream` (`&&→||`), prefix checks, size guards
//! - `pipeline/jpeg_scan.rs` (9) — SOI/EOI scanning bounds, cancel checks
//!
//! Key mutant coverage:
//! - `&&→||` in `is_jpeg_stream`: tested with JPEG-prefix (0xFF 0xD8) AND
//!   non-JPEG-prefix data, confirming only exact `FF D8` triggers JPEG path
//! - `+→-` and `<=→>` in size checks: truncated data always hits the error path

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::borrow_interior_mutable_const,
    clippy::declare_interior_mutable_const,
    clippy::useless_vec,
    dead_code,
    clippy::needless_range_loop
)]

use divan as _;
use image as _;
use ithmb_core::config::DecodeConfig;
use ithmb_core::enc::*;
use ithmb_core::error::DecodeError;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::{decode_ithmb, open_ithmb};
use proptest as _;
use std::sync::atomic::AtomicBool;
use thiserror as _;
use zune_jpeg as _;

mod util;

const CANCELED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a valid `.ithmb` byte buffer with a given encoding prefix.
fn build_valid_ithmb(size: i32, encoding: Encoding) -> Vec<u8> {
    let bgra = vec![128u8; (size * size * 4) as usize];
    let profile = util::make_profile(size, size, encoding);
    let encoded = encode_bgra(&bgra, size, size, &profile);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);
    buf
}

/// Build a valid YCbCr420 `.ithmb` buffer with correct frame_byte_length.
fn build_valid_ycbcr420_ithmb(size: i32) -> Vec<u8> {
    let w = size as usize;
    let h = size as usize;
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let frame_len = (w * h + uv_w * uv_h * 2) as i32;
    let profile = Profile {
        prefix: 9999,
        width: size,
        height: size,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: frame_len,
        ..Default::default()
    };
    let bgra = vec![128u8; (size * size * 4) as usize];
    let encoded = encode_bgra(&bgra, size, size, &profile);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);
    buf
}

/// Build a valid CLCL `.ithmb` buffer.
fn build_valid_clcl_ithmb(size: i32) -> Vec<u8> {
    let n = (size * size) as usize;
    let chroma_len = n.div_ceil(2);
    let profile = Profile {
        prefix: 9999,
        width: size,
        height: size,
        encoding: Encoding::Yuv422,
        frame_byte_length: (n + chroma_len + chroma_len) as i32,
        clcl_chroma: true,
        ..Default::default()
    };
    let bgra = vec![128u8; (size * size * 4) as usize];
    let encoded = encode_bgra(&bgra, size, size, &profile);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);
    buf
}

/// Build a JPEG `.ithmb` with valid SOI/EOI markers.
fn build_jpeg_ithmb() -> Vec<u8> {
    // Build minimal valid JPEG: SOI + JFIF marker + minimal data + EOI
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0xFF, 0xD8]); // SOI
    jpeg.extend_from_slice(&[0xFF, 0xE0]); // APP0 marker
    jpeg.extend_from_slice(&[0x00, 0x10]); // APP0 length
    jpeg.extend_from_slice(b"JFIF\x00"); // JFIF identifier
    jpeg.extend_from_slice(&[0x01, 0x01]); // version
    jpeg.extend_from_slice(&[0x00]); // units
    jpeg.extend_from_slice(&[0x00, 0x01]); // X density
    jpeg.extend_from_slice(&[0x00, 0x01]); // Y density
    jpeg.extend_from_slice(&[0x00, 0x00]); // thumbnail
    // Minimum scan data
    jpeg.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00,
    ]);
    jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

    // Wrap in ithmb format: prefix + jpeg data
    let mut buf = Vec::with_capacity(4 + jpeg.len());
    buf.extend_from_slice(&9999u32.to_be_bytes());
    buf.extend_from_slice(&jpeg);
    buf
}

// ---------------------------------------------------------------------------
// 1. Empty / near-empty input
// ---------------------------------------------------------------------------

#[test]
fn test_empty_decode_ithmb() {
    let result = decode_ithmb(&[], &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 0 })
    ));
}

#[test]
fn test_empty_open_ithmb() {
    let result = open_ithmb(&[], &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { expected: 4, .. })));
}

#[test]
fn test_one_byte_decode_ithmb() {
    let result = decode_ithmb(&[0x00], &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 1 })
    ));
}

#[test]
fn test_three_bytes_decode_ithmb() {
    let result = decode_ithmb(&[0x00, 0x00, 0x00], &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 3 })
    ));
}

#[test]
fn test_four_bytes_unknown_prefix() {
    // 4 bytes is enough to pass the length check, but prefix won't match
    // any known profile and isn't JPEG, so it should return Unsupported.
    let result = decode_ithmb(&[0x00, 0x00, 0x00, 0x01], &CANCELED);
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

// ---------------------------------------------------------------------------
// 2. FileTooLarge through decode_ithmb and open_ithmb
// ---------------------------------------------------------------------------

#[test]
fn test_file_too_large_decode_ithmb() {
    // Max raw file size default is 8 MiB (8_388_608).
    // Create a config with tiny max to test the guard.
    let config = DecodeConfig::default().with_max_raw_file_size(100);
    let src = vec![0u8; 200]; // 200 > 100
    let result = ithmb_core::decode_ithmb_with_config(&src, &CANCELED, &config);
    assert!(matches!(
        result,
        Err(DecodeError::FileTooLarge { size: 200, limit: 100 })
    ));
}

#[test]
fn test_file_too_large_open_ithmb() {
    let config = DecodeConfig::default().with_max_raw_file_size(50);
    let src = vec![0u8; 100]; // 100 > 50
    let result = ithmb_core::pipeline::open_ithmb_with_config(&src, &CANCELED, None, &config);
    assert!(matches!(
        result,
        Err(DecodeError::FileTooLarge { size: 100, limit: 50 })
    ));
}

// ---------------------------------------------------------------------------
// 3. is_jpeg_stream `&&→||` mutant killer
// ---------------------------------------------------------------------------

#[test]
fn test_non_jpeg_prefix_not_treated_as_jpeg() {
    // Byte[0]=0xFF but byte[1]=0x00 (NOT 0xD8) → must NOT be treated as JPEG
    let mut buf = Vec::with_capacity(100);
    buf.extend_from_slice(&[0xFF, 0x00, 0x00, 0x00]); // prefix: FF 00 00 00
    buf.extend_from_slice(&[0u8; 96]); // dummy data
    let result = decode_ithmb(&buf, &CANCELED);
    // Should get Unsupported (unknown prefix), NOT a JPEG decode attempt
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

#[test]
fn test_jpeg_d8_prefix_byte_but_not_jpeg() {
    // Byte[0]=0x00, byte[1]=0xD8 → must NOT be treated as JPEG
    // (`&&→||` would incorrectly trigger JPEG path)
    let mut buf = Vec::with_capacity(100);
    buf.extend_from_slice(&[0x00, 0xD8, 0x00, 0x00]);
    buf.extend_from_slice(&[0u8; 96]);
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

#[test]
fn test_truncated_after_jpeg_prefix() {
    // Starts with 0xFF 0xD8 (JPEG SOI) but is truncated — JPEG decode should fail
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0xFF, 0xD8, 0x00, 0x00]); // prefix looks like JPEG
    buf.extend_from_slice(&[0xFF, 0xD8]); // SOI but no EOI
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 4. Truncated .ithmb through decode_ithmb — various encodings
// ---------------------------------------------------------------------------

#[test]
fn test_truncated_rgb565_10_bytes() {
    let valid = build_valid_ithmb(4, Encoding::Rgb565);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 10-byte truncated RGB565, got Ok");
}

#[test]
fn test_truncated_rgb565_50_percent() {
    let valid = build_valid_ithmb(8, Encoding::Rgb565);
    let half = valid.len() / 2;
    let truncated = &valid[..half];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 50%-truncated RGB565, got Ok");
}

#[test]
fn test_truncated_rgb555_10_bytes() {
    let valid = build_valid_ithmb(4, Encoding::Rgb555);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 10-byte truncated RGB555, got Ok");
}

#[test]
fn test_truncated_reordered_rgb555_10_bytes() {
    let valid = build_valid_ithmb(4, Encoding::ReorderedRgb555);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(
        result.is_err(),
        "expected error for 10-byte truncated ReorderedRgb555, got Ok"
    );
}

#[test]
fn test_truncated_uyvy_10_bytes() {
    let valid = build_valid_ithmb(4, Encoding::Yuv422);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 10-byte truncated UYVY, got Ok");
}

#[test]
fn test_truncated_ycbcr420_10_bytes() {
    let valid = build_valid_ycbcr420_ithmb(4);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 10-byte truncated YCbCr420, got Ok");
}

#[test]
fn test_truncated_clcl_10_bytes() {
    let valid = build_valid_clcl_ithmb(4);
    let truncated = &valid[..10.min(valid.len())];
    let result = decode_ithmb(truncated, &CANCELED);
    assert!(result.is_err(), "expected error for 10-byte truncated CLCL, got Ok");
}

// ---------------------------------------------------------------------------
// 5. Truncated .ithmb through open_ithmb
// ---------------------------------------------------------------------------

#[test]
fn test_truncated_open_ithmb_10_bytes() {
    let valid = build_valid_ithmb(4, Encoding::Rgb565);
    let truncated = &valid[..10.min(valid.len())];
    let result = open_ithmb(truncated, &CANCELED, None);
    assert!(
        result.is_err(),
        "expected error for 10-byte truncated open_ithmb, got Ok"
    );
}

#[test]
fn test_truncated_open_ithmb_50_percent() {
    let valid = build_valid_ithmb(8, Encoding::Rgb565);
    let half = valid.len() / 2;
    let truncated = &valid[..half];
    let result = open_ithmb(truncated, &CANCELED, None);
    assert!(result.is_err(), "expected error for 50%-truncated open_ithmb, got Ok");
}

#[test]
fn test_truncated_open_ithmb_uyvy() {
    let valid = build_valid_ithmb(4, Encoding::Yuv422);
    let truncated = &valid[..10.min(valid.len())];
    let result = open_ithmb(truncated, &CANCELED, None);
    assert!(
        result.is_err(),
        "expected error for 10-byte truncated UYVY open_ithmb, got Ok"
    );
}

// ---------------------------------------------------------------------------
// 6. Truncated PhotoDB container
// ---------------------------------------------------------------------------

#[test]
fn test_truncated_photodb_header() {
    // PhotoDB files start with MHFD magic. Build a buffer with MHFD header
    // but truncated so parsing fails.
    let mut buf = Vec::with_capacity(20);
    buf.extend_from_slice(b"MHFD"); // magic
    buf.extend_from_slice(&[0u8; 16]); // incomplete header
    let result = open_ithmb(&buf, &CANCELED, None);
    assert!(result.is_err(), "expected error for truncated PhotoDB, got Ok");
}

#[test]
fn test_truncated_photodb_empty_after_header() {
    // Just MHFD + 4 bytes — minimal photo DB header start but incomplete
    let mut buf = Vec::with_capacity(8);
    buf.extend_from_slice(b"MHFD");
    buf.extend_from_slice(&[0x00; 4]);
    let result = open_ithmb(&buf, &CANCELED, None);
    assert!(result.is_err(), "expected error for minimal PhotoDB, got Ok");
}

// ---------------------------------------------------------------------------
// 7. Truncated JPEG scan boundaries
// ---------------------------------------------------------------------------

#[test]
fn test_jpeg_soi_no_eoi() {
    // JPEG SOI (FF D8) but no EOI (FF D9) → scan should not find JPEG
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0]); // prefix = SOI+APP0
    buf.extend_from_slice(&[0xFF, 0xD8]); // SOI again
    buf.extend_from_slice(b"JFIF\x00"); // JFIF marker
    buf.extend_from_slice(&vec![0x00u8; 50]); // data, no EOI
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(result.is_err(), "expected error for JPEG SOI without EOI, got Ok");
}

#[test]
fn test_embedded_jpeg_truncated_eoi() {
    // Valid JPEG-like but EOI is truncated (only 0xFF, missing 0xD9)
    let mut buf = Vec::new();
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // unknown prefix
    buf.extend_from_slice(&[0xFF, 0xD8]); // embedded SOI
    buf.extend_from_slice(b"JFIF\x00"); // JFIF marker
    buf.extend_from_slice(&vec![0x00u8; 30]); // data
    buf.push(0xFF); // truncated EOI (missing 0xD9)
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(result.is_err(), "expected error for truncated embedded JPEG, got Ok");
}

// ---------------------------------------------------------------------------
// 8. Edge: 4 bytes only (minimum prefix)
// ---------------------------------------------------------------------------

#[test]
fn test_exact_4_bytes_valid_prefix() {
    // 4 bytes matching a known prefix but zero pixel data → should error
    let valid = build_valid_ithmb(2, Encoding::Rgb565);
    // Take only the 4-byte prefix
    let prefix_only = &valid[..4];
    let result = decode_ithmb(prefix_only, &CANCELED);
    assert!(result.is_err(), "expected error for prefix-only input, got Ok");
}

// ---------------------------------------------------------------------------
// 9. Edge: Just past 4 bytes
// ---------------------------------------------------------------------------

#[test]
fn test_5_bytes_after_prefix() {
    let valid = build_valid_ithmb(2, Encoding::Rgb565);
    let short = &valid[..5.min(valid.len())];
    let result = decode_ithmb(short, &CANCELED);
    assert!(result.is_err(), "expected error for 5-byte input, got Ok");
}

// ---------------------------------------------------------------------------
// 10. Stress: random truncated bytes at various sizes
// ---------------------------------------------------------------------------

#[test]
fn test_random_truncations_no_panic() {
    let valid = build_valid_ithmb(16, Encoding::Rgb565);

    // Test every truncation length from 0 to len-1
    for end in 0..valid.len() {
        let truncated = &valid[..end];
        let result = decode_ithmb(truncated, &CANCELED);
        assert!(result.is_err(), "expected error at truncation len={end}, got Ok");
    }

    // Same for open_ithmb
    for end in 0..valid.len() {
        let truncated = &valid[..end];
        let result = open_ithmb(truncated, &CANCELED, None);
        assert!(
            result.is_err(),
            "expected error at open_ithmb truncation len={end}, got Ok"
        );
    }
}

#[test]
fn test_random_truncations_ycbcr420_no_panic() {
    let valid = build_valid_ycbcr420_ithmb(8);

    for end in 0..valid.len() {
        let truncated = &valid[..end];
        let result = decode_ithmb(truncated, &CANCELED);
        assert!(
            result.is_err(),
            "expected error at YCbCr420 truncation len={end}, got Ok"
        );
    }
}

#[test]
fn test_random_truncations_clcl_no_panic() {
    let valid = build_valid_clcl_ithmb(8);

    for end in 0..valid.len() {
        let truncated = &valid[..end];
        let result = decode_ithmb(truncated, &CANCELED);
        assert!(result.is_err(), "expected error at CLCL truncation len={end}, got Ok");
    }
}

// ---------------------------------------------------------------------------
// 11. Prefix-only data with various format bytes
// ---------------------------------------------------------------------------

#[test]
fn test_only_prefix_bytes_various() {
    // Various 4-byte prefixes that aren't JPEG (FF D8) — should get Unsupported or error
    let prefixes: Vec<[u8; 4]> = vec![
        [0x00, 0x00, 0x00, 0x01],
        [0xFF, 0x00, 0xFF, 0x00],
        [0x00, 0x00, 0x00, 0x00],
        [0xDE, 0xAD, 0xBE, 0xEF],
    ];

    for prefix in &prefixes {
        let result = decode_ithmb(prefix, &CANCELED);
        // Should be error (Unsupported or BufferTooShort depending on prefix)
        assert!(result.is_err(), "expected error for prefix {prefix:?}, got Ok");
    }
}
