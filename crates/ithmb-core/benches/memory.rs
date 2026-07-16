// SPDX-License-Identifier: MIT
//! Memory allocation benchmarks for all 8 pixel decoders at 512×512.
//!
//! Uses a custom [`#[global_allocator]`] to track allocation count, total bytes,
//! and peak heap usage per decode. Timing is reported via Divan benches;
//! allocation statistics are printed before the Divan output.
//!
//! # Global allocator design
//!
//! All calls to `alloc`/`dealloc`/`realloc` pass through a counting wrapper
//! around [`std::alloc::System`].  The counters are monotonically growing
//! across the process lifetime, but each measurement captures a difference
//! between pre- and post-snapshots so that only the allocations strictly
//! inside the measured decode are reported.
//!
//! Peak heap usage is tracked by incrementing a [`AtomicUsize`] high-water
//! mark on each allocation and resetting it to the current level immediately
//! before the measured call.

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    unsafe_code,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::{cl, clcl, enc, jpeg, reordered_rgb555, rgb555, rgb565, uyvy, ycbcr420};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Counting global allocator
// ---------------------------------------------------------------------------

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// Thin wrapper around [`std::alloc::System`] that atomically tracks every
/// allocation and deallocation.
struct CountingAlloc;

/// Number of `alloc` calls (including those from `realloc`).
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
/// Sum of the `size` arguments of every `alloc` / `realloc` call.
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Number of bytes currently live (allocated minus freed).  An [`AtomicIsize`]
/// so that the subtraction in `dealloc` can safely represent a transient dip
/// below zero when a pre-existing allocation is freed after a counter reset.
static CURRENT_ALLOC: AtomicIsize = AtomicIsize::new(0);
/// All-time high-water mark of `CURRENT_ALLOC` since the last explicit reset.
static PEAK_ALLOC: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        ALLOC_BYTES.fetch_add(size, Ordering::SeqCst);
        let prev = CURRENT_ALLOC.fetch_add(size as isize, Ordering::SeqCst);
        // Update the peak high-water mark
        let cur = (prev as usize).wrapping_add(size);
        PEAK_ALLOC.fetch_max(cur, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, layout: Layout) {
        CURRENT_ALLOC.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        unsafe { System.dealloc(_ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        ALLOC_BYTES.fetch_add(new_size, Ordering::SeqCst);
        if new_size > old_size {
            let added = new_size - old_size;
            let prev = CURRENT_ALLOC.fetch_add(added as isize, Ordering::SeqCst);
            let cur = (prev as usize).wrapping_add(added);
            PEAK_ALLOC.fetch_max(cur, Ordering::SeqCst);
        } else if new_size < old_size {
            CURRENT_ALLOC.fetch_sub((old_size - new_size) as isize, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

// ---------------------------------------------------------------------------
// Allocation measurement helpers
// ---------------------------------------------------------------------------

/// Snapshot the current counters and run `f`, returning the delta.
///
/// Peak heap is tracked by resetting the high-water mark to the current
/// live-byte count just before `f` runs, so the returned peak reflects
/// only allocations inside `f`.
fn measure_alloc<F>(f: F) -> (usize, usize, usize)
where
    F: FnOnce(),
{
    let pre_count = ALLOC_COUNT.load(Ordering::SeqCst);
    let pre_bytes = ALLOC_BYTES.load(Ordering::SeqCst);
    let pre_current = CURRENT_ALLOC.load(Ordering::SeqCst);
    // Reset the high-water mark to the current live-byte count so that
    // allocations made before this measurement are excluded.
    PEAK_ALLOC.store(pre_current as usize, Ordering::SeqCst);

    f();

    let count = ALLOC_COUNT.load(Ordering::SeqCst) - pre_count;
    let bytes = ALLOC_BYTES.load(Ordering::SeqCst) - pre_bytes;
    let peak = PEAK_ALLOC.load(Ordering::SeqCst);
    let peak_usage = peak.saturating_sub(pre_current as usize);
    (count, bytes, peak_usage)
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const W: usize = 512;
const H: usize = 512;
const TOTAL_PIXELS: usize = W * H;
const OUTPUT_BYTES: u64 = (TOTAL_PIXELS * 4) as u64;

// ---------------------------------------------------------------------------
// Profile helpers
// ---------------------------------------------------------------------------

/// Create a [`Profile`] with the given encoding and frame length, setting
/// the dimensions to 512×512.
fn make_profile(encoding: Encoding, encoded_len: usize) -> Profile {
    let mut profile = util::make_profile(W as i32, H as i32, encoding);
    profile.frame_byte_length = encoded_len as i32;
    profile
}

/// Profile for CLCL (nibble-chroma, uses `Encoding::Yuv422` +
/// `clcl_chroma` flag).
fn make_profile_clcl(encoded_len: usize) -> Profile {
    let mut p = make_profile(Encoding::Yuv422, encoded_len);
    p.clcl_chroma = true;
    p
}

/// Profile for CL (per-pixel chroma, uses `Encoding::Yuv422` +
/// `cl_chroma` flag).
fn make_profile_cl(encoded_len: usize) -> Profile {
    let mut p = make_profile(Encoding::Yuv422, encoded_len);
    p.cl_chroma = true;
    p
}

// ---------------------------------------------------------------------------
// JPEG fixture generation
// ---------------------------------------------------------------------------

fn make_jpeg_512() -> Vec<u8> {
    let bgra = util::bgra_checkerboard(W, H);
    // Convert BGRA → RGB (JPEG has no alpha).
    let mut rgb = Vec::with_capacity(W * H * 3);
    for px in bgra.chunks(4) {
        rgb.push(px[2]); // R
        rgb.push(px[1]); // G
        rgb.push(px[0]); // B
    }
    let mut jpeg_data = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_data, 75);
    encoder
        .encode(&rgb, W as u32, H as u32, image::ExtendedColorType::Rgb8)
        .expect("JPEG encoding of 512×512 checkerboard should succeed");
    jpeg_data
}

// ---------------------------------------------------------------------------
// Allocation measurement table
// ---------------------------------------------------------------------------

fn run_alloc_benchmarks() {
    let bgra = util::bgra_checkerboard(W, H);

    println!();
    println!("=== Memory Allocation per Decode (512×512) ===");
    println!(
        "{:<28} {:>10} {:>14} {:>14} {:>14}",
        "Format", "Allocs", "Total Bytes", "Peak Heap", "Bytes/px"
    );
    println!("{}", "-".repeat(82));

    // ---- helper ----
    macro_rules! measure_and_print {
        ($name:expr, $encoded:expr, $profile:expr, $decoder:expr) => {{
            let canceled = AtomicBool::new(false);
            // Warmup: run the decoder once to settle the allocator.
            let _ = $decoder(&$encoded, &$profile, &canceled);
            // Measure.
            let (count, bytes, peak) = measure_alloc(|| {
                let _ = $decoder(&$encoded, &$profile, &canceled);
            });
            let bpp = bytes as f64 / TOTAL_PIXELS as f64;
            println!("{:<28} {:>10} {:>14} {:>14} {:>13.2}", $name, count, bytes, peak, bpp);
        }};
    }

    // 1. RGB565
    {
        let encoded = enc::encode_rgb565(&bgra, W as i32, H as i32, false);
        let profile = make_profile(Encoding::Rgb565, encoded.len());
        measure_and_print!("RGB565", encoded, profile, rgb565::decode);
    }

    // 2. RGB555
    {
        let encoded = enc::encode_rgb555(&bgra, W as i32, H as i32, false, false);
        let profile = make_profile(Encoding::Rgb555, encoded.len());
        measure_and_print!("RGB555", encoded, profile, rgb555::decode);
    }

    // 3. Reordered RGB555
    {
        let encoded = enc::encode_reordered_rgb555(&bgra, W as i32, H as i32, true);
        let profile = make_profile(Encoding::ReorderedRgb555, encoded.len());
        measure_and_print!("Reordered RGB555", encoded, profile, reordered_rgb555::decode);
    }

    // 4. UYVY
    {
        let encoded = enc::encode_uyvy(&bgra, W as i32, H as i32);
        let profile = make_profile(Encoding::Yuv422, encoded.len());
        measure_and_print!("UYVY 4:2:2", encoded, profile, uyvy::decode);
    }

    // 5. YCbCr 4:2:0
    {
        let encoded = enc::encode_ycbcr420(&bgra, W as i32, H as i32, false);
        let profile = make_profile(Encoding::Ycbcr420, encoded.len());
        measure_and_print!("YCbCr 4:2:0", encoded, profile, ycbcr420::decode);
    }

    // 6. CLCL (nibble-chroma)
    {
        let encoded = enc::encode_clcl(&bgra, W as i32, H as i32);
        let profile = make_profile_clcl(encoded.len());
        measure_and_print!("CLCL", encoded, profile, clcl::decode);
    }

    // 7. CL (per-pixel chroma)
    {
        let encoded = enc::encode_cl(&bgra, W as i32, H as i32);
        let profile = make_profile_cl(encoded.len());
        measure_and_print!("CL", encoded, profile, cl::decode);
    }

    // 8. JPEG
    {
        let encoded = make_jpeg_512();
        let profile = make_profile(Encoding::Jpeg, encoded.len());
        let canceled = AtomicBool::new(false);
        // Warmup
        let _ = jpeg::decode(&encoded, &profile, &canceled);
        // Measure
        let (count, bytes, peak) = measure_alloc(|| {
            let _ = jpeg::decode(&encoded, &profile, &canceled);
        });
        let bpp = bytes as f64 / TOTAL_PIXELS as f64;
        println!("{:<28} {:>10} {:>14} {:>14} {:>13.2}", "JPEG", count, bytes, peak, bpp);
    }

    // Footer
    println!("{}", "-".repeat(82));
    println!(
        "{:<28} {:>10} {:>14} {:>14} {:>14}",
        "All sizes are 512×512", "", "", "", ""
    );
    println!();
}

// ---------------------------------------------------------------------------
// Divan timing benches
// ---------------------------------------------------------------------------

/// Pre-encoded data + profile + cancelled flag for one format.
struct BenchFixture {
    encoded: Vec<u8>,
    profile: Profile,
    canceled: AtomicBool,
}

macro_rules! bench_decoder {
    ($name:ident, $encode:expr, $profile:expr, $decoder:expr) => {
        #[divan::bench]
        fn $name(bencher: divan::Bencher) {
            let bgra = util::bgra_checkerboard(W, H);
            let encoded = $encode(&bgra, W as i32, H as i32);
            let profile = $profile(encoded.len());
            let _canceled = AtomicBool::new(false);
            bencher
                .counter(BytesCount::new(OUTPUT_BYTES))
                .with_inputs(|| BenchFixture {
                    encoded: encoded.clone(),
                    profile: profile.clone(),
                    canceled: AtomicBool::new(false),
                })
                .bench_values(|fixture| {
                    let _ = divan::black_box($decoder(&fixture.encoded, &fixture.profile, &fixture.canceled));
                });
        }
    };
}

bench_decoder!(
    decode_rgb565,
    |b, w, h| enc::encode_rgb565(b, w, h, false),
    |len| make_profile(Encoding::Rgb565, len),
    rgb565::decode
);
bench_decoder!(
    decode_rgb555,
    |b, w, h| enc::encode_rgb555(b, w, h, false, false),
    |len| make_profile(Encoding::Rgb555, len),
    rgb555::decode
);
bench_decoder!(
    decode_reordered_rgb555,
    |b, w, h| enc::encode_reordered_rgb555(b, w, h, true),
    |len| make_profile(Encoding::ReorderedRgb555, len),
    reordered_rgb555::decode
);
bench_decoder!(
    decode_uyvy,
    enc::encode_uyvy,
    |len| make_profile(Encoding::Yuv422, len),
    uyvy::decode
);
bench_decoder!(
    decode_ycbcr420,
    |b, w, h| enc::encode_ycbcr420(b, w, h, false),
    |len| make_profile(Encoding::Ycbcr420, len),
    ycbcr420::decode
);
bench_decoder!(decode_clcl, enc::encode_clcl, make_profile_clcl, clcl::decode);
bench_decoder!(decode_cl, enc::encode_cl, make_profile_cl, cl::decode);

// JPEG uses a different fixture path (no encoder, synthetic JPEG via `image` crate).
#[divan::bench]
fn decode_jpeg(bencher: divan::Bencher) {
    let encoded = make_jpeg_512();
    let profile = make_profile(Encoding::Jpeg, encoded.len());
    let _canceled = AtomicBool::new(false);
    bencher
        .counter(BytesCount::new(OUTPUT_BYTES))
        .with_inputs(|| BenchFixture {
            encoded: encoded.clone(),
            profile: profile.clone(),
            canceled: AtomicBool::new(false),
        })
        .bench_values(|fixture| {
            let _ = divan::black_box(jpeg::decode(&fixture.encoded, &fixture.profile, &fixture.canceled));
        });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    run_alloc_benchmarks();
    divan::main();
}
