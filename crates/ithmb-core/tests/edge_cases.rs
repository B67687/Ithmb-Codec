//! Edge-case and random-byte-mutation fuzz tests.
//!
//! Coverage:
//!   1. Empty input (0-byte slice -> `DecodeError`)
//!   2. Truncated 4-byte prefix only
//!   3. Truncated pixel data mid-stream
//!   4. Corrupted format ID (invalid u32 prefix)
//!   5. Zero-width/height profile -> `DecodeError::InvalidFormat`
//!   6. Max reasonable dimensions 4096x4096
//!   7. Malformed `PhotoDB` chunks (wrong magic, truncated header, invalid chunk type)
//!   8. JPEG with corrupt SOI/EOI markers
//!   9. CL/CLCL with partial nibble data
//!  10. Random byte mutation fuzz (10 000 iterations total)
#![allow(
    clippy::borrow_interior_mutable_const,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::doc_markdown,
    clippy::declare_interior_mutable_const,
    clippy::pedantic,
    clippy::unwrap_used
)]
// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

use divan as _;
use image as _;
use ithmb_core::enc::*;
use ithmb_core::error::DecodeError;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::{decode_ithmb, open_ithmb};
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use proptest as _;
use std::sync::atomic::AtomicBool;
use thiserror as _;

mod util;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const CANCELED: AtomicBool = AtomicBool::new(false);

/// Build a valid `.ithmb` byte buffer for a given raw encoding.
///
/// `size` is the pixel width/height (square).
fn build_valid_ithmb(size: i32, encoding: Encoding) -> Vec<u8> {
    let bgra = vec![128u8; (size * size * 4) as usize]; // neutral gray
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

/// Build a valid CL `.ithmb` buffer.
fn build_valid_cl_ithmb(size: i32) -> Vec<u8> {
    let n = (size * size) as usize;
    let profile = Profile {
        prefix: 9999,
        width: size,
        height: size,
        encoding: Encoding::Yuv422,
        frame_byte_length: (n * 2) as i32,
        cl_chroma: true,
        ..Default::default()
    };
    let bgra = vec![128u8; (size * size * 4) as usize];
    let encoded = encode_bgra(&bgra, size, size, &profile);
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    buf.extend_from_slice(&encoded);
    buf
}

// ---------------------------------------------------------------------------
// 1. Empty input (0-byte slice)
// ---------------------------------------------------------------------------

#[test]
fn test_empty_input_decode_ithmb() {
    let result = decode_ithmb(&[], &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 0 })
    ));
}

#[test]
fn test_empty_input_open_ithmb() {
    let result = open_ithmb(&[], &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { expected: 4, .. })));
}

#[test]
fn test_empty_input_decode_with_profile() {
    let profile = util::make_profile(1, 1, Encoding::Rgb565);
    let result = decode_with_profile(&[], &profile, &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 0 })
    ));
}

// ---------------------------------------------------------------------------
// 2. Truncated 4-byte prefix only
// ---------------------------------------------------------------------------

#[test]
fn test_truncated_prefix_3_bytes() {
    let result = decode_ithmb(b"\x10\x00\x00", &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 3 })
    ));
}

#[test]
fn test_truncated_prefix_1_byte() {
    let result = decode_ithmb(b"\x10", &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 1 })
    ));
}

#[test]
fn test_truncated_prefix_0_bytes_with_decode_with_profile() {
    let profile = util::make_profile(1, 1, Encoding::Rgb565);
    let result = decode_with_profile(b"\x00\x00\x00", &profile, &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort { expected: 4, actual: 3 })
    ));
}

// ---------------------------------------------------------------------------
// 3. Truncated pixel data mid-stream
// ---------------------------------------------------------------------------

