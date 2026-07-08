// SPDX-License-Identifier: MIT
//! Concurrency stress tests for the LRU [`CachedDecoder`].

#![allow(clippy::pedantic, clippy::unwrap_used, unused_crate_dependencies)]

use ithmb_core::cache::CachedDecoder;
use ithmb_core::enc::encode_rgb565;
use ithmb_core::profile::Profile;
use std::sync::atomic::AtomicBool;

fn make_pair(r: u8, g: u8, b: u8) -> (Profile, Vec<u8>) {
    let w = 4i32;
    let h = 4i32;
    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..(w * h) {
        bgra.push(b);
        bgra.push(g);
        bgra.push(r);
        bgra.push(255);
    }
    let encoded = encode_rgb565(&bgra, w, h, false);
    assert_eq!(encoded.len(), 32);
    let mut data = vec![0u8; 4]; // 4-byte prefix
    data.extend_from_slice(&encoded);
    let profile = Profile {
        width: w,
        height: h,
        frame_byte_length: encoded.len() as i32, // data part only
        ..Default::default()
    };
    (profile, data)
}

fn check(img: &ithmb_core::error::DecodedImage, p: &Profile) {
    assert_eq!(img.width, p.width as u32);
    assert_eq!(img.height, p.height as u32);
    assert_eq!(img.data.len() as u32, img.width * img.height * 4);
}

// ---------------------------------------------------------------------------
#[test]
fn concurrent_decode_same_data_16_threads() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    let (profile, data) = make_pair(255, 0, 0);
    std::thread::scope(|s| {
        for _ in 0..16 {
            s.spawn(|| {
                let img = p.decode_with_cache(&profile, &data, c).unwrap();
                check(&img, &profile);
            });
        }
    });
    assert_eq!(decoder.len(), 1);
}

#[test]
fn concurrent_decode_different_data_32_threads() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    std::thread::scope(|s| {
        for i in 0..32u8 {
            let (profile, data) = make_pair(i * 8, 128, 128);
            s.spawn(move || {
                let img = p.decode_with_cache(&profile, &data, c).unwrap();
                check(&img, &profile);
            });
        }
    });
    assert!(decoder.len() >= 28, "at least 28 distinct entries: {}", decoder.len());
}

#[test]
fn concurrent_decode_different_data_128_threads() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    std::thread::scope(|s| {
        for i in 0..128u8 {
            let (profile, data) = make_pair(i.wrapping_mul(37), i.wrapping_mul(73), i.wrapping_mul(11));
            s.spawn(move || {
                let img = p.decode_with_cache(&profile, &data, c).unwrap();
                check(&img, &profile);
            });
        }
    });
    let n = decoder.len();
    assert!(n <= 64, "LRU cap: {n}");
    assert!(n >= 16, "LRU survivors: {n}");
}

#[test]
fn concurrent_decode_shared_cancellation() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    std::thread::scope(|s| {
        for i in 0..16u8 {
            let (profile, data) = make_pair(i, 128, 255 - i);
            s.spawn(move || {
                let img = p.decode_with_cache(&profile, &data, c).unwrap();
                check(&img, &profile);
            });
        }
    });
}

#[test]
fn concurrent_decode_same_data_cache_hit_all() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    let (profile, data) = make_pair(42, 42, 42);
    check(&p.decode_with_cache(&profile, &data, c).unwrap(), &profile);
    std::thread::scope(|s| {
        for _ in 0..16 {
            s.spawn(|| {
                let img = p.decode_with_cache(&profile, &data, c).unwrap();
                check(&img, &profile);
            });
        }
    });
    assert_eq!(decoder.len(), 1);
}

#[test]
fn concurrent_decode_with_clear_interleaved() {
    let decoder = CachedDecoder::new();
    let cancel = AtomicBool::new(false);
    let p = &decoder;
    let c = &cancel;
    for i in 0..10u8 {
        let (profile, data) = make_pair(i, 200 - i, 100 + i);
        p.decode_with_cache(&profile, &data, c).ok();
    }
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| p.clear());
            let (profile, data) = make_pair(0, 255, 0);
            s.spawn(move || {
                let _ = p.decode_with_cache(&profile, &data, c);
            });
        }
    });
}
