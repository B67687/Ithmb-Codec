//! Multi-threaded decode concurrency stress tests for ithmb-core.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    unused_crate_dependencies,
    unused_extern_crates,
    clippy::pedantic,
    clippy::unwrap_used
)]

use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};

use ithmb_core::enc::*;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::{DecodeError, DecodedImage};
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use thiserror as _;

fn prefix_buf(profile: &Profile, encoded: &[u8]) -> Vec<u8> {
    let mut buf = profile.prefix.to_be_bytes().to_vec();
    buf.extend(encoded);
    buf
}

fn decode_buf(buf: &[u8], profile: &Profile, canceled: &AtomicBool) -> DecodedImage {
    decode_with_profile(buf, profile, canceled).expect("decode_with_profile should succeed")
}

fn roundtrip_expected(profile: &Profile, encoded: &[u8]) -> Vec<u8> {
    let buf = prefix_buf(profile, encoded);
    decode_buf(&buf, profile, &AtomicBool::new(false)).data
}

struct FormatFixture {
    profile: Profile,
    raw_encoded: Vec<u8>,
    expected_bgra: Vec<u8>,
}

fn make_fixture(profile: Profile, raw_encoded: Vec<u8>) -> FormatFixture {
    let expected_bgra = roundtrip_expected(&profile, &raw_encoded);
    FormatFixture {
        profile,
        raw_encoded,
        expected_bgra,
    }
}

fn all_format_fixtures() -> Vec<FormatFixture> {
    let bgra: Vec<u8> = vec![0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255];
    let (w, h) = (2, 2);

    vec![
        make_fixture(
            Profile {
                prefix: 0x1000_0001,
                width: w,
                height: h,
                encoding: Encoding::Rgb565,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            encode_rgb565(&bgra, w, h, false),
        ),
        make_fixture(
            Profile {
                prefix: 0x1000_0001,
                width: w,
                height: h,
                encoding: Encoding::Rgb555,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            encode_rgb555(&bgra, w, h, false, false),
        ),
        make_fixture(
            Profile {
                prefix: 0x1000_0001,
                width: w,
                height: h,
                encoding: Encoding::Yuv422,
                frame_byte_length: w * h * 2,
                ..Default::default()
            },
            encode_uyvy(&bgra, w, h),
        ),
        make_fixture(
            Profile {
                prefix: 0x1000_0001,
                width: w,
                height: h,
                encoding: Encoding::Ycbcr420,
                frame_byte_length: w * h * 2,
                ..Default::default()
            },
            encode_ycbcr420(&bgra, w, h, false),
        ),
        make_fixture(
            Profile {
                prefix: 0x1000_0001,
                width: w,
                height: h,
                encoding: Encoding::ReorderedRgb555,
                frame_byte_length: w * h * 2,
                little_endian: true,
                ..Default::default()
            },
            encode_reordered_rgb555(&bgra, w, h, false),
        ),
    ]
}

fn num_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(4, NonZero::<usize>::get)
        .max(4)
}

#[test]
fn concurrent_decode_all_formats() {
    let fixtures = all_format_fixtures();
    let buf_refs: Vec<(&Profile, Vec<u8>, Vec<u8>)> = fixtures
        .iter()
        .map(|f| {
            (
                &f.profile,
                prefix_buf(&f.profile, &f.raw_encoded),
                f.expected_bgra.clone(),
            )
        })
        .collect();

    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for (profile, buf_data, expected) in &buf_refs {
            handles.push(s.spawn(move || {
                let canceled = AtomicBool::new(false);
                let img = decode_buf(buf_data, profile, &canceled);
                assert_eq!(img.data, expected.as_slice());
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    });
}

#[test]
fn concurrent_decode_same_format() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[0];
    let buf = Arc::new(prefix_buf(&f.profile, &f.raw_encoded));
    let expected = Arc::new(f.expected_bgra.clone());
    let n = num_threads();

    std::thread::scope(|s| {
        for _ in 0..n {
            let buf = Arc::clone(&buf);
            let expected = Arc::clone(&expected);
            s.spawn(move || {
                let canceled = AtomicBool::new(false);
                let img = decode_buf(&buf, &f.profile, &canceled);
                assert_eq!(img.data, *expected);
            });
        }
    });
}

#[test]
fn concurrent_decode_barrier_sync() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[0];
    let buf = Arc::new(prefix_buf(&f.profile, &f.raw_encoded));
    let expected = Arc::new(f.expected_bgra.clone());
    let n = num_threads();
    let barrier = Arc::new(Barrier::new(n));

    std::thread::scope(|s| {
        for _ in 0..n {
            let buf = Arc::clone(&buf);
            let expected = Arc::clone(&expected);
            let barrier = Arc::clone(&barrier);
            s.spawn(move || {
                barrier.wait();
                let canceled = AtomicBool::new(false);
                let img = decode_buf(&buf, &f.profile, &canceled);
                assert_eq!(img.data, *expected);
            });
        }
    });
}

#[test]
fn concurrent_decode_shared_cancellation() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[0];
    let buf = Arc::new(prefix_buf(&f.profile, &f.raw_encoded));
    let expected_len = f.expected_bgra.len();
    let n = num_threads().max(6);
    let half = n / 2;
    let canceled = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new(n));

    std::thread::scope(|s| {
        for i in 0..n {
            let buf = Arc::clone(&buf);
            let canceled = Arc::clone(&canceled);
            let barrier = Arc::clone(&barrier);
            s.spawn(move || {
                barrier.wait();
                if i < half {
                    std::hint::spin_loop();
                    canceled.store(true, Ordering::SeqCst);
                }
                let result = decode_with_profile(&buf, &f.profile, &canceled);
                match result {
                    Ok(img) => {
                        assert_eq!(img.data.len(), expected_len);
                    }
                    Err(e) => {
                        assert!(matches!(e, DecodeError::Canceled(_)));
                    }
                }
            });
        }
    });
}