#[test]
fn test_truncated_pixel_data_rgb565() {
    // Valid prefix for known profile 1007 (480×864 RGB565), but pixel data is
    // far shorter than expected.
    let mut buf = vec![0u8; 10];
    buf[0..4].copy_from_slice(&1007i32.to_be_bytes());
    // Only 6 bytes of pixel data instead of 480*864*2 = 829 440.
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_truncated_pixel_data_rgb555() {
    // Use decode_with_profile with a 4×4 RGB555 profile but only 4 bytes of
    // pixel data instead of 32.
    let profile = util::make_profile(32, 32, Encoding::Rgb555);
    let mut buf = vec![0u8; 8]; // 4 prefix + 4 pixel (need 2048 → deficit=2044 >> 256)
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_truncated_pixel_data_uyvy() {
    let profile = util::make_profile(32, 32, Encoding::Yuv422);
    let mut buf = vec![0u8; 8]; // 4 prefix + 4 pixel (need 2048 → deficit=2044 >> 256)
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

// ---------------------------------------------------------------------------
// 4. Corrupted format ID (invalid / unknown prefix)
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_prefix_not_jpeg() {
    // Random prefix that does not match any known profile, and is not a JPEG
    // SOI marker.
    let buf = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(matches!(result, Err(DecodeError::Unsupported(ref m)) if m.contains("unknown")));
}

#[test]
fn test_unknown_prefix_all_zeros() {
    // 0x00000000 is not a known profile and not JPEG SOI.
    let buf = [0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

#[test]
fn test_unknown_prefix_all_ones() {
    // 0xFFFFFFFF is not a known profile. But also not JPEG SOI.
    let buf = [0xFF, 0xFF, 0xFF, 0xFF];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

// ---------------------------------------------------------------------------
// 5. Zero-width/height profile
// ---------------------------------------------------------------------------

#[test]
fn test_zero_width_decode_with_profile() {
    let profile = Profile {
        prefix: 9999,
        width: 0,
        height: 1,
        encoding: Encoding::Rgb565,
        frame_byte_length: 0,
        ..Default::default()
    };
    let buf = vec![0u8; 4]; // prefix only
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::InvalidFormat(ref m)) if m.contains("width and height must be positive"))
    );
}

#[test]
fn test_zero_height_decode_with_profile() {
    let profile = Profile {
        prefix: 9999,
        width: 1,
        height: 0,
        encoding: Encoding::Rgb565,
        frame_byte_length: 0,
        ..Default::default()
    };
    let buf = vec![0u8; 4];
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::InvalidFormat(ref m)) if m.contains("width and height must be positive"))
    );
}

#[test]
fn test_negative_width_decode_with_profile() {
    let profile = Profile {
        prefix: 9999,
        width: -1,
        height: 1,
        encoding: Encoding::Rgb565,
        frame_byte_length: -2,
        ..Default::default()
    };
    let buf = vec![0u8; 4];
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::InvalidFormat(ref m)) if m.contains("width and height must be positive"))
    );
}

#[test]
fn test_negative_height_decode_with_profile() {
    let profile = Profile {
        prefix: 9999,
        width: 1,
        height: -2,
        encoding: Encoding::Rgb565,
        frame_byte_length: -4,
        ..Default::default()
    };
    let buf = vec![0u8; 4];
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::InvalidFormat(ref m)) if m.contains("width and height must be positive"))
    );
}

#[test]
fn test_zero_both_dimensions_decode_with_profile() {
    let profile = Profile {
        prefix: 9999,
        width: 0,
        height: 0,
        encoding: Encoding::Rgb565,
        frame_byte_length: 0,
        ..Default::default()
    };
    let buf = vec![0u8; 4];
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
}

// ---------------------------------------------------------------------------
// 6. Max reasonable dimensions 4096×4096
// ---------------------------------------------------------------------------

#[test]
fn test_max_dimensions_rgb565_decode() {
    // 4096×4096 RGB565 = 33 554 432 bytes of pixel data.
    let w = 4096i32;
    let h = 4096i32;
    let profile = Profile {
        prefix: 9999,
        width: w,
        height: h,
        encoding: Encoding::Rgb565,
        frame_byte_length: w * h * 2,
        ..Default::default()
    };
    let pixel_count = (w * h) as usize;
    let mut buf = vec![0u8; 4 + pixel_count * 2];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    // Fill with white RGB565 pixels (0xFFFF LE).
    buf[4..].fill(0xFF);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(result.is_ok(), "4096×4096 decode should succeed: {:?}", result.err());

    let img = result.unwrap();
    assert_eq!(img.width, 4096);
    assert_eq!(img.height, 4096);
    // Verify first pixel and last pixel.
    assert_eq!(&img.data[0..4], &[255, 255, 255, 255], "first pixel should be white");
    let last_offset = pixel_count * 4 - 4;
    assert_eq!(
        &img.data[last_offset..last_offset + 4],
        &[255, 255, 255, 255],
        "last pixel should be white"
    );
}

// ---------------------------------------------------------------------------
// 7. Malformed PhotoDB chunks
// ---------------------------------------------------------------------------

/// Minimal valid LE PhotoDB preamble: just the 12-byte MHFD header.
fn photodb_mhfd_only() -> Vec<u8> {
    let mut data = vec![0u8; 12];
    data[0..4].copy_from_slice(b"mhfd");
    data[4..8].copy_from_slice(&[12, 0, 0, 0]); // header_size = 12
    data[8..12].copy_from_slice(&[1, 0, 0, 0]); // entry_count = 1
    data
}

