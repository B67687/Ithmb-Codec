// SPDX-License-Identifier: MIT
//! Direct comparison of scalar vs SIMD decode throughput for each pixel format.
//!
//! Each format has two benchmarks:
//! - `decode_{format}_scalar` — portable scalar fallback (inlined)
//! - `decode_{format}_simd` — SIMD dispatch (SSE2/AVX2/NEON when available)
//!
//! A third shot at the speedup ratios in the README at 256×256, both paths
//! processing *identical* checkerboard input in the *same binary*.  Requires
//! `--features simd` to see SIMD numbers; the scalar variants are always
//! available.
//!
//! Run:  cargo bench --features simd --bench simd_compare
//!
//! The speedup table in the README was produced by comparing SIMD runs against
//! scalar runs of the benches in `decoders.rs`.  This file adds both paths to
//! a single binary so the comparison is unambiguous.

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::similar_names,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::profile::Encoding;
use ithmb_core::{enc, simd, yuv};
use util::{bgra_checkerboard, make_profile, never_canceled};

// All benches use a single size (256×256) matching the README speedup table.
const W: usize = 256;
const H: usize = 256;
/// Output bytes for one 256×256 BGRA frame.
const OUTPUT_BYTES: u64 = (W * H * 4) as u64;

// ---------------------------------------------------------------------------
// Scalar fallback implementations (inlined — no production code changes)
// ---------------------------------------------------------------------------

fn msb5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}
fn msb6(v: u32) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}

fn unpack_rgb565(pixel: u16) -> [u8; 4] {
    let r5 = u32::from((pixel >> 11) & 0x1F);
    let g6 = u32::from((pixel >> 5) & 0x3F);
    let b5 = u32::from(pixel & 0x1F);
    [msb5(b5), msb6(g6), msb5(r5), 255]
}

fn unpack_rgb555(pixel: u16) -> [u8; 4] {
    let r5 = u32::from((pixel >> 10) & 0x1F);
    let g5 = u32::from((pixel >> 5) & 0x1F);
    let b5 = u32::from(pixel & 0x1F);
    [msb5(b5), msb5(g5), msb5(r5), 255]
}

fn rgb565_row_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    let n = src.len() / 2;
    debug_assert_eq!(dst.len(), n * 4);
    for i in 0..n {
        let px = unpack_rgb565(u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]));
        let o = i * 4;
        dst[o] = px[0];
        dst[o + 1] = px[1];
        dst[o + 2] = px[2];
        dst[o + 3] = px[3];
    }
}

fn rgb555_row_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    let n = src.len() / 2;
    debug_assert_eq!(dst.len(), n * 4);
    for i in 0..n {
        let px = unpack_rgb555(u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]));
        let o = i * 4;
        dst[o] = px[0];
        dst[o + 1] = px[1];
        dst[o + 2] = px[2];
        dst[o + 3] = px[3];
    }
}

fn uyvy_quad_to_bgra_scalar(quad: &[u8; 4]) -> [u8; 8] {
    let [u, y0, v, y1] = *quad;
    let p0 = yuv::yuv_to_bgra(y0, u, v);
    let p1 = yuv::yuv_to_bgra(y1, u, v);
    [p0[0], p0[1], p0[2], p0[3], p1[0], p1[1], p1[2], p1[3]]
}

fn yuv420_quad_to_bgra_scalar(quad: &[u8; 6]) -> [u8; 16] {
    let [y0, y1, y2, y3, cb, cr] = *quad;
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&yuv::yuv_to_bgra(y0, cb, cr));
    out[4..8].copy_from_slice(&yuv::yuv_to_bgra(y1, cb, cr));
    out[8..12].copy_from_slice(&yuv::yuv_to_bgra(y2, cb, cr));
    out[12..].copy_from_slice(&yuv::yuv_to_bgra(y3, cb, cr));
    out
}

fn fill_gray_row_scalar(gray: &[u8]) -> Vec<u8> {
    gray.iter().flat_map(|&g| [g, g, g, 255]).collect()
}