#[test]
fn canceled_before_decode() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[0];
    let buf = prefix_buf(&f.profile, &f.raw_encoded);
    let canceled = AtomicBool::new(true);
    let result = decode_with_profile(&buf, &f.profile, &canceled);
    assert!(matches!(result, Err(DecodeError::Canceled(_))));
}

#[test]
fn concurrent_repeated_decode() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[2];
    let buf = Arc::new(prefix_buf(&f.profile, &f.raw_encoded));
    let expected = Arc::new(f.expected_bgra.clone());
    let n = num_threads().min(8);

    std::thread::scope(|s| {
        for _ in 0..n {
            let buf = Arc::clone(&buf);
            let expected = Arc::clone(&expected);
            s.spawn(move || {
                for _ in 0..10 {
                    let canceled = AtomicBool::new(false);
                    let img = decode_buf(&buf, &f.profile, &canceled);
                    assert_eq!(img.data, *expected);
                }
            });
        }
    });
}

#[test]
fn sequential_baseline_vs_concurrent() {
    let fixtures = all_format_fixtures();
    let f = &fixtures[0];
    let buf = Arc::new(prefix_buf(&f.profile, &f.raw_encoded));
    let expected = Arc::new(f.expected_bgra.clone());
    let n = num_threads().min(8);

    for _ in 0..n {
        let canceled = AtomicBool::new(false);
        let img = decode_buf(&buf, &f.profile, &canceled);
        assert_eq!(img.data, *expected);
    }

    std::thread::scope(|s| {
        for _ in 0..n {
            let buf = Arc::clone(&buf);
            let expected = Arc::clone(&expected);
            s.spawn(move || {
                let canceled = AtomicBool::new(false);
                let img = decode_buf(&buf, &f.profile, &canceled);
                assert_eq!(img.data, *expected);
            });
        }
    });
}