#[test]
fn test_photodb_wrong_magic_goes_to_decode_ithmb() {
    // Data starts with "xxxx" which is not PhotoDB magic, so open_ithmb falls
    // through to decode_ithmb.  This is also an unknown prefix → Unsupported.
    let buf = b"xxxxxxxxxxxxxxxx";
    let result = open_ithmb(buf, &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

#[test]
fn test_photodb_truncated_mhfd_header() {
    // Starts with "mhfd" but is shorter than 12 bytes.
    let buf = b"mhfd\x0c\x00"; // only 6 bytes
    let result = open_ithmb(buf, &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_photodb_truncated_mhsd_header() {
    // Valid MHFD, but MHSD header is truncated.
    let mut data = photodb_mhfd_only();
    // Extend with partial MHSD (only 8 bytes of the expected 16).
    data.extend_from_slice(b"mhsd\x10\x00\x00\x00");
    let result = open_ithmb(&data, &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_photodb_invalid_chunk_type_in_tree() {
    // Valid MHFD, valid MHSD, but the next chunk has an unknown magic.
    let mut data = photodb_mhfd_only();
    // MHSD header (16 bytes) covering itself + the next chunk.
    let mhsd_total: u32 = 16 + 12; // MHSD + unknown 12-byte body
    data.extend_from_slice(b"mhsd");
    data.extend_from_slice(&mhsd_total.to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0]); // index=0, record_type=0, entry_count=1

    // Invalid chunk: "xxxx" magic, 12 bytes
    data.extend_from_slice(b"xxxx\x0c\x00\x00\x00\x00\x00\x00\x00");

    let result = open_ithmb(&data, &CANCELED, None);
    // The parser should not crash; it may return Ok (skip unknown) or Err.
    assert!(result.is_ok(), "parser must not panic on invalid chunk type");
}

#[test]
fn test_photodb_mhni_with_offset_out_of_bounds() {
    // Build a minimal LE PhotoDB where the MHNI entry's ithmb_offset points
    // past the buffer end.
    let tree_size = 12 + 16 + 12 + 12 + 36;
    let pixel_offset = tree_size + 999; // far past buffer end
    let mut data = vec![0u8; tree_size + 10]; // some trailing data but not enough

    let mut off = 0usize;

    // MHFD
    data[off..off + 4].copy_from_slice(b"mhfd");
    data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]);
    data[off + 8..off + 12].copy_from_slice(&[1, 0, 0, 0]);
    off += 12;

    // MHSD
    let mhsd_total: u32 = 16 + 12 + 12 + 36;
    data[off..off + 4].copy_from_slice(b"mhsd");
    data[off + 4..off + 8].copy_from_slice(&mhsd_total.to_le_bytes());
    data[off + 8..off + 10].copy_from_slice(&[0, 0]);
    data[off + 10..off + 12].copy_from_slice(&[4, 0]);
    data[off + 12..off + 16].copy_from_slice(&[1, 0, 0, 0]);
    off += 16;

    // MHL
    data[off..off + 4].copy_from_slice(b"mhli");
    data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]);
    data[off + 8..off + 12].copy_from_slice(&[1, 0, 0, 0]);
    off += 12;

    // MHII
    let mhii_total: u32 = 12 + 36;
    data[off..off + 4].copy_from_slice(b"mhii");
    data[off + 4..off + 8].copy_from_slice(&[12, 0, 0, 0]);
    data[off + 8..off + 12].copy_from_slice(&mhii_total.to_le_bytes());
    off += 12;

    // MHNI
    data[off..off + 4].copy_from_slice(b"mhni");
    data[off + 4..off + 8].copy_from_slice(&[36, 0, 0, 0]);
    // format_id at +16 = 1019 (known profile)
    data[off + 16..off + 20].copy_from_slice(&[0xFB, 0x03, 0, 0]);
    // ithmb_offset at +20 points past end
    data[off + 20..off + 24].copy_from_slice(&i32::try_from(pixel_offset).unwrap().to_le_bytes());
    // image_size at +24 = 100
    data[off + 24..off + 28].copy_from_slice(&[100, 0, 0, 0]);
    // height at +32, width at +34
    data[off + 32..off + 34].copy_from_slice(&[16, 0]);
    data[off + 34..off + 36].copy_from_slice(&[16, 0]);

    let result = open_ithmb(&data, &CANCELED, None);
    // Should not panic. The entry data is out of bounds so it is skipped.
    // Out-of-bounds MHNI entries are silently skipped (parser does not error).
    assert!(result.is_ok(), "out-of-bounds MHNI offset should be skipped gracefully");
}

// ---------------------------------------------------------------------------
// 8. JPEG with corrupt SOI/EOI markers
// ---------------------------------------------------------------------------

#[test]
fn test_jpeg_no_soi_marker() {
    // JPEG data without the SOI (\xFF\xD8) marker — starts with garbage.
    let buf = [0x00, 0x01, 0x02, 0x03];
    let result = decode_ithmb(&buf, &CANCELED);
    // Not JPEG SOI and not a known prefix → Unsupported.
    assert!(matches!(result, Err(DecodeError::Unsupported(_))));
}

#[test]
fn test_jpeg_only_soi_truncated() {
    // Only SOI marker, no actual JPEG data after it.
    // This will be detected as a JPEG stream (SOI match), so the fallback
    // JPEG profile is used. The JPEG decoder should fail.
    let buf = [0xFF, 0xD8];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::Jpeg(_) | DecodeError::BufferTooShort { .. })),
        "expected Jpeg or BufferTooShort error for truncated JPEG, got {result:?}",
    );
}