/// Decode one row of CLCL data (scalar).
///
/// Input layout (src has `w + ceil(w/2) + ceil(w/2)` bytes per row):
///   Y  (w bytes) + Cb packed nibbles (ceil(w/2) bytes) + Cr packed nibbles.
fn clcl_decode_row_scalar(y_row: &[u8], cb_row: &[u8], cr_row: &[u8], dst: &mut [u8]) {
    let w = y_row.len();
    for i in 0..w {
        let y = y_row[i];
        let cb_nib = if i & 1 == 0 {
            cb_row[i / 2] & 0x0F
        } else {
            cb_row[i / 2] >> 4
        };
        let cr_nib = if i & 1 == 0 {
            cr_row[i / 2] & 0x0F
        } else {
            cr_row[i / 2] >> 4
        };
        let px = yuv::yuv_to_bgra(y, cb_nib << 4, cr_nib << 4);
        let o = i * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn main() {
    // ── RGB565 ──────────────────────────────────────────────────────────
    #[divan::bench]
    fn decode_rgb565_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_rgb565(&bgra, W as i32, H as i32, false);
        let n_pix = enc.len() / 2;
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| {
                let dst = vec![0u8; n_pix * 4];
                (enc.clone(), dst)
            })
            .bench_refs(|(src, dst)| rgb565_row_to_bgra_scalar(src, dst));
    }

    #[divan::bench]
    fn decode_rgb565_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_rgb565(&bgra, W as i32, H as i32, false);
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| enc.clone())
            .bench_refs(|src| {
                let _ = divan::black_box(simd::rgb565_row_to_bgra(src));
            });
    }

    // ── RGB555 ──────────────────────────────────────────────────────────
    #[divan::bench]
    fn decode_rgb555_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_rgb555(&bgra, W as i32, H as i32, false, false);
        let n_pix = enc.len() / 2;
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| {
                let dst = vec![0u8; n_pix * 4];
                (enc.clone(), dst)
            })
            .bench_refs(|(src, dst)| rgb555_row_to_bgra_scalar(src, dst));
    }

    #[divan::bench]
    fn decode_rgb555_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_rgb555(&bgra, W as i32, H as i32, false, false);
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| enc.clone())
            .bench_refs(|src| {
                let _ = divan::black_box(simd::rgb555_row_to_bgra(src));
            });
    }

    // ── UYVY ────────────────────────────────────────────────────────────
    #[divan::bench]
    fn decode_uyvy_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_uyvy(&bgra, W as i32, H as i32);
        let quads = enc.len() / 4;
        let dst = vec![0u8; quads * 8];
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| (enc.clone(), dst.clone()))
            .bench_refs(|(src, d)| {
                for g in 0..quads {
                    let si = g * 4;
                    let q: &[u8; 4] = src[si..si + 4].try_into().unwrap();
                    let px = uyvy_quad_to_bgra_scalar(q);
                    let di = g * 8;
                    d[di..di + 8].copy_from_slice(&px);
                }
            });
    }

    #[divan::bench]
    fn decode_uyvy_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_uyvy(&bgra, W as i32, H as i32);
        let quads = enc.len() / 4;
        let dst = vec![0u8; quads * 8];
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| (enc.clone(), dst.clone()))
            .bench_refs(|(src, d)| {
                for g in 0..quads {
                    let si = g * 4;
                    let q: &[u8; 4] = src[si..si + 4].try_into().unwrap();
                    let px = simd::uyvy_quad_to_bgra(q);
                    let di = g * 8;
                    d[di..di + 8].copy_from_slice(&px);
                }
            });
    }

    // ── YCbCr 4:2:0 ─────────────────────────────────────────────────────
    #[divan::bench]
    fn decode_ycbcr420_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_ycbcr420(&bgra, W as i32, H as i32, false);
        let y_size = W * H;
        let uv_w = W.div_ceil(2);
        let uv_h = H.div_ceil(2);
        let dst = vec![0u8; W * H * 4];
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| (enc.clone(), dst.clone()))
            .bench_refs(|(src, d)| {
                for cy in 0..uv_h {
                    for cx in 0..uv_w {
                        let y00 = src[cy * 2 * W + cx * 2];
                        let y01 = src[cy * 2 * W + cx * 2 + 1];
                        let y10 = src[(cy * 2 + 1) * W + cx * 2];
                        let y11 = src[(cy * 2 + 1) * W + cx * 2 + 1];
                        let cb = src[y_size + cy * uv_w + cx];
                        let cr = src[y_size + uv_w * uv_h + cy * uv_w + cx];
                        let px = yuv420_quad_to_bgra_scalar(&[y00, y01, y10, y11, cb, cr]);
                        let di = (cy * 2 * W + cx * 2) * 4;
                        let di2 = ((cy * 2 + 1) * W + cx * 2) * 4;
                        d[di..di + 4].copy_from_slice(&px[..4]);
                        d[di + 4..di + 8].copy_from_slice(&px[4..8]);
                        d[di2..di2 + 4].copy_from_slice(&px[8..12]);
                        d[di2 + 4..di2 + 8].copy_from_slice(&px[12..]);
                    }
                }
            });
    }

    #[divan::bench]
    fn decode_ycbcr420_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_ycbcr420(&bgra, W as i32, H as i32, false);
        let uv_w = W.div_ceil(2);
        let uv_h = H.div_ceil(2);
        let uv_size = uv_w * uv_h;
        let y_size = W * H;
        let dst = vec![0u8; W * H * 4];
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| (enc.clone(), dst.clone()))
            .bench_refs(|(src, d)| {
                for cy in 0..uv_h {
                    for cx in 0..uv_w {
                        let quad = [
                            src[cy * 2 * W + cx * 2],
                            src[cy * 2 * W + cx * 2 + 1],
                            src[(cy * 2 + 1) * W + cx * 2],
                            src[(cy * 2 + 1) * W + cx * 2 + 1],
                            src[y_size + cy * uv_w + cx],
                            src[y_size + uv_size + cy * uv_w + cx],
                        ];
                        let px = simd::yuv420_quad_to_bgra(&quad);
                        let di = (cy * 2 * W + cx * 2) * 4;
                        let di2 = ((cy * 2 + 1) * W + cx * 2) * 4;
                        d[di..di + 4].copy_from_slice(&px[..4]);
                        d[di + 4..di + 8].copy_from_slice(&px[4..8]);
                        d[di2..di2 + 4].copy_from_slice(&px[8..12]);
                        d[di2 + 4..di2 + 8].copy_from_slice(&px[12..]);
                    }
                }
            });
    }

    // ── CL (per-pixel nibble chroma) ────────────────────────────────────
    #[divan::bench]
    fn decode_cl_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_cl(&bgra, W as i32, H as i32);
        let n_pix = enc.len() / 2;
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| {
                let dst = vec![0u8; n_pix * 4];
                (enc.clone(), dst)
            })
            .bench_refs(|(src, d)| {
                let (y_plane, chroma) = src.split_at(n_pix);
                for i in 0..n_pix {
                    let cr = chroma[i] & 0xF0;
                    let cb = (chroma[i] & 0x0F) << 4;
                    let px = yuv::yuv_to_bgra(y_plane[i], cb, cr);
                    let o = i * 4;
                    d[o..o + 4].copy_from_slice(&px);
                }
            });
    }

    #[divan::bench]
    fn decode_cl_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_cl(&bgra, W as i32, H as i32);
        let n_pix = enc.len() / 2;
        let quads = n_pix / 4;
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| {
                let dst = vec![0u8; n_pix * 4];
                (enc.clone(), dst)
            })
            .bench_refs(|(src, d)| {
                let (y_plane, chroma) = src.split_at(n_pix);
                for q in 0..quads {
                    let yi = q * 4;
                    let quad = [
                        y_plane[yi],
                        y_plane[yi + 1],
                        y_plane[yi + 2],
                        y_plane[yi + 3],
                        chroma[yi],
                        chroma[yi + 1],
                        chroma[yi + 2],
                        chroma[yi + 3],
                    ];
                    let px = simd::cl_quad_to_bgra(&quad);
                    let di = yi * 4;
                    d[di..di + 16].copy_from_slice(&px);
                }
                // Remainder pixels (if n_pix % 4 != 0)
                for i in (quads * 4)..n_pix {
                    let cr = chroma[i] & 0xF0;
                    let cb = (chroma[i] & 0x0F) << 4;
                    let px = yuv::yuv_to_bgra(y_plane[i], cb, cr);
                    let o = i * 4;
                    d[o..o + 4].copy_from_slice(&px);
                }
            });
    }

    // ── CLCL (separate Cb/Cr nibble planes) ─────────────────────────────
    // Speedup (71×) comes from row-level SIMD over plane-separated layout.
    // Here we compare the full decoder path (SIMD via `clcl::decode`) against
    // an inline scalar implementation.
    #[divan::bench]
    fn decode_clcl_scalar(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_clcl(&bgra, W as i32, H as i32);
        let cb_off = W * H;
        let chroma_len = (W * H).div_ceil(2);
        let cr_off = cb_off + chroma_len;
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| {
                let dst = vec![0u8; W * H * 4];
                (enc.clone(), dst)
            })
            .bench_refs(|(src, d)| {
                for row in 0..H {
                    let idx = row * W;
                    let y_row = &src[idx..idx + W];
                    let cb_row = &src[cb_off + idx / 2..cb_off + idx / 2 + W.div_ceil(2)];
                    let cr_row = &src[cr_off + idx / 2..cr_off + idx / 2 + W.div_ceil(2)];
                    let dst_row = &mut d[idx * 4..(idx + W) * 4];
                    clcl_decode_row_scalar(y_row, cb_row, cr_row, dst_row);
                }
            });
    }

    #[divan::bench]
    fn decode_clcl_simd(bencher: divan::Bencher) {
        let bgra = bgra_checkerboard(W, H);
        let enc = enc::encode_clcl(&bgra, W as i32, H as i32);
        let mut profile = make_profile(W as i32, H as i32, Encoding::Yuv422);
        profile.frame_byte_length = enc.len() as i32;
        profile.clcl_chroma = true;
        let canceled = never_canceled();
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| (enc.clone(), &profile, &canceled))
            .bench_refs(|(src, profile, canceled)| {
                let _ = divan::black_box(ithmb_core::clcl::decode(src, profile, canceled));
            });
    }

    // ── Gray / monochrome ───────────────────────────────────────────────
    #[divan::bench]
    fn decode_gray_scalar(bencher: divan::Bencher) {
        let gray: Vec<u8> = (0..(W * H) as u8).collect();
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| gray.clone())
            .bench_refs(|g| {
                let _ = divan::black_box(fill_gray_row_scalar(g));
            });
    }

    #[divan::bench]
    fn decode_gray_simd(bencher: divan::Bencher) {
        let gray: Vec<u8> = (0..(W * H) as u8).collect();
        bencher
            .counter(BytesCount::new(OUTPUT_BYTES))
            .with_inputs(|| gray.clone())
            .bench_refs(|g| {
                let _ = divan::black_box(simd::fill_gray_row(g));
            });
    }

    divan::main();
}
