//! SIMD-equivalent-to-scalar regression tests.
//!
//! Kills ~152 MISSED cargo-mutants (98 in uyvy/mod.rs, 34 in yuv/mod.rs,
//! 20 in clcl/mod.rs) by comparing SIMD path output against the scalar
//! ground truth (`yuv_to_bgra` BT.601).  Any `+→-`, `|→^`, `<<→>>`,
//! `*→/` mutation in the SIMD row functions diverges from the expected
//! result.
//!
//! Widths are chosen to exercise SIMD batch-loop boundaries:
//!
//! | Format | Width | Bytes  | Loop path                       |
//! |--------|-------|--------|---------------------------------|
//! | UYVY   | 2     | 4      | remainder only (1 quad)          |
//! | UYVY   | 8     | 16     | 1 main iter, no remainder        |
//! | UYVY   | 10    | 20     | 1 main + 1 remainder             |
//! | UYVY   | 16    | 32     | 2 main iters, no remainder       |
//! | YUV420 | 2     | 6      | 1 macroblock                     |
//! | YUV420 | 8     | 24     | 4 macroblocks                    |
//! | YUV420 | 10    | 30     | 5 macroblocks (odd boundary)     |
//! | CLCL   | 2     | 6      | scalar remainder only             |
//! | CLCL   | 8     | 24     | SSE2 1 batch, no remainder       |
//! | CLCL   | 10    | 30     | SSE2 1 batch + scalar remainder  |
//! | CLCL   | 16    | 48     | SSE2 2 batches, no remainder     |

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::borrow_interior_mutable_const,
    clippy::declare_interior_mutable_const,
    clippy::needless_range_loop
)]

use divan as _;
use image as _;
use proptest as _;
use thiserror as _;
use zune_jpeg as _;

use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::yuv::yuv_to_bgra;
use std::sync::atomic::AtomicBool;

mod util;

const CANCELED: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Ground-truth helpers
// ---------------------------------------------------------------------------

/// Compute expected BGRA for a UYVY quad `[U, Y0, V, Y1]` → 2 pixels.
fn uyvy_quad_ground_truth(quad: &[u8; 4]) -> [u8; 8] {
    let mut out = [0u8; 8];
    let p0 = yuv_to_bgra(quad[1], quad[0], quad[2]);
    let p1 = yuv_to_bgra(quad[3], quad[0], quad[2]);
    out[0..4].copy_from_slice(&p0);
    out[4..8].copy_from_slice(&p1);
    out
}

/// Compute expected BGRA for a YUV420 macroblock `[Y0, Y1, Y2, Y3, Cb, Cr]` → 4 pixels.
fn yuv420_quad_ground_truth(quad: &[u8; 6]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let p0 = yuv_to_bgra(quad[0], quad[4], quad[5]);
    let p1 = yuv_to_bgra(quad[1], quad[4], quad[5]);
    let p2 = yuv_to_bgra(quad[2], quad[4], quad[5]);
    let p3 = yuv_to_bgra(quad[3], quad[4], quad[5]);
    out[0..4].copy_from_slice(&p0);
    out[4..8].copy_from_slice(&p1);
    out[8..12].copy_from_slice(&p2);
    out[12..16].copy_from_slice(&p3);
    out
}

/// Compute expected BGRA for one CLCL pixel given Y, nibble-packed Cb/Cr bytes,
/// and pixel index within the row.
fn clcl_pixel_ground_truth(y: u8, cb_byte: u8, cr_byte: u8, px_idx: usize) -> [u8; 4] {
    let n_cb = if px_idx & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
    let n_cr = if px_idx & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
    yuv_to_bgra(y, n_cb << 4, n_cr << 4)
}