#[test]
fn test_jpeg_soi_plus_garbage() {
    // SOI marker followed by data that is not valid JPEG.
    let buf = [0xFF, 0xD8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::Jpeg(_))),
        "expected Jpeg error for garbage-after-SOI, got {result:?}",
    );
}

#[test]
fn test_jpeg_soi_eoi_minimal() {
    // Minimal valid JPEG: just SOI + EOI (valid structure, but no actual image
    // data — the JPEG decoder will likely fail because there are no frames).
    let buf = [0xFF, 0xD8, 0xFF, 0xD9];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::Jpeg(_))),
        "expected Jpeg error for SOI+EOI minimal, got {result:?}",
    );
}

#[test]
fn test_jpeg_corrupt_invalid_marker() {
    // Start with SOI, then have a corrupt marker before any image data.
    let buf = [0xFF, 0xD8, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0xD9];
    let result = decode_ithmb(&buf, &CANCELED);
    assert!(
        matches!(result, Err(DecodeError::Jpeg(_))),
        "expected Jpeg error for corrupt marker, got {result:?}",
    );
}

// ---------------------------------------------------------------------------
// 9. CL / CLCL with partial nibble data
// ---------------------------------------------------------------------------

#[test]
fn test_clcl_truncated_y_plane() {
    // CLCL 4×4: needs Y(16) + Cb(8) + Cr(8) = 32 bytes after prefix.
    let profile = Profile {
        prefix: 9999,
        width: 32,
        height: 32,
        encoding: Encoding::Yuv422,
        frame_byte_length: 2048,
        clcl_chroma: true,
        ..Default::default()
    };
    // Only prefix + partial Y plane (8 bytes instead of 16).
    let mut buf = vec![0u8; 4 + 8]; // 4 prefix + 8 Y bytes (need 2048 → deficit=2040 >> 256)
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(128);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_clcl_truncated_chroma() {
    // CLCL 141×2: prefix + Y(282) + partial Cb(4 instead of 282).
    let profile = Profile {
        prefix: 9999,
        width: 141,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 564,
        clcl_chroma: true,
        ..Default::default()
    };
    let mut buf = vec![0u8; 4 + 16 + 4];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(128);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

fn test_clcl_empty_after_prefix() {
    // CLCL 130×2: 520 bytes needed after prefix.
    let profile = Profile {
        prefix: 9999,
        width: 130,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 520,
        clcl_chroma: true,
        ..Default::default()
    };
    let buf = vec![0u8; 4]; // prefix only
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

fn test_cl_truncated_data() {
    // CL 134×2: needs 536 bytes after prefix.
    let profile = Profile {
        prefix: 9999,
        width: 134,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 536,
        cl_chroma: true,
        ..Default::default()
    };
    // Only prefix + 8 bytes instead of 32.
    let mut buf = vec![0u8; 4 + 8];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(128);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

fn test_cl_truncated_chroma_plane() {
    // CL 141×2: prefix + Y(282) + partial CbCr(4 instead of 282).
    let profile = Profile {
        prefix: 9999,
        width: 141,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 564,
        cl_chroma: true,
        ..Default::default()
    };
    // Prefix + full Y(16) + partial CbCr(4 instead of 16).
    let mut buf = vec![0u8; 4 + 16 + 4];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(128);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

fn test_cl_empty_after_prefix() {
    // CL 130×2: 520 bytes needed after prefix.
    let profile = Profile {
        prefix: 9999,
        width: 130,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 520,
        cl_chroma: true,
        ..Default::default()
    };
    let buf = vec![0u8; 4]; // prefix only
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

// ---------------------------------------------------------------------------
// 10. Random byte mutation fuzz  — 10 000 iterations
// ---------------------------------------------------------------------------
//
// For each format, generate valid .ithmb data, then apply random mutations
// (bit flips, truncation, byte/run repetition) at random positions via a
// seeded RNG.  Verify the decoder returns Err (any DecodeError variant)
// without panicking.  On the rare chance the mutation produces a valid
// file, Ok is accepted too — the invariant is "no panic".

fn apply_mutation(data: &[u8], rng: &mut util::rng::SeededRng) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let mutation = rng.next_u32() % 4;
    match mutation {
        0 => {
            // BitFlip
            let mut mutated = data.to_vec();
            let pos = (rng.next_u32() as usize) % mutated.len();
            let bit = 1u8 << (rng.next_u32() % 8);
            mutated[pos] ^= bit;
            mutated
        }
        1 => {
            // Truncate
            let new_len = if data.is_empty() {
                0
            } else {
                (rng.next_u32() as usize) % data.len()
            };
            data[..new_len].to_vec()
        }
        2 => {
            // Repeat
            let start = (rng.next_u32() as usize) % data.len();
            let end = if data.is_empty() {
                0
            } else {
                let max = data.len() - start;
                if max == 0 {
                    start
                } else {
                    start + (rng.next_u32() as usize) % max
                }
            };
            let count = 1 + (rng.next_u32() % 4);
            let mut mutated = data.to_vec();
            let chunk = mutated[start..end].to_vec();
            for _ in 0..count {
                let insert_pos = (rng.next_u32() as usize) % (mutated.len() + 1);
                let mut extended = Vec::with_capacity(mutated.len() + chunk.len());
                extended.extend_from_slice(&mutated[..insert_pos]);
                extended.extend_from_slice(&chunk);
                extended.extend_from_slice(&mutated[insert_pos..]);
                mutated = extended;
            }
            mutated
        }
        _ => {
            // CorruptPrefix
            let mut mutated = data.to_vec();
            if mutated.len() >= 4 {
                mutated[0] = rng.next_u32() as u8;
                mutated[1] = rng.next_u32() as u8;
                mutated[2] = rng.next_u32() as u8;
                mutated[3] = rng.next_u32() as u8;
            }
            mutated
        }
    }
}

/// Same as `run_fuzz_iterations` but uses `decode_with_profile` with a
/// specific profile.
fn run_fuzz_iterations_with_profile<F>(name: &str, build_valid: F, profile: &Profile, iterations: usize)
where
    F: Fn() -> Vec<u8>,
{
    let mut rng = util::rng::SeededRng::new(0xBEEF_CAFE_D00D_2026);
    for i in 0..iterations {
        let valid = build_valid();
        let mutated = apply_mutation(&valid, &mut rng);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = decode_with_profile(&mutated, profile, &CANCELED);
        }));
        assert!(
            result.is_ok(),
            "{name}: panic on mutation iteration {i} (mutated len {})",
            mutated.len(),
        );
    }
}

// --- RGB565 fuzz (2000 iterations) ---

#[test]
fn test_fuzz_rgb565_random_mutations() {
    let profile = util::make_profile(4, 4, Encoding::Rgb565);
    run_fuzz_iterations_with_profile("rgb565", || build_valid_ithmb(4, Encoding::Rgb565), &profile, 2000);
}

// --- RGB555 fuzz (1000 iterations) ---

#[test]
fn test_fuzz_rgb555_random_mutations() {
    let profile = util::make_profile(4, 4, Encoding::Rgb555);
    run_fuzz_iterations_with_profile("rgb555", || build_valid_ithmb(4, Encoding::Rgb555), &profile, 1000);
}

// --- Reordered RGB555 fuzz (1000 iterations) ---

#[test]
fn test_fuzz_reordered_rgb555_random_mutations() {
    let profile = Profile {
        prefix: 9999,
        width: 4,
        height: 4,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: 32,
        little_endian: false,
        ..Default::default()
    };
    run_fuzz_iterations_with_profile(
        "reordered_rgb555",
        || build_valid_ithmb(4, Encoding::ReorderedRgb555),
        &profile,
        1000,
    );
}

// --- UYVY fuzz (2000 iterations) ---

#[test]
fn test_fuzz_uyvy_random_mutations() {
    let profile = util::make_profile(4, 4, Encoding::Yuv422);
    run_fuzz_iterations_with_profile("uyvy", || build_valid_ithmb(4, Encoding::Yuv422), &profile, 2000);
}

// --- YCbCr 4:2:0 fuzz (2000 iterations) ---

#[test]
fn test_fuzz_ycbcr420_random_mutations() {
    let profile = Profile {
        prefix: 9999,
        width: 4,
        height: 4,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: 24, // 4×4=16 Y + 2×2=4 Cb + 2×2=4 Cr
        ..Default::default()
    };
    run_fuzz_iterations_with_profile("ycbcr420", || build_valid_ycbcr420_ithmb(4), &profile, 2000);
}

// --- CLCL fuzz (1000 iterations) ---

#[test]
fn test_fuzz_clcl_random_mutations() {
    let n = 16usize; // 4×4 = 16 pixels
    let chroma_len = n.div_ceil(2);
    let profile = Profile {
        prefix: 9999,
        width: 4,
        height: 4,
        encoding: Encoding::Yuv422,
        frame_byte_length: (n + chroma_len + chroma_len) as i32,
        clcl_chroma: true,
        ..Default::default()
    };
    run_fuzz_iterations_with_profile("clcl", || build_valid_clcl_ithmb(4), &profile, 1000);
}

// --- CL fuzz (1000 iterations) ---

#[test]
fn test_fuzz_cl_random_mutations() {
    let n = 16usize;
    let profile = Profile {
        prefix: 9999,
        width: 4,
        height: 4,
        encoding: Encoding::Yuv422,
        frame_byte_length: (n * 2) as i32,
        cl_chroma: true,
        ..Default::default()
    };
    run_fuzz_iterations_with_profile("cl", || build_valid_cl_ithmb(4), &profile, 1000);
}

// ---------------------------------------------------------------------------
// Additional edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_decode_ithmb_prefix_1007_with_extra_trailing_bytes() {
    // Provide valid data for profile 1007 (480×864 RGB565) plus extra bytes
    // at the end.  The decoder should ignore trailing bytes and succeed.
    let w = 480usize;
    let h = 864usize;
    let mut buf = vec![0u8; 4 + w * h * 2 + 100]; // 100 extra bytes
    buf[0..4].copy_from_slice(&1007i32.to_be_bytes());
    buf[4..4 + w * h * 2].fill(0xFF);

    let result = decode_ithmb(&buf, &CANCELED);
    assert!(result.is_ok(), "trailing bytes should not cause failure");
    let img = result.unwrap();
    assert_eq!(img.width, 480);
    assert_eq!(img.height, 864);
}

#[test]
fn test_decode_ithmb_prefix_with_file_too_large_no_oom() {
    // Ensure the decoder doesn't OOM on a huge but plausible file.
    // Use decode_with_profile with a profile that says 10000×10000 but with
    // a small input buffer.  This should return BufferTooShort, not OOM.
    let profile = Profile {
        prefix: 9999,
        width: 10000,
        height: 10000,
        encoding: Encoding::Rgb565,
        frame_byte_length: 200_000_000,
        ..Default::default()
    };
    let buf = vec![0u8; 4 + 100]; // only 100 bytes — far too short
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

#[test]
fn test_profile_with_crop_exceeds_image() {
    // Crop region completely outside the decoded image should clamp.
    let profile = Profile {
        prefix: 9999,
        width: 2,
        height: 2,
        encoding: Encoding::Rgb565,
        frame_byte_length: 8,
        crop_x: 5,
        crop_y: 5,
        crop_width: 10,
        crop_height: 10,
        ..Default::default()
    };
    let mut buf = vec![0u8; 4 + 4 * 2];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(0xFF);

    let result = decode_with_profile(&buf, &profile, &CANCELED);
    // Should not panic; clamped to 2×2 (original image unchanged).
    assert!(result.is_ok());
    let img = result.unwrap();
    assert_eq!(img.width, 2);
    assert_eq!(img.height, 2);
}

#[test]
fn test_profile_with_swapped_dimensions_and_zero_size() {
    // swaps_dimensions with zero dimensions still returns InvalidFormat
    // because the decoder checks w/h before swap.
    let profile = Profile {
        prefix: 9999,
        width: 0,
        height: 1,
        encoding: Encoding::Rgb565,
        frame_byte_length: 0,
        swaps_dimensions: true,
        ..Default::default()
    };
    let buf = vec![0u8; 4];
    let result = decode_with_profile(&buf, &profile, &CANCELED);
    assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));
}

