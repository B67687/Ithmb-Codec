// SPDX-License-Identifier: MIT
//! Benchmarks for all 8 pixel decoders at multiple sizes with diverse input
//! patterns.  Each benchmark encodes 4 variants (checkerboard / random /
//! gradient / solid white) into the target format and measures aggregate
//! decode throughput — encoding time is excluded from the measurement.

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::profile::Encoding;
use ithmb_core::{cl, clcl, enc, jpeg, reordered_rgb555, rgb555, rgb565, uyvy, ycbcr420};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use util::{CHECKERBOARD_JPEG_64, all_inputs, make_profile, never_canceled};

// ---------------------------------------------------------------------------
// Benchmark sizes
// ---------------------------------------------------------------------------

const SIZES: &[(usize, usize)] = &[(64, 64), (256, 256), (512, 512), (720, 480)];

// ---------------------------------------------------------------------------
// JPEG — single-size only (embedded fixture is fixed at 64×64)
// ---------------------------------------------------------------------------

const JPEG_W: usize = 64;
const JPEG_H: usize = 64;
const JPEG_OUTPUT_BYTES: u64 = (JPEG_W * JPEG_H * 4) as u64;

#[divan::bench]
fn decode_jpeg(bencher: divan::Bencher) {
    let src = CHECKERBOARD_JPEG_64;
    let profile = make_profile(JPEG_W as i32, JPEG_H as i32, Encoding::Jpeg);
    let canceled = never_canceled();
    bencher
        .counter(BytesCount::new(JPEG_OUTPUT_BYTES))
        .with_inputs(|| (src, &profile, &canceled))
        .bench_refs(|(src, profile, canceled)| {
            let _ = divan::black_box(jpeg::decode(src, profile, canceled));
        });
}

// ---------------------------------------------------------------------------
// Macro: generate a multi-size decoder bench with input diversity
// ---------------------------------------------------------------------------

macro_rules! bench_decoder {
    ($name:ident, $encode:expr, $encoding:expr, $decoder:expr $(, $extra_profile:expr)?) => {
        #[divan::bench(args = SIZES)]
        fn $name(bencher: divan::Bencher, (w, h): &(usize, usize)) {
            let inputs: Vec<_> = all_inputs(*w, *h).into_iter().map(|(_name, bgra)| {
                let encoded = $encode(&bgra, *w as i32, *h as i32);
                let mut profile = make_profile(*w as i32, *h as i32, $encoding);
                $( $extra_profile(&mut profile); )?
                profile.frame_byte_length = encoded.len() as i32;
                (encoded, profile)
            }).collect();
            let input_count = inputs.len();
            let counter = BytesCount::new((w * h * 4) as u64);
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            bencher
                .counter(counter)
                .with_inputs(|| {
                    COUNTER.fetch_add(1, Ordering::Relaxed) % input_count
                })
                .bench_refs(|i| {
                    let (ref src, ref profile) = inputs[*i];
                    let canceled = AtomicBool::new(false);
                    let _ = divan::black_box($decoder(src, profile, &canceled));
                });
        }
    };
}

bench_decoder!(
    decode_rgb565,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb565(bgra, w, h, false),
    Encoding::Rgb565,
    rgb565::decode
);

bench_decoder!(
    decode_rgb555,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb555(bgra, w, h, false, false),
    Encoding::Rgb555,
    rgb555::decode
);

bench_decoder!(
    decode_reordered_rgb555,
    |bgra: &[u8], w: i32, h: i32| enc::encode_reordered_rgb555(bgra, w, h, true),
    Encoding::ReorderedRgb555,
    reordered_rgb555::decode
);

bench_decoder!(
    decode_uyvy,
    |bgra: &[u8], w: i32, h: i32| enc::encode_uyvy(bgra, w, h),
    Encoding::Yuv422,
    uyvy::decode
);

bench_decoder!(
    decode_ycbcr420,
    |bgra: &[u8], w: i32, h: i32| enc::encode_ycbcr420(bgra, w, h, false),
    Encoding::Ycbcr420,
    ycbcr420::decode
);

bench_decoder!(
    decode_clcl,
    |bgra: &[u8], w: i32, h: i32| enc::encode_clcl(bgra, w, h),
    Encoding::Yuv422,
    clcl::decode,
    |profile: &mut ithmb_core::profile::Profile| {
        profile.clcl_chroma = true;
    }
);

bench_decoder!(
    decode_cl,
    |bgra: &[u8], w: i32, h: i32| enc::encode_cl(bgra, w, h),
    Encoding::Yuv422,
    cl::decode,
    |profile: &mut ithmb_core::profile::Profile| {
        profile.cl_chroma = true;
    }
);

fn main() {
    // -----------------------------------------------------------------------
    // SIMD row benches (single-size 256×256 — direct dispatch micro-bench)
    // -----------------------------------------------------------------------

    #[divan::bench]
    fn simd_rgb565_row_to_bgra(bencher: divan::Bencher) {
        let bgra = util::bgra_checkerboard(256, 256);
        let encoded = enc::encode_rgb565(&bgra, 256, 256, false);
        bencher
            .counter(BytesCount::new((256 * 256 * 4) as u64))
            .with_inputs(|| &encoded)
            .bench_refs(|src| {
                let _ = divan::black_box(ithmb_core::simd::rgb565_row_to_bgra(src));
            });
    }

    #[divan::bench]
    fn simd_rgb555_row_to_bgra(bencher: divan::Bencher) {
        let bgra = util::bgra_checkerboard(256, 256);
        let encoded = enc::encode_rgb555(&bgra, 256, 256, false, false);
        bencher
            .counter(BytesCount::new((256 * 256 * 4) as u64))
            .with_inputs(|| &encoded)
            .bench_refs(|src| {
                let _ = divan::black_box(ithmb_core::simd::rgb555_row_to_bgra(src));
            });
    }
    divan::main();
}