// ---------------------------------------------------------------------------
// Test 1: UYVY row — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_uyvy_row_eq_scalar() {
    // Diverse pixel values to exercise different arithmetic paths.
    let quads: &[[u8; 4]] = &[
        [200, 100, 50, 150],
        [128, 128, 128, 128], // neutral gray
        [0, 0, 0, 0],         // black
        [255, 255, 255, 255], // white (clamp boundary)
        [10, 20, 30, 40],     // low values
        [200, 220, 240, 250], // high values
        [1, 2, 3, 4],         // near-zero chroma
        [64, 128, 192, 64],   // mixed
    ];

    for w in [2, 4, 8, 10, 14, 16] {
        // Build UYVY source: w pixels = w/2 quads = w/2 * 4 bytes
        let n_quads = w / 2;
        let mut src = Vec::with_capacity(n_quads * 4);
        for i in 0..n_quads {
            src.extend_from_slice(&quads[i % quads.len()]);
        }

        let mut simd_dst = vec![0u8; w * 4];
        ithmb_core::simd::uyvy_row_to_bgra(&src, &mut simd_dst).unwrap();

        // Compute expected from scalar ground truth
        let mut expected = vec![0u8; w * 4];
        for i in 0..n_quads {
            let gt = uyvy_quad_ground_truth(&src[i * 4..(i + 1) * 4].try_into().unwrap());
            expected[i * 8..(i + 1) * 8].copy_from_slice(&gt);
        }

        assert_eq!(simd_dst, expected, "UYVY row mismatch at w={w}");
    }
}

// ---------------------------------------------------------------------------
// Test 2: UYVY double-quad — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_uyvy_double_quad_eq_scalar() {
    let quads: [[u8; 4]; 8] = [
        [200, 100, 50, 150],
        [128, 128, 128, 128],
        [0, 0, 0, 0],
        [255, 255, 255, 255],
        [10, 20, 30, 40],
        [200, 220, 240, 250],
        [1, 2, 3, 4],
        [64, 128, 192, 64],
    ];

    for n in [1, 2, 3, 4] {
        let mut src = Vec::with_capacity(n * 8);
        for i in 0..n {
            src.extend_from_slice(&quads[i * 2]);
            src.extend_from_slice(&quads[i * 2 + 1]);
        }

        let simd_out = ithmb_core::simd::uyvy_double_quad_to_bgra(src[..8].try_into().unwrap());

        // Compute expected from the actual 8 input bytes (2 quads)
        let q0: [u8; 4] = src[0..4].try_into().unwrap();
        let q1: [u8; 4] = src[4..8].try_into().unwrap();
        let mut expected = [0u8; 16];
        expected[0..8].copy_from_slice(&uyvy_quad_ground_truth(&q0));
        expected[8..16].copy_from_slice(&uyvy_quad_ground_truth(&q1));

        assert_eq!(simd_out, expected, "UYVY double-quad mismatch at pair {n}");
    }
}

// ---------------------------------------------------------------------------
// Test 3: YUV420 row pair — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_yuv420_row_pair_eq_scalar() {
    // Row pair: y_row has 2w bytes (2 rows), cb_row has w/2 bytes, cr_row has w/2 bytes.
    // Output dst has 2 * w * 4 bytes (2 rows of BGRA).

    for w in [2, 4, 6, 8, 10, 14, 16] {
        let cb_w = w / 2;
        // Y values: row0 = sequential luma, row1 = reversed
        let mut y_row = vec![0u8; 2 * w];
        for i in 0..w {
            y_row[i] = (i as u8).wrapping_mul(17).wrapping_add(10);
            y_row[w + i] = (i as u8).wrapping_mul(13).wrapping_add(50);
        }
        // Chroma: diverse Cb/Cr values
        let cb_row: Vec<u8> = (0..cb_w).map(|i| (i as u8).wrapping_mul(30).wrapping_add(80)).collect();
        let cr_row: Vec<u8> = (0..cb_w)
            .map(|i| (i as u8).wrapping_mul(25).wrapping_add(100))
            .collect();

        let mut simd_dst = vec![0u8; 2 * w * 4];
        ithmb_core::simd::yuv420_row_pair_to_bgra(&y_row, &cb_row, &cr_row, &mut simd_dst, w, cb_w);

        // Compute expected from scalar ground truth
        let mut expected = vec![0u8; 2 * w * 4];
        for cx in 0..cb_w {
            let quad = [
                y_row[cx * 2],
                y_row[cx * 2 + 1],
                y_row[w + cx * 2],
                y_row[w + cx * 2 + 1],
                cb_row[cx],
                cr_row[cx],
            ];
            let out = yuv420_quad_ground_truth(&quad);
            let off = cx * 8;
            expected[off..off + 4].copy_from_slice(&out[0..4]);
            expected[off + 4..off + 8].copy_from_slice(&out[4..8]);
            let off2 = off + w * 4;
            expected[off2..off2 + 4].copy_from_slice(&out[8..12]);
            expected[off2 + 4..off2 + 8].copy_from_slice(&out[12..16]);
        }

        assert_eq!(simd_dst, expected, "YUV420 row-pair mismatch at w={w}");
    }
}