#[test]
fn test_decode_ithmb_prefix_1019_interlaced_uyvy_trailing_garbage() {
    // Profile 1019 is 720×480 interlaced UYVY.  Provide valid data plus
    // trailing garbage.
    let w = 720usize;
    let h = 480usize;
    let mut buf = vec![0u8; 4 + w * h * 2 + 50];
    buf[0..4].copy_from_slice(&1019i32.to_be_bytes());
    buf[4..4 + w * h * 2].fill(128);

    let result = decode_ithmb(&buf, &CANCELED);
    assert!(result.is_ok(), "trailing garbage should be tolerated");
}

#[test]
fn test_decode_ithmb_prefix_2002_big_endian_rgb565() {
    // Profile 2002 is 50×50 big-endian RGB565.
    let w = 50usize;
    let h = 50usize;
    let mut buf = vec![0u8; 4 + w * h * 2];
    buf[0..4].copy_from_slice(&2002i32.to_be_bytes());
    buf[4..].fill(0xFF);

    let img = decode_ithmb(&buf, &CANCELED).unwrap();
    assert_eq!(img.width, w as u32);
    assert_eq!(img.height, h as u32);
    assert_eq!(img.data.len(), w * h * 4);
}

#[test]
fn test_decode_with_profile_rotation_noop() {
    // Rotation value that is not a multiple of 90 should be a no-op.
    let profile = Profile {
        prefix: 9999,
        width: 2,
        height: 1,
        encoding: Encoding::Rgb565,
        frame_byte_length: 4,
        rotation: 45,
        ..Default::default()
    };
    let mut buf = vec![0u8; 4 + 2 * 2];
    buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
    buf[4..].fill(0xFF);

    let img = decode_with_profile(&buf, &profile, &CANCELED).unwrap();
    assert_eq!(img.width, 2);
    assert_eq!(img.height, 1);
    assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
}

