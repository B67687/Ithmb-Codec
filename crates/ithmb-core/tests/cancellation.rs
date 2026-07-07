#![allow(
    unused_crate_dependencies,
    clippy::match_same_arms,
    clippy::borrow_interior_mutable_const,
    clippy::declare_interior_mutable_const
)]
//! Cancellation tests for decoder operations.
//!
//! These tests verify that the cancellation mechanism (`&AtomicBool`) is properly
//! checked during decode operations across raw pixel formats and JPEG decoding.
//!
//! Coverage:
//!   1. `canceled_before_decode` — Cancel flag set **before** calling decode;
//!      expects `DecodeError::Canceled`.
//!   2. `canceled_mid_decode_raw` — Spawn a decode thread for a large raw image,
//!      set the flag after a short delay, accept either `Ok` or `Canceled`.
//!   3. `jpeg_decode_canceled` — Cancel flag set before JPEG decode; expects
//!      `DecodeError::Canceled`.
//!   4. `jpeg_canceled_mid_decode` — Cancel JPEG mid-decode (small image may
//!      finish before the flag is polled — accept both outcomes).
//!   5. `canceled_mid_decode_every_raw_format` — Every raw format tested with
//!      cancellation flag set before decode.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use divan as _;
use ithmb_core::DecodeError;
use ithmb_core::enc::*;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use jpeg_decoder as _;
use thiserror as _;

mod util;

struct FormatCase {
    name: &'static str,
    profile: Profile,
    raw_encoded: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Prepend the 4-byte profile prefix to an encoded frame.
fn prefix_buf(profile: &Profile, encoded: &[u8]) -> Vec<u8> {
    let mut buf = profile.prefix.to_be_bytes().to_vec();
    buf.extend(encoded);
    buf
}

/// Build a valid RGB565 fixture of size `w × h` with neutral gray pixels.
#[allow(clippy::cast_sign_loss)]
fn make_rgb565_fixture(w: i32, h: i32) -> (Profile, Vec<u8>) {
    let (wu, hu) = (w as usize, h as usize);
    let bgra = vec![128u8; wu * hu * 4];
    let profile = Profile {
        prefix: 9999,
        width: w,
        height: h,
        encoding: Encoding::Rgb565,
        frame_byte_length: w * h * 2,
        ..Default::default()
    };
    // encode_rgb565(bgra, w, h, big_endian); false = little-endian
    let encoded = encode_rgb565(&bgra, w, h, false);
    let prefixed = prefix_buf(&profile, &encoded);
    (profile, prefixed)
}

// ---------------------------------------------------------------------------
// Test 1: Cancel flag set before decode → Canceled
// ---------------------------------------------------------------------------

#[test]
fn canceled_before_decode() {
    let (profile, buf) = make_rgb565_fixture(2, 2);
    let canceled = AtomicBool::new(true);
    let result = decode_with_profile(&buf, &profile, &canceled);
    assert!(
        matches!(result, Err(DecodeError::Canceled(_))),
        "expected Canceled, got {result:?}",
    );
}

// ---------------------------------------------------------------------------
// Test 2: Cancel flag set mid-decode for a large raw image
// ---------------------------------------------------------------------------

#[test]
fn canceled_mid_decode_raw() {
    // 512 × 512 × 2 = 512 KB of pixel data — enough for cancellation polling.
    let (profile, buf) = make_rgb565_fixture(512, 512);
    let buf = Arc::new(buf);

    std::thread::scope(|s| {
        let canceled = Arc::new(AtomicBool::new(false));

        // Decode thread
        let buf = Arc::clone(&buf);
        let flag = Arc::clone(&canceled);
        s.spawn(move || {
            let result = decode_with_profile(&buf, &profile, &flag);
            let is_ok = result.is_ok();
            let is_canceled = matches!(&result, Err(DecodeError::Canceled(_)));
            assert!(is_ok || is_canceled, "expected Canceled or Ok, got {result:?}",);
        });

        // Brief delay, then cancel.
        std::thread::sleep(std::time::Duration::from_millis(1));
        canceled.store(true, Ordering::SeqCst);
    });
}

// ---------------------------------------------------------------------------
// Test 3: JPEG decode canceled before start
// ---------------------------------------------------------------------------

#[test]
fn jpeg_decode_canceled() {
    let jpeg_data = include_bytes!("fixtures/jpeg_solid_white_2x2.enc");
    let profile = Profile {
        encoding: Encoding::Jpeg,
        ..Default::default()
    };
    let canceled = AtomicBool::new(true);
    let result = decode_with_profile(jpeg_data, &profile, &canceled);
    assert!(
        matches!(result, Err(DecodeError::Canceled(_))),
        "expected Canceled, got {result:?}",
    );
}

// ---------------------------------------------------------------------------
// Test 4: JPEG canceled mid-decode (small image — accept both outcomes)
// ---------------------------------------------------------------------------

#[test]
fn jpeg_canceled_mid_decode() {
    // The 2×2 JPEG is small — it may decode fully before the flag is set.
    // Accept both Ok (completed before cancel) and Err(Canceled).
    let jpeg_data = include_bytes!("fixtures/jpeg_solid_white_2x2.enc");
    let profile = Profile {
        encoding: Encoding::Jpeg,
        ..Default::default()
    };

    std::thread::scope(|s| {
        let canceled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&canceled);

        s.spawn(move || {
            let result = decode_with_profile(jpeg_data, &profile, &flag);
            let is_ok = result.is_ok();
            let is_canceled = matches!(&result, Err(DecodeError::Canceled(_)));
            assert!(is_ok || is_canceled, "expected Canceled or Ok, got {result:?}",);
        });

        std::thread::sleep(std::time::Duration::from_millis(1));
        canceled.store(true, Ordering::SeqCst);
    });
}