// ---------------------------------------------------------------------------
// Test 4: CLCL full decode — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_clcl_decode_eq_scalar() {
    // CLCL layout: Y(w*h) + Cb_nibble(w*h/2) + Cr_nibble(w*h/2)
    // We decode via clcl::decode() which calls clcl_row_to_bgra per row.
    for w in [2usize, 4, 6, 8, 10, 14, 16] {
        let h: usize = 2; // 2 rows to exercise row-loop
        let n = w * h;
        let chroma_len = n.div_ceil(2);

        let mut y_plane = vec![0u8; n];
        let mut cb_packed = vec![0u8; chroma_len];
        let mut cr_packed = vec![0u8; chroma_len];

        // Fill with diverse values
        for i in 0..n {
            y_plane[i] = (i as u8).wrapping_mul(7).wrapping_add(30);
        }
        for i in 0..chroma_len {
            // Even pixel nibble in low, odd pixel nibble in high
            let cb_lo = ((i * 3) as u8) & 0x0F;
            let cb_hi = ((i * 5 + 7) as u8) & 0x0F;
            cb_packed[i] = (cb_hi << 4) | cb_lo;
            let cr_lo = ((i * 2 + 1) as u8) & 0x0F;
            let cr_hi = ((i * 4 + 3) as u8) & 0x0F;
            cr_packed[i] = (cr_hi << 4) | cr_lo;
        }

        // Assemble the full CLCL planar buffer
        let mut src = Vec::with_capacity(n + 2 * chroma_len);
        src.extend_from_slice(&y_plane);
        src.extend_from_slice(&cb_packed);
        src.extend_from_slice(&cr_packed);

        let profile = Profile {
            prefix: 0,
            width: w as i32,
            height: h as i32,
            encoding: Encoding::Yuv422,
            frame_byte_length: src.len() as i32,
            clcl_chroma: true,
            ..Default::default()
        };

        let decoded = ithmb_core::clcl::decode(&src, &profile, &CANCELED).unwrap();
        assert_eq!(decoded.width, w as u32);
        assert_eq!(decoded.height, h as u32);

        // Compute expected from scalar ground truth
        let mut expected = vec![0u8; n * 4];
        for row in 0..h {
            for col in 0..w {
                let px = row * w + col;
                let gt = clcl_pixel_ground_truth(y_plane[px], cb_packed[px / 2], cr_packed[px / 2], col);
                let off = px * 4;
                expected[off..off + 4].copy_from_slice(&gt);
            }
        }

        assert_eq!(decoded.data, expected, "CLCL decode mismatch at w={w}, h={h}");
    }
}

// ---------------------------------------------------------------------------
// Test 5: UYVY full decode through format decoder — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_uyvy_full_decode_eq_scalar() {
    // Tests the complete path: uyvy::decode → uyvy_row_to_bgra at each row
    for w in [2, 8, 10, 16, 17] {
        let h: usize = 4;
        let quads: [[u8; 4]; 4] = [
            [200, 100, 50, 150],
            [0, 0, 0, 0],
            [255, 255, 255, 255],
            [64, 128, 192, 64],
        ];

        // UYVY source: w * 2 bytes per row (= row_stride)
        // For odd w: n_quads * 4 complete bytes + 2 trailing bytes [U, Y]
        let row_stride = w * 2;
        let n_quads = w / 2;
        let mut src = vec![0u8; row_stride * h];
        for row in 0..h {
            let row_start = row * row_stride;
            // Fill complete quads
            for q in 0..n_quads {
                let qdata = quads[(row * n_quads + q) % quads.len()];
                let off = row_start + q * 4;
                src[off..off + 4].copy_from_slice(&qdata);
            }
            // Fill trailing [U, Y] pair for odd width
            if w % 2 != 0 {
                let trail_off = row_start + n_quads * 4;
                src[trail_off] = 0xAB; // U for trailing pixel
                src[trail_off + 1] = 0xCD; // Y for trailing pixel
            }
        }

        let profile = Profile {
            prefix: 0,
            width: w as i32,
            height: h as i32,
            encoding: Encoding::Yuv422,
            frame_byte_length: src.len() as i32,
            ..Default::default()
        };

        let decoded = ithmb_core::uyvy::decode(&src, &profile, &CANCELED).unwrap();

        // Compute expected per-pixel from ground truth
        let mut expected = vec![0u8; w * h * 4];
        for row in 0..h {
            let row_start = row * row_stride;
            for q in 0..n_quads {
                let quad: [u8; 4] = src[row_start + q * 4..row_start + q * 4 + 4].try_into().unwrap();
                let gt = uyvy_quad_ground_truth(&quad);
                let px_off = (row * w + q * 2) * 4;
                expected[px_off..px_off + 8].copy_from_slice(&gt);
            }
            // Odd width: decode_row reads [U, Y] from trailing bytes,
            // V from last complete quad's V byte (offset groups*4-2 in row_src)
            if w % 2 != 0 {
                let last_px = row * w + w - 1;
                let trail_off = row_start + n_quads * 4;
                let u = src[trail_off];
                let y = src[trail_off + 1];
                let v = if n_quads > 0 {
                    src[row_start + n_quads * 4 - 2] // V from last complete quad
                } else {
                    128
                };
                let gt = yuv_to_bgra(y, u, v);
                expected[last_px * 4..last_px * 4 + 4].copy_from_slice(&gt);
            }
        }

        assert_eq!(decoded.data, expected, "UYVY full decode mismatch at w={w}");
    }
}