// ---------------------------------------------------------------------------
// Additional malformed PhotoDB — open_ithmb with bad magic
// ---------------------------------------------------------------------------

#[test]
fn test_open_ithmb_photodb_be_magic_truncated() {
    // Starts with BE magic "dfhm" but is too short.
    let buf: &[u8] = &[0x64, 0x66, 0x68, 0x6d, 0x00]; // only 5 bytes
    let result = open_ithmb(buf, &CANCELED, None);
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));
}

// ---------------------------------------------------------------------------
// Decoder determinism — same input always produces same output
// ---------------------------------------------------------------------------

#[test]
fn test_decoder_deterministic_rgb565() {
    let profile = util::make_profile(4, 4, Encoding::Rgb565);
    let data = build_valid_ithmb(4, Encoding::Rgb565);
    let r1 = decode_with_profile(&data, &profile, &CANCELED).unwrap();
    let r2 = decode_with_profile(&data, &profile, &CANCELED).unwrap();
    assert_eq!(r1.data, r2.data);
    assert_eq!(r1.width, r2.width);
    assert_eq!(r1.height, r2.height);
}

#[test]
fn test_decoder_deterministic_uyvy() {
    let profile = util::make_profile(4, 4, Encoding::Yuv422);
    let data = build_valid_ithmb(4, Encoding::Yuv422);
    let r1 = decode_with_profile(&data, &profile, &CANCELED).unwrap();
    let r2 = decode_with_profile(&data, &profile, &CANCELED).unwrap();
    assert_eq!(r1.data, r2.data);
}