#[test]
fn concurrent_reordered_rgb555() {
    let (w, h) = (4, 4);
    let mut bgra = Vec::with_capacity(64);
    for i in 0..16 {
        let (x, y) = (i % 4, i / 4);
        match (y < 2, x < 2) {
            (true, true) => {
                bgra.push(255);
                bgra.push(0);
                bgra.push(0);
                bgra.push(255);
            }
            (true, false) => {
                bgra.push(0);
                bgra.push(255);
                bgra.push(0);
                bgra.push(255);
            }
            (false, true) => {
                bgra.push(0);
                bgra.push(0);
                bgra.push(255);
                bgra.push(255);
            }
            (false, false) => {
                bgra.push(255);
                bgra.push(255);
                bgra.push(255);
                bgra.push(255);
            }
        }
    }
    let bgra = bgra;

    let profile = Arc::new(Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: w * h * 2,
        little_endian: true,
        ..Default::default()
    });

    let raw_encoded = encode_reordered_rgb555(&bgra, w, h, false);
    let buf = Arc::new(prefix_buf(&profile, &raw_encoded));
    let expected = Arc::new({
        let buf = prefix_buf(&profile, &raw_encoded);
        decode_buf(&buf, &profile, &AtomicBool::new(false)).data
    });
    let n = num_threads().min(8);

    std::thread::scope(|s| {
        for _ in 0..n {
            let buf = Arc::clone(&buf);
            let expected_arc = Arc::clone(&expected);
            let profile = Arc::clone(&profile);
            s.spawn(move || {
                let canceled = AtomicBool::new(false);
                let img = decode_with_profile(&buf, &profile, &canceled).expect("decode should succeed");
                assert_eq!(img.data, *expected_arc);
            });
        }
    });
}

#[test]
fn concurrent_ycbcr420() {
    let (w, h) = (4, 2);
    let bgra: Vec<u8> = vec![
        0, 0, 255, 255, 0, 0, 255, 255, 0, 255, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 255, 255,
        255, 255, 255, 255, 255,
    ];
    let profile = Arc::new(Profile {
        prefix: 0x1000_0001,
        width: w,
        height: h,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: w * h * 2,
        ..Default::default()
    });
    let raw_encoded = encode_ycbcr420(&bgra, w, h, false);
    let buf = Arc::new(prefix_buf(&profile, &raw_encoded));
    let expected = Arc::new({
        let buf = prefix_buf(&profile, &raw_encoded);
        decode_buf(&buf, &profile, &AtomicBool::new(false)).data
    });
    let n = num_threads().min(8);

    std::thread::scope(|s| {
        for _ in 0..n {
            let expected_arc = Arc::clone(&expected);
            let profile = Arc::clone(&profile);
            let buf = Arc::clone(&buf);
            s.spawn(move || {
                let canceled = AtomicBool::new(false);
                let img = decode_with_profile(&buf, &profile, &canceled).expect("decode should succeed");
                assert_eq!(img.data, *expected_arc);
            });
        }
    });
}

#[test]
fn sequential_vs_concurrent_all_formats() {
    let fixtures = all_format_fixtures();

    let seq: Vec<Vec<u8>> = fixtures
        .iter()
        .map(|f| {
            let buf = prefix_buf(&f.profile, &f.raw_encoded);
            let canceled = AtomicBool::new(false);
            decode_buf(&buf, &f.profile, &canceled).data
        })
        .collect();

    let con: Vec<Vec<u8>> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for f in &fixtures {
            let buf = prefix_buf(&f.profile, &f.raw_encoded);
            handles.push(s.spawn(move || {
                let canceled = AtomicBool::new(false);
                decode_buf(&buf, &f.profile, &canceled).data
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (i, _) in fixtures.iter().enumerate() {
        assert_eq!(con[i], seq[i], "concurrent differs from sequential");
    }
}

#[test]
fn canceled_mid_decode_every_format() {
    let fixtures = all_format_fixtures();
    for f in &fixtures {
        let buf = prefix_buf(&f.profile, &f.raw_encoded);
        let canceled = AtomicBool::new(true);
        let result = decode_with_profile(&buf, &f.profile, &canceled);
        if !matches!(result, Err(DecodeError::Canceled(_))) {
            assert!(result.is_ok(), "expected Canceled or Ok, got {result:?}");
        }
    }
}