// ---------------------------------------------------------------------------
// Test 6: YUV420 full decode through format decoder — SIMD eq scalar
// ---------------------------------------------------------------------------

#[test]
fn simd_ycbcr420_full_decode_eq_scalar() {
    // Tests the complete path: ycbcr420::decode → yuv420_row_pair_to_bgra
    for w in [2usize, 4, 8, 10, 14, 16] {
        let h: usize = 4; // must be even for YCbCr420
        let cb_w = w.div_ceil(2);
        let uv_h = h.div_ceil(2);
        let frame_len = w * h + cb_w * uv_h * 2;

        // Build frame data: Y plane + Cb plane + Cr plane
        let mut frame = vec![0u8; frame_len];
        // Y values: diverse per-row
        for row in 0..h {
            for col in 0..w {
                frame[row * w + col] = (row as u8)
                    .wrapping_mul(17)
                    .wrapping_add((col as u8).wrapping_mul(13).wrapping_add(10));
            }
        }
        // Cb plane: after Y plane
        let cb_off = w * h;
        for row in 0..uv_h {
            for col in 0..cb_w {
                frame[cb_off + row * cb_w + col] = (row as u8)
                    .wrapping_mul(30)
                    .wrapping_add((col as u8).wrapping_mul(11).wrapping_add(80));
            }
        }
        // Cr plane: after Cb plane
        let cr_off = cb_off + cb_w * uv_h;
        for row in 0..uv_h {
            for col in 0..cb_w {
                frame[cr_off + row * cb_w + col] = (row as u8)
                    .wrapping_mul(25)
                    .wrapping_add((col as u8).wrapping_mul(19).wrapping_add(100));
            }
        }

        let profile = Profile {
            prefix: 0,
            width: w as i32,
            height: h as i32,
            encoding: Encoding::Ycbcr420,
            frame_byte_length: frame_len as i32,
            ..Default::default()
        };

        let decoded = ithmb_core::ycbcr420::decode(&frame, &profile, &CANCELED).unwrap();

        // Compute expected per-pixel from ground truth
        let mut expected = vec![0u8; w * h * 4];
        for mb_row in 0..uv_h {
            for mb_col in 0..cb_w {
                let cb = frame[cb_off + mb_row * cb_w + mb_col];
                let cr = frame[cr_off + mb_row * cb_w + mb_col];
                for dy in 0..2 {
                    let py = mb_row * 2 + dy;
                    if py >= h {
                        continue;
                    }
                    for dx in 0..2 {
                        let px = mb_col * 2 + dx;
                        if px >= w {
                            continue;
                        }
                        let y = frame[py * w + px];
                        let gt = yuv_to_bgra(y, cb, cr);
                        let off = (py * w + px) * 4;
                        expected[off..off + 4].copy_from_slice(&gt);
                    }
                }
            }
        }

        assert_eq!(decoded.data, expected, "YCbCr420 full decode mismatch at w={w}");
    }
}