// ---------------------------------------------------------------------------
// 11. Trailing padding tolerance (256-byte deficit threshold)
// ---------------------------------------------------------------------------
//
// The `TRAILING_PADDING_TOLERANCE` constant (256 bytes) controls how much
// short a buffer can be before the decoder rejects it. A deficit ≤ 256 is
// zero-padded and decoded successfully; deficit > 256 returns BufferTooShort.

#[test]
fn test_trailing_padding_within_tolerance_rgb565() {
    // RGB565 16×16: 512 bytes pixel data.
    // Truncate by 128 → deficit 128 < 256 → SUCCEEDS (zero-padded).
    let data = build_valid_ithmb(16, Encoding::Rgb565);
    let expected = 16 * 16 * 2; // 512
    let deficit = 128usize;
    let truncated = data[..4 + expected - deficit].to_vec();

    let profile = util::make_profile(16, 16, Encoding::Rgb565);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(result.is_ok(), "deficit {deficit} < 256 should succeed, got {result:?}");
    let img = result.unwrap();
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 16);
    assert_eq!(img.data.len(), 16 * 16 * 4);
}

#[test]
fn test_trailing_padding_at_tolerance_boundary_rgb565() {
    // Truncate by exactly 256 → deficit == 256 → SUCCEEDS (at boundary).
    let data = build_valid_ithmb(16, Encoding::Rgb565);
    let expected = 16 * 16 * 2; // 512
    let deficit = 256usize;
    let truncated = data[..4 + expected - deficit].to_vec();

    let profile = util::make_profile(16, 16, Encoding::Rgb565);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(
        result.is_ok(),
        "deficit {deficit} == 256 should succeed, got {result:?}"
    );
    let img = result.unwrap();
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 16);
    assert_eq!(img.data.len(), 16 * 16 * 4);
}

#[test]
fn test_trailing_padding_exceeds_tolerance_rgb565() {
    // Truncate by 300 → deficit 300 > 256 → BufferTooShort.
    let data = build_valid_ithmb(16, Encoding::Rgb565);
    let expected = 16 * 16 * 2; // 512
    let deficit = 300usize;
    let truncated = data[..4 + expected - deficit].to_vec();
    // actual = 212

    let profile = util::make_profile(16, 16, Encoding::Rgb565);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort {
            expected: 512,
            actual: 212,
        })
    ));
}

#[test]
fn test_trailing_padding_within_tolerance_uyvy() {
    // UYVY 16×16: 512 bytes pixel data.
    // Truncate by 128 → deficit 128 < 256 → SUCCEEDS (zero-padded).
    let data = build_valid_ithmb(16, Encoding::Yuv422);
    let expected = 16 * 16 * 2; // 512
    let deficit = 128usize;
    let truncated = data[..4 + expected - deficit].to_vec();

    let profile = util::make_profile(16, 16, Encoding::Yuv422);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(result.is_ok(), "deficit {deficit} < 256 should succeed, got {result:?}");
    let img = result.unwrap();
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 16);
    assert_eq!(img.data.len(), 16 * 16 * 4);
}

#[test]
fn test_trailing_padding_at_tolerance_boundary_uyvy() {
    // Truncate by exactly 256 → deficit == 256 → SUCCEEDS (at boundary).
    let data = build_valid_ithmb(16, Encoding::Yuv422);
    let expected = 16 * 16 * 2; // 512
    let deficit = 256usize;
    let truncated = data[..4 + expected - deficit].to_vec();

    let profile = util::make_profile(16, 16, Encoding::Yuv422);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(
        result.is_ok(),
        "deficit {deficit} == 256 should succeed, got {result:?}"
    );
    let img = result.unwrap();
    assert_eq!(img.width, 16);
    assert_eq!(img.height, 16);
    assert_eq!(img.data.len(), 16 * 16 * 4);
}

#[test]
fn test_trailing_padding_exceeds_tolerance_uyvy() {
    // Truncate by 300 → deficit 300 > 256 → BufferTooShort.
    let data = build_valid_ithmb(16, Encoding::Yuv422);
    let expected = 16 * 16 * 2; // 512
    let deficit = 300usize;
    let truncated = data[..4 + expected - deficit].to_vec();
    // actual = 212

    let profile = util::make_profile(16, 16, Encoding::Yuv422);
    let result = decode_with_profile(&truncated, &profile, &CANCELED);
    assert!(matches!(
        result,
        Err(DecodeError::BufferTooShort {
            expected: 512,
            actual: 212,
        })
    ));
}

