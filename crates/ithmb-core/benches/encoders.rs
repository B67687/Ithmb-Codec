// SPDX-License-Identifier: MIT
//! Benchmarks for all 7 pixel encoders.
//!
//! Each encoder is benchmarked at three sizes: 64×64, 256×256, 512×512.
//! The main benchmarks cycle through 4 input patterns (checkerboard, random,
//! gradient, solid).  The `_be` variants use only checkerboard (original
//! behavior).  Throughput is reported as MB/s of the *encoded* byte stream
//! produced.

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::enc;
use std::sync::atomic::{AtomicUsize, Ordering};
use util::{all_inputs, bgra_checkerboard};

// ---------------------------------------------------------------------------
// Benchmark sizes
// ---------------------------------------------------------------------------

const SIZES: &[(usize, usize)] = &[(64, 64), (256, 256), (512, 512)];

/// Encoded bytes for a 2 Bpp format: w × h × 2.
const fn bytes_2bpp(w: usize, h: usize) -> u64 {
    (w * h * 2) as u64
}

/// Encoded bytes for YCbCr 4:2:0: Y (w×h) + Cb + Cr (each `ceil(w/2)*ceil(h/2)`).
const fn bytes_ycbcr420(w: usize, h: usize) -> u64 {
    let y = w * h;
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    (y + uv_w * uv_h * 2) as u64
}

// ---------------------------------------------------------------------------
// Macro: generate a multi-pattern encoder bench (4 input variants)
// ---------------------------------------------------------------------------

macro_rules! bench_encoder {
    ($name:ident, $encode:expr, $counter_fn:expr) => {
        #[divan::bench(args = SIZES)]
        fn $name(bencher: divan::Bencher, (w, h): &(usize, usize)) {
            let inputs = all_inputs(*w, *h);
            let input_count = inputs.len();
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            bencher
                .counter(BytesCount::new($counter_fn(*w, *h)))
                .with_inputs(|| COUNTER.fetch_add(1, Ordering::Relaxed) % input_count)
                .bench_refs(|i| {
                    let (_name, ref bgra) = inputs[*i];
                    let _ = divan::black_box($encode(bgra, *w as i32, *h as i32));
                });
        }
    };
}

// ---------------------------------------------------------------------------
// Macro: generate a checkerboard-only encoder bench (_be variant)
// ---------------------------------------------------------------------------

macro_rules! bench_encoder_be {
    ($name:ident, $encode:expr, $counter_fn:expr) => {
        #[divan::bench(args = SIZES)]
        fn $name(bencher: divan::Bencher, (w, h): &(usize, usize)) {
            let bgra = bgra_checkerboard(*w, *h);
            bencher
                .counter(BytesCount::new($counter_fn(*w, *h)))
                .with_inputs(|| bgra.clone())
                .bench_values(|bgra| divan::black_box($encode(&bgra, *w as i32, *h as i32)));
        }
    };
}

// ---------------------------------------------------------------------------
// Encoders — multi-pattern (all 4 inputs)
// ---------------------------------------------------------------------------

bench_encoder!(
    encode_rgb565,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb565(bgra, w, h, false),
    bytes_2bpp
);

bench_encoder!(
    encode_rgb555,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb555(bgra, w, h, false, false),
    bytes_2bpp
);

bench_encoder!(
    encode_reordered_rgb555,
    |bgra: &[u8], w: i32, h: i32| enc::encode_reordered_rgb555(bgra, w, h, true),
    bytes_2bpp
);

bench_encoder!(
    encode_uyvy,
    |bgra: &[u8], w: i32, h: i32| enc::encode_uyvy(bgra, w, h),
    bytes_2bpp
);

bench_encoder!(
    encode_ycbcr420,
    |bgra: &[u8], w: i32, h: i32| enc::encode_ycbcr420(bgra, w, h, false),
    bytes_ycbcr420
);

bench_encoder!(
    encode_clcl,
    |bgra: &[u8], w: i32, h: i32| enc::encode_clcl(bgra, w, h),
    bytes_2bpp
);

bench_encoder!(
    encode_cl,
    |bgra: &[u8], w: i32, h: i32| enc::encode_cl(bgra, w, h),
    bytes_2bpp
);

// ---------------------------------------------------------------------------
// Encoders — checkerboard-only (_be variants)
// ---------------------------------------------------------------------------

bench_encoder_be!(
    encode_rgb565_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb565(bgra, w, h, false),
    bytes_2bpp
);

bench_encoder_be!(
    encode_rgb555_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_rgb555(bgra, w, h, false, false),
    bytes_2bpp
);

bench_encoder_be!(
    encode_reordered_rgb555_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_reordered_rgb555(bgra, w, h, true),
    bytes_2bpp
);

bench_encoder_be!(
    encode_uyvy_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_uyvy(bgra, w, h),
    bytes_2bpp
);

bench_encoder_be!(
    encode_ycbcr420_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_ycbcr420(bgra, w, h, false),
    bytes_ycbcr420
);

bench_encoder_be!(
    encode_clcl_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_clcl(bgra, w, h),
    bytes_2bpp
);

bench_encoder_be!(
    encode_cl_be,
    |bgra: &[u8], w: i32, h: i32| enc::encode_cl(bgra, w, h),
    bytes_2bpp
);

fn main() {
    divan::main();
}
