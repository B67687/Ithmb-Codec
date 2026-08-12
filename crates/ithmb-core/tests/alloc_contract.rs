#![allow(clippy::pedantic, clippy::unwrap_used, clippy::missing_panics_doc)]
//! Allocation-contract tests — pin that absurd sizes derived from untrusted
//! input are rejected **before** any large allocation (the CWE-400 class).
//!
//! The motivating regression: a 166-byte progressive JPEG declaring
//! 65535×65535 triggered an ~8 GiB coefficient-buffer allocation in
//! `jpeg-decoder` 0.3.2 and aborted the process. The fix caps JPEG dimensions
//! against a pixel budget *before* decode; the raw-format decoders validate
//! `width × height × bytes-per-pixel` via `checked_mul` before allocating the
//! output buffer.
//!
//! These tests assert the **public** API rejects the absurd inputs — if any
//! guard regressed, the decode would attempt a multi-gigabyte allocation and
//! the test process would be killed (a visible failure).
//!
//! What `dhat` would add on top: precise, per-call heap-usage assertions
//! (e.g. "decode of the 193-byte JPEG allocates < 1 MiB total"). That needs a
//! nightly-only heap profiler (`dhat-heap`), which this stable-toolchain
//! workspace does not enable — so the guard-ordering tests below are the
//! regression pin instead.

use divan as _;
use image as _;
use jpeg_decoder as _;
#[cfg(feature = "logging")]
use log as _;
#[cfg(feature = "cache")]
use lru as _;
use proptest as _;
use thiserror as _;

use ithmb_core::enc::build_ithmb_file;
use ithmb_core::error::DecodeError;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::{decode_ithmb, decode_with_profile};
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// CWE-400 fixture — 193-byte progressive JPEG declaring 65535×65535
// ---------------------------------------------------------------------------

/// Builds a minimal progressive JPEG (`SOF2`) declaring 65535×65535 — the
/// CWE-400 regression fixture, byte-identical to the artifact verified by the
/// security-research PoC engineers (mirrors `src/jpeg.rs`'s unit-test builder).
fn huge_progressive_jpeg() -> Vec<u8> {
    fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(payload.len() + 4);
        out.extend_from_slice(&[0xFF, marker]);
        let seg_len = u16::try_from(payload.len() + 2).expect("test segment fits u16");
        out.extend_from_slice(&seg_len.to_be_bytes());
        out.extend_from_slice(payload);
        out
    }
    // DQT: precision 0, tables 0 and 1, all elements nonzero.
    let mut dqt = vec![0x00];
    dqt.extend_from_slice(&[0x01; 64]);
    dqt.push(0x01);
    dqt.extend_from_slice(&[0x01; 64]);
    let dqt = segment(0xDB, &dqt);
    // SOF2 (progressive): prec=8, H=65535, W=65535, 3 components.
    let sof2 = segment(
        0xC2,
        &[8, 0xFF, 0xFF, 0xFF, 0xFF, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1],
    );
    // DHT: one DC table (class 0, index 1), one symbol.
    let mut huff = vec![0x01, 0x01];
    huff.extend_from_slice(&[0x00; 15]);
    huff.push(0x00);
    let huff = segment(0xC4, &huff);
    // SOS: DC-only progressive first scan.
    let sos = segment(0xDA, &[3, 1, 0x10, 2, 0x10, 3, 0x10, 0, 0, 0]);
    let mut jpeg = vec![0xFF, 0xD8];
    jpeg.extend_from_slice(&dqt);
    jpeg.extend_from_slice(&sof2);
    jpeg.extend_from_slice(&huff);
    jpeg.extend_from_slice(&sos);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);
    jpeg
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The CVE-400 fix, pinned at the public API: `decode_ithmb` must reject the
/// 65535×65535 progressive JPEG **before** allocating the ~8 GiB coefficient
/// buffer (which would otherwise abort the process).
#[test]
fn decode_ithmb_rejects_oversized_jpeg_before_allocation() {
    let jpeg = huge_progressive_jpeg();
    assert_eq!(jpeg.len(), 193, "fixture drifted from the verified artifact");

    let canceled = AtomicBool::new(false);
    // JPEG SOI (FF D8) routes through the embedded-JPEG path of decode_ithmb.
    let result = decode_ithmb(&jpeg, &canceled);
    assert!(
        matches!(result, Err(DecodeError::Jpeg(ref msg)) if msg.contains("exceed")),
        "expected dimension-budget rejection, got {result:?}",
    );
}

/// Raw decoders must reject dimension products that imply an absurd output
/// buffer **before** allocating it. `i32::MAX × i32::MAX × 4` bytes is ~18.4 EB
/// — if validation regressed, this test would attempt that allocation and be
/// killed. On 64-bit the guard returns `BufferTooShort`; on 32-bit the
/// `checked_mul` overflows and returns `InvalidFormat("dimensions too large")`.
#[test]
fn decode_with_profile_rejects_absurd_dimensions_before_allocation() {
    let canceled = AtomicBool::new(false);
    // Any 4 bytes satisfy the prefix-strip step; the frame payload is empty.
    let data = [0u8; 4];

    for encoding in [Encoding::Rgb565, Encoding::Yuv422, Encoding::Ycbcr420] {
        let profile = Profile {
            prefix: 9999,
            width: i32::MAX,
            height: i32::MAX,
            encoding,
            ..Default::default()
        };
        let result = decode_with_profile(&data, &profile, &canceled);
        assert!(
            matches!(
                result,
                Err(DecodeError::BufferTooShort { .. }) | Err(DecodeError::InvalidFormat(_))
            ),
            "{encoding:?}: expected pre-allocation rejection, got {result:?}",
        );
    }
}

/// A profile parsed from JSON with an absurd `frame_byte_length` (i32::MAX)
/// must not drive allocation on the decode path: decode sizing comes from the
/// validated `width`/`height`, never from `frame_byte_length`.
#[test]
fn parsed_profile_with_huge_frame_byte_length_does_not_allocate() {
    let json = r#"[{"prefix":9999,"width":16,"height":16,"encoding":"rgb565","frame_byte_length":2147483647}]"#;
    let profiles = ithmb_core::profile_parser::parse_profiles_json(json).expect("profile JSON must parse");
    let profile = &profiles[0];
    assert_eq!(profile.frame_byte_length, i32::MAX);

    // Build a valid 16×16 RGB565 file and decode with the hostile profile.
    let bgra = vec![0u8; 16 * 16 * 4];
    let file = build_ithmb_file(&bgra, 16, 16, profile);
    let canceled = AtomicBool::new(false);
    let decoded = decode_with_profile(&file, profile, &canceled).expect("decode sizing must ignore frame_byte_length");
    assert_eq!(decoded.width, 16);
    assert_eq!(decoded.height, 16);
    // Decode output is black with alpha forced to 255 (decoder invariant).
    assert!(decoded.data.chunks_exact(4).all(|c| c[..3] == [0, 0, 0] && c[3] == 255));
}