// ---------------------------------------------------------------------------
// Test 5: Every raw format — canceled before decode
// ---------------------------------------------------------------------------

#[test]
fn canceled_mid_decode_every_raw_format() {
    let bgra: Vec<u8> = vec![
        0, 0, 255, 255, // red
        0, 255, 0, 255, // green
        255, 0, 0, 255, // blue
        255, 255, 255, 255, // white
    ];
    let (w, h) = (2, 2);

    let cases = [
        FormatCase {
            name: "rgb565",
            profile: Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Rgb565,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            raw_encoded: encode_rgb565(&bgra, w, h, false),
        },
        FormatCase {
            name: "rgb555",
            profile: Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Rgb555,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            raw_encoded: encode_rgb555(&bgra, w, h, false, false),
        },
        FormatCase {
            name: "uyvy",
            profile: Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Yuv422,
                frame_byte_length: w * h * 2,
                ..Default::default()
            },
            raw_encoded: encode_uyvy(&bgra, w, h),
        },
        FormatCase {
            name: "ycbcr420",
            profile: Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::Ycbcr420,
                frame_byte_length: w * h * 2,
                ..Default::default()
            },
            raw_encoded: encode_ycbcr420(&bgra, w, h, false),
        },
        FormatCase {
            name: "reordered_rgb555",
            profile: Profile {
                prefix: 9999,
                width: w,
                height: h,
                encoding: Encoding::ReorderedRgb555,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            raw_encoded: encode_reordered_rgb555(&bgra, w, h, false),
        },
    ];

    for case in &cases {
        let buf = prefix_buf(&case.profile, &case.raw_encoded);

        // Cancel before decode — expect Canceled.
        let canceled = AtomicBool::new(true);
        let result = decode_with_profile(&buf, &case.profile, &canceled);
        assert!(
            matches!(result, Err(DecodeError::Canceled(_))),
            "{}: expected Canceled, got {result:?}",
            case.name,
        );

        // Also verify that decode succeeds without cancellation.
        let no_cancel = AtomicBool::new(false);
        let ok_result = decode_with_profile(&buf, &case.profile, &no_cancel);
        assert!(
            ok_result.is_ok(),
            "{}: decode should succeed without cancellation: {ok_result:?}",
            case.name,
        );
    }
}