// ---------------------------------------------------------------------------
// 12. T-prefix / embedded JPEG carving fallback
// ---------------------------------------------------------------------------
//
// When `decode_ithmb` encounters an unknown prefix that is not a JPEG SOI
// and does not match the data-length heuristic, it falls back to scanning
// the buffer for an embedded JPEG (SOI → JFIF/Exif → EOI) within the
// first 4 MiB. These tests cover that fallback path.
// ---------------------------------------------------------------------------

/// T-prefix signature used in real-world files (unknown to the profile DB).
const UNKNOWN_TPREFIX: [u8; 4] = [0x4D, 0xA9, 0xE9, 0xA0];

/// Build a valid 8×8 JPEG with JFIF marker using the `image` crate encoder.
fn make_jpeg_jfif_8x8() -> Vec<u8> {
    let mut out = Vec::new();
    let pixels = vec![128u8; 8 * 8 * 3];
    let mut encoder = image::codecs::jpeg::JpegEncoder::new(&mut out);
    encoder
        .encode(&pixels, 8, 8, image::ExtendedColorType::Rgb8)
        .expect("JPEG encoder should succeed");
    out
}

#[test]
fn test_embedded_jpeg_in_unknown_prefix() {
    // Buffer: [T-prefix] [valid JPEG with JFIF]
    // The scanner should find the embedded JPEG and decode it successfully.
    let jpeg_data = make_jpeg_jfif_8x8();
    let mut buf = Vec::with_capacity(4 + jpeg_data.len());
    buf.extend_from_slice(&UNKNOWN_TPREFIX);
    buf.extend_from_slice(&jpeg_data);

    let img = decode_ithmb(&buf, &AtomicBool::new(false))
        .expect("embedded JPEG with JFIF should decode via carving fallback");
    assert_eq!(img.width, 8);
    assert_eq!(img.height, 8);
    assert_eq!(img.data.len(), 8 * 8 * 4);
}

#[test]
fn test_embedded_jpeg_no_jfif_marker() {
    // Buffer: [T-prefix] [SOI + garbage + EOI] — no JFIF or Exif marker.
    // The scanner skips the SOI because `has_jpeg_marker` returns false,
    // then exhausts the buffer and returns None → Unsupported.
    let embedded: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xD9];
    let mut buf = Vec::with_capacity(4 + embedded.len());
    buf.extend_from_slice(&UNKNOWN_TPREFIX);
    buf.extend_from_slice(&embedded);

    let result = decode_ithmb(&buf, &AtomicBool::new(false));
    assert!(
        matches!(result, Err(DecodeError::Unsupported(_))),
        "expected Unsupported, got {result:?}"
    );
}

#[test]
fn test_embedded_jpeg_exceeds_scan_limit() {
    // Buffer: [T-prefix] [3 MB × 0x00 padding] [JPEG with JFIF]
    // The JPEG SOI is at offset 4 200 004 > 4 MiB (JPEG_SCAN_LIMIT).
    // The scanner never sees it → Unsupported.
    let jpeg_data = make_jpeg_jfif_8x8();
    let offset = 4_200_000; // beyond the 4 MiB scan window
    let mut buf = vec![0u8; offset + jpeg_data.len()];
    buf[0..4].copy_from_slice(&UNKNOWN_TPREFIX);
    buf[offset..offset + jpeg_data.len()].copy_from_slice(&jpeg_data);

    let result = decode_ithmb(&buf, &AtomicBool::new(false));
    assert!(
        matches!(result, Err(DecodeError::Unsupported(_))),
        "expected Unsupported, got {result:?}"
    );
}

#[test]
fn test_embedded_jpeg_canceled_mid_scan() {
    // Buffer: [T-prefix] [2.9 MB × 0x00 padding] [JPEG with JFIF]
    // JPEG SOI is at ~offset 3 000 004, within scan window.
    // Canceled=true means the JPEG decode loop checks cancellation;
    // small images may decode before cancellation takes effect.
    let jpeg_data = make_jpeg_jfif_8x8();
    let offset = 3_000_000; // within scan window
    let mut buf = vec![0u8; offset + jpeg_data.len()];
    buf[0..4].copy_from_slice(&UNKNOWN_TPREFIX);
    buf[offset..offset + jpeg_data.len()].copy_from_slice(&jpeg_data);

    let canceled = AtomicBool::new(true);
    let result = decode_ithmb(&buf, &canceled);
    match result {
        Ok(img) => {
            // Small image may complete before cancellation takes effect.
            assert_eq!(img.width, 8);
            assert_eq!(img.height, 8);
        }
        Err(e) => {
            assert!(matches!(e, DecodeError::Canceled(_)), "expected Canceled, got {e:?}");
        }
    }
}
