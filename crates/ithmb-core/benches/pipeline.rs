// SPDX-License-Identifier: MIT
//! Benchmarks for the full pipeline: `decode_ithmb`, `build_ithmb_file`, and
//! `open_ithmb` (PhotoDB).

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::enc;
use ithmb_core::photodb::builder::{BuildEntry, try_build_photodb};
use ithmb_core::pipeline::open_ithmb;
use ithmb_core::profile::Encoding;
use ithmb_core::profile_db::ProfileDb;
use util::{bgra_checkerboard, make_profile, never_canceled};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const W_256: usize = 256;
const H_256: usize = 256;
const W_512: usize = 512;
const H_512: usize = 512;

const OUTPUT_BYTES_256: u64 = (W_256 * H_256 * 4) as u64;
const OUTPUT_BYTES_512: u64 = (W_512 * H_512 * 4) as u64;

/// Known profile ID for 50×50 RGB565 big-endian (used for PhotoDB bench).
const PHOTODB_FORMAT_ID: i32 = 2002;

/// Profile prefix used for `decode_with_profile` benchmarks.
const DECODE_PREFIX: i32 = 9999;

// ---------------------------------------------------------------------------
// decode_ithmb — raw blob decode.
//
// Uses `decode_with_profile` with a custom profile (prefix 9999) rather
// than a built-in DB lookup, so we can benchmark arbitrary sizes.
// ---------------------------------------------------------------------------

fn make_decode_ithmb_buf(w: usize, h: usize) -> Vec<u8> {
    let bgra = bgra_checkerboard(w, h);
    let encoded = enc::encode_rgb565(&bgra, w as i32, h as i32, false);
    let prefix = DECODE_PREFIX.to_be_bytes();
    let mut buf = Vec::with_capacity(4 + encoded.len());
    buf.extend_from_slice(&prefix);
    buf.extend_from_slice(&encoded);
    buf
}

#[divan::bench]
fn decode_ithmb_256(bencher: divan::Bencher) {
    let src = make_decode_ithmb_buf(W_256, H_256);
    let profile = make_profile(W_256 as i32, H_256 as i32, Encoding::Rgb565);
    let canceled = never_canceled();
    bencher
        .counter(BytesCount::new(OUTPUT_BYTES_256))
        .with_inputs(|| (&src, &profile, &canceled))
        .bench_refs(|(src, profile, canceled)| {
            let _ = divan::black_box(ithmb_core::pipeline::decode_with_profile(src, profile, canceled));
        });
}

#[divan::bench]
fn decode_ithmb_512(bencher: divan::Bencher) {
    let src = make_decode_ithmb_buf(W_512, H_512);
    let profile = make_profile(W_512 as i32, H_512 as i32, Encoding::Rgb565);
    let canceled = never_canceled();
    bencher
        .counter(BytesCount::new(OUTPUT_BYTES_512))
        .with_inputs(|| (&src, &profile, &canceled))
        .bench_refs(|(src, profile, canceled)| {
            let _ = divan::black_box(ithmb_core::pipeline::decode_with_profile(src, profile, canceled));
        });
}

// ---------------------------------------------------------------------------
// build_ithmb_file — end-to-end encode pipeline
// ---------------------------------------------------------------------------

#[divan::bench]
fn build_ithmb_file_256(bencher: divan::Bencher) {
    let bgra = bgra_checkerboard(W_256, H_256);
    let profile = make_profile(W_256 as i32, H_256 as i32, Encoding::Rgb565);
    bencher
        .counter(BytesCount::new(OUTPUT_BYTES_256))
        .with_inputs(|| (bgra.clone(), profile.clone()))
        .bench_values(|(bgra, profile)| {
            let _ = divan::black_box(enc::build_ithmb_file(&bgra, W_256 as i32, H_256 as i32, &profile));
        });
}

// ---------------------------------------------------------------------------
// open_ithmb — PhotoDB with 1 entry (format 2002, 50×50 RGB565 big-endian)
// ---------------------------------------------------------------------------

fn build_photodb_entry() -> Vec<u8> {
    let db = ProfileDb::load_builtin().unwrap();
    let profile = db.get(PHOTODB_FORMAT_ID).unwrap();
    let w = profile.width as usize;
    let h = profile.height as usize;
    let frame_len = profile.frame_byte_length as usize;

    let bgra = bgra_checkerboard(w, h);
    let mut encoded = enc::encode_rgb565(&bgra, w as i32, h as i32, !profile.little_endian);
    encoded.resize(frame_len, 0);

    let entry = BuildEntry {
        format_id: PHOTODB_FORMAT_ID,
        data: encoded,
    };

    // Use the same header/padding values as the builder's own tests.
    try_build_photodb(&[entry], 36, 40, false).unwrap()
}

#[divan::bench]
fn open_ithmb_photodb(bencher: divan::Bencher) {
    let photodb = build_photodb_entry();
    let canceled = never_canceled();
    bencher
        .counter(BytesCount::new(4096u64))
        .with_inputs(|| (&photodb, &canceled))
        .bench_refs(|(photodb, canceled)| {
            let _ = divan::black_box(open_ithmb(photodb, canceled, None));
        });
}

fn main() {
    divan::main();
}
