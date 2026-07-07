// SPDX-License-Identifier: MIT
// Encoder helper utilities: interlaced field encoding and BT.601 color conversion tables
//
// Ported from C# `IthmbCodecPlugin.EncoderHelpers.cs`.
//! Shared encoder utilities: BT.601 forward transform, field interlace, clamp.

// Forward-export functions intentionally dead until T2+ encoder modules land.
#![allow(clippy::similar_names)]

use crate::profile::Encoding;

// ---- BT.601 forward transform helpers (fixed-point) ----

/// Compute BT.601 luma (Y) from R/G/B using fixed-point coefficients.
///
/// Y = 0.299R + 0.587G + 0.114B  (fixed-point: 77/256, 150/256, 29/256).
#[inline]
#[must_use]
pub(crate) fn bt601_y(r: i32, g: i32, b: i32) -> i32 {
    (77 * r + 150 * g + 29 * b) >> 8
}

/// Compute BT.601 blue-difference chroma (Cb) from R/G/B.
///
/// Cb = -0.169R - 0.331G + 0.500B + 128
///      (fixed-point: -43/256, -85/256, 128/256).
#[inline]
#[must_use]
pub(crate) fn bt601_cb(r: i32, g: i32, b: i32) -> i32 {
    ((-43 * r - 85 * g + 128 * b) >> 8) + 128
}

/// Compute BT.601 red-difference chroma (Cr) from R/G/B.
///
/// Cr = 0.500R - 0.419G - 0.081B + 128
///      (fixed-point: 128/256, -107/256, -21/256).
#[inline]
#[must_use]
pub(crate) fn bt601_cr(r: i32, g: i32, b: i32) -> i32 {
    ((128 * r - 107 * g - 21 * b) >> 8) + 128
}

/// Interlace fields for interlaced YCbCr 4:2:0 and 2 Bpp formats.
///
/// For non-YCbCr420 formats (2 Bpp), each row is `w × 2` bytes; rows are
/// reordered so that even rows (field 0) come first, then odd rows (field 1).
///
/// For YCbCr 4:2:0 planar, each of the three planes (Y, Cb, Cr) is interlaced
/// separately using its own row stride.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub(crate) fn interlace_fields(planar: &[u8], w: i32, h: i32, enc: Encoding) -> Vec<u8> {
    let w = w as usize;
    let h = h as usize;

    if enc != Encoding::Ycbcr420 {
        // 2 Bpp interlace: each row is w * 2 bytes, just reorder rows
        let row_stride = w * 2;
        let half_rows = h.div_ceil(2);
        let mut result = vec![0u8; planar.len()];
        for y in 0..h {
            let src_off = y * row_stride;
            let dst_off = if y % 2 == 0 {
                (y / 2) * row_stride
            } else {
                (half_rows + y / 2) * row_stride
            };
            result[dst_off..dst_off + row_stride].copy_from_slice(&planar[src_off..src_off + row_stride]);
        }
        return result;
    }

    // YCbCr 4:2:0 planar — 3 planes: Y (w×h), Cb (w/2×h/2), Cr (w/2×h/2)
    let y_size = w * h;
    let uv_w = w.div_ceil(2);
    let uv_h = h.div_ceil(2);
    let c_size = uv_w * uv_h;
    let y_row = w;
    let c_row = uv_w;
    let half_h = h.div_ceil(2);
    let half_h_uv = uv_h.div_ceil(2);

    let mut result = vec![0u8; planar.len()];

    // --- Interlace Y plane (full resolution) ---
    for y in 0..h {
        let src_off = y * y_row;
        let dst_off = if y % 2 == 0 {
            (y / 2) * y_row
        } else {
            (half_h + y / 2) * y_row
        };
        result[dst_off..dst_off + y_row].copy_from_slice(&planar[src_off..src_off + y_row]);
    }

    // --- Interlace Cb plane (half resolution) ---
    let cb_off = y_size;
    for y in 0..h / 2 {
        let src_off = cb_off + y * c_row;
        let dst_off = cb_off
            + if y % 2 == 0 {
                (y / 2) * c_row
            } else {
                (half_h_uv + y / 2) * c_row
            };
        result[dst_off..dst_off + c_row].copy_from_slice(&planar[src_off..src_off + c_row]);
    }

    // --- Interlace Cr plane (half resolution) ---
    let cr_off = y_size + c_size;
    for y in 0..h / 2 {
        let src_off = cr_off + y * c_row;
        let dst_off = cr_off
            + if y % 2 == 0 {
                (y / 2) * c_row
            } else {
                (half_h_uv + y / 2) * c_row
            };
        result[dst_off..dst_off + c_row].copy_from_slice(&planar[src_off..src_off + c_row]);
    }

    result
}
// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- BT.601 Y ----

    #[test]
    fn bt601_y_black() {
        // Y(0, 0, 0) = 0
        assert_eq!(bt601_y(0, 0, 0), 0);
    }

    #[test]
    fn bt601_y_white() {
        // Y(255, 255, 255) = (77+150+29)*255 / 256 = 65280/256 = 255
        assert_eq!(bt601_y(255, 255, 255), 255);
    }

    #[test]
    fn bt601_y_mid_gray() {
        // Y(128, 128, 128) = (77+150+29)*128 / 256 = 32768/256 = 128
        assert_eq!(bt601_y(128, 128, 128), 128);
    }

    #[test]
    fn bt601_y_red() {
        // Y(255, 0, 0) = 77*255 / 256 = 19635/256 = 76
        assert_eq!(bt601_y(255, 0, 0), 76);
    }

    #[test]
    fn bt601_y_green() {
        // Y(0, 255, 0) = 150*255 / 256 = 38250/256 = 149
        assert_eq!(bt601_y(0, 255, 0), 149);
    }

    #[test]
    fn bt601_y_blue() {
        // Y(0, 0, 255) = 29*255 / 256 = 7395/256 = 28
        assert_eq!(bt601_y(0, 0, 255), 28);
    }

    // ---- BT.601 Cb ----

    #[test]
    fn bt601_cb_black() {
        // Cb(0, 0, 0) = 0 + 128 = 128
        assert_eq!(bt601_cb(0, 0, 0), 128);
    }

    #[test]
    fn bt601_cb_white() {
        // Cb(255, 255, 255) = (-43-85+128)*255 / 256 + 128 = 0 + 128 = 128
        assert_eq!(bt601_cb(255, 255, 255), 128);
    }

    #[test]
    fn bt601_cb_mid_gray() {
        // Cb(128, 128, 128) = (-43-85+128)*128 / 256 + 128 = 0 + 128 = 128
        assert_eq!(bt601_cb(128, 128, 128), 128);
    }

    #[test]
    fn bt601_cb_red() {
        // Cb(255, 0, 0) = -43*255 / 256 + 128 = -43 + 128 = 85
        assert_eq!(bt601_cb(255, 0, 0), 85);
    }

    #[test]
    fn bt601_cb_blue() {
        // Cb(0, 0, 255) = 128*255 / 256 + 128 = 127 + 128 = 255
        assert_eq!(bt601_cb(0, 0, 255), 255);
    }

    // ---- BT.601 Cr ----

    #[test]
    fn bt601_cr_black() {
        // Cr(0, 0, 0) = 0 + 128 = 128
        assert_eq!(bt601_cr(0, 0, 0), 128);
    }

    #[test]
    fn bt601_cr_white() {
        // Cr(255, 255, 255) = (128-107-21)*255 / 256 + 128 = 0 + 128 = 128
        assert_eq!(bt601_cr(255, 255, 255), 128);
    }

    #[test]
    fn bt601_cr_mid_gray() {
        // Cr(128, 128, 128) = (128-107-21)*128 / 256 + 128 = 0 + 128 = 128
        assert_eq!(bt601_cr(128, 128, 128), 128);
    }

    #[test]
    fn bt601_cr_red() {
        // Cr(255, 0, 0) = 128*255 / 256 + 128 = 127 + 128 = 255
        assert_eq!(bt601_cr(255, 0, 0), 255);
    }

    #[test]
    fn bt601_cr_blue() {
        // Cr(0, 0, 255) = -21*255 / 256 + 128 = -21 + 128 = 107
        assert_eq!(bt601_cr(0, 0, 255), 107);
    }

    // ---- InterlaceFields — 2Bpp path (non-Ycbcr420) ----

    #[test]
    fn interlace_2bpp_4x4() {
        // 4 rows × 2 Bpp = w*2 = 4 bytes/row.
        // Assign each row a distinct byte pattern so reordering is visible.
        let w = 2;
        let h = 4;
        let row_stride = w * 2; // 4
        let mut planar = Vec::with_capacity(row_stride * h);
        for row in 0..h {
            for col in 0..row_stride {
                planar.push(u8::try_from(row * 16 + col).unwrap());
            }
        }
        // Expected order: even rows first (0, 2), then odd rows (1, 3).
        //   row 0 → offset  0
        //   row 2 → offset  4
        //   row 1 → offset  8  (half_rows=2 → 2*4=8)
        //   row 3 → offset 12  (half_rows=2 → (2+1)*4=12)
        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Rgb565,
        );
        assert_eq!(result.len(), planar.len());

        let half = h.div_ceil(2); // 2
        for y in 0..h {
            let src_off = y * row_stride;
            let expected_dst = if y % 2 == 0 {
                (y / 2) * row_stride
            } else {
                (half + y / 2) * row_stride
            };
            assert_eq!(
                &result[expected_dst..expected_dst + row_stride],
                &planar[src_off..src_off + row_stride],
                "row {y} mapped to wrong position"
            );
        }
    }

    #[test]
    fn interlace_2bpp_single_row() {
        // h=1: single even row, should map to position 0.
        let w = 4;
        let h = 1;
        let planar = vec![0xAAu8; w * 2 * h];
        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Rgb565,
        );
        assert_eq!(result, planar);
    }

    #[test]
    fn interlace_2bpp_single_column() {
        // w=1, h=4: narrow interlace.
        let w = 1;
        let h = 4;
        let row_stride = w * 2;
        let mut planar = Vec::with_capacity(row_stride * h);
        for row in 0..h {
            planar.push(u8::try_from(row * 10).unwrap());
            planar.push(u8::try_from(row * 10 + 1).unwrap());
        }
        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Rgb565,
        );
        assert_eq!(result.len(), planar.len());

        let half = h.div_ceil(2);
        for y in 0..h {
            let src_off = y * row_stride;
            let expected_dst = if y % 2 == 0 {
                (y / 2) * row_stride
            } else {
                (half + y / 2) * row_stride
            };
            assert_eq!(
                &result[expected_dst..expected_dst + row_stride],
                &planar[src_off..src_off + row_stride]
            );
        }
    }

    // ---- InterlaceFields — YCbCr420 path ----

    #[test]
    fn interlace_ycbcr420_4x4() {
        // 4×4 YCbCr420: Y = 16 bytes, Cb = 4 bytes, Cr = 4 bytes.
        let w = 4;
        let h = 4;
        let y_size = w * h; // 16
        let uv_w = 2;
        let uv_h = 2;
        let c_size = uv_w * uv_h; // 4
        let total = y_size + c_size * 2; // 24

        let mut planar = vec![0u8; total];
        // Fill Y plane with row-major indices
        for row in 0..h {
            for col in 0..w {
                planar[row * w + col] = u8::try_from(row * 16 + col).unwrap();
            }
        }
        // Fill Cb plane
        for row in 0..uv_h {
            for col in 0..uv_w {
                planar[y_size + row * uv_w + col] = u8::try_from(100 + row * 4 + col).unwrap();
            }
        }
        // Fill Cr plane
        for row in 0..uv_h {
            for col in 0..uv_w {
                planar[y_size + c_size + row * uv_w + col] = u8::try_from(200 + row * 4 + col).unwrap();
            }
        }

        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Ycbcr420,
        );
        assert_eq!(result.len(), total);

        let half_h = h.div_ceil(2); // 2
        let half_h_uv = uv_h.div_ceil(2); // 1

        // Verify Y plane interlace
        for y in 0..h {
            let src_off = y * w;
            let dst_off = if y % 2 == 0 { (y / 2) * w } else { (half_h + y / 2) * w };
            assert_eq!(
                &result[dst_off..dst_off + w],
                &planar[src_off..src_off + w],
                "Y row {y} mismatch"
            );
        }

        // Verify Cb plane interlace
        for y in 0..h / 2 {
            let src_off = y_size + y * uv_w;
            let dst_off = y_size
                + if y % 2 == 0 {
                    (y / 2) * uv_w
                } else {
                    (half_h_uv + y / 2) * uv_w
                };
            assert_eq!(
                &result[dst_off..dst_off + uv_w],
                &planar[src_off..src_off + uv_w],
                "Cb row {y} mismatch"
            );
        }

        // Verify Cr plane interlace
        let cr_base = y_size + c_size;
        for y in 0..h / 2 {
            let src_off = cr_base + y * uv_w;
            let dst_off = cr_base
                + if y % 2 == 0 {
                    (y / 2) * uv_w
                } else {
                    (half_h_uv + y / 2) * uv_w
                };
            assert_eq!(
                &result[dst_off..dst_off + uv_w],
                &planar[src_off..src_off + uv_w],
                "Cr row {y} mismatch"
            );
        }
    }

    #[test]
    fn interlace_ycbcr420_single_row() {
        // h=1: Y = 4 bytes, Cb = 2 bytes (uv_w=2, uv_h=1), Cr = 2 bytes.
        // Chroma planes have h/2=0 loops, so Cb/Cr remain zero-initialized.
        let w: usize = 4;
        let h: usize = 1;
        let uv_w = w.div_ceil(2);
        let uv_h = h.div_ceil(2);
        let y_size = w * h;
        let c_size = uv_w * uv_h;
        let planar = vec![0x42u8; y_size + c_size * 2];
        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Ycbcr420,
        );
        assert_eq!(result.len(), planar.len());
        // Y plane preserved (trivial for 1 row)
        assert_eq!(&result[..y_size], &planar[..y_size]);
        // Cb/Cr planes are zero — h/2=0 loops skip chroma interlace
        assert_eq!(&result[y_size..y_size + c_size], &[0u8; 2]);
        assert_eq!(&result[y_size + c_size..], &[0u8; 2]);
    }
    #[test]
    fn interlace_ycbcr420_odd_dimensions() {
        // w=6, h=5: odd height forces ceiling division in uv dimensions.
        let w: usize = 6;
        let h: usize = 5;
        let uv_w = w.div_ceil(2); // 3
        let uv_h = h.div_ceil(2); // 3
        let y_size = w * h; // 30
        let c_size = uv_w * uv_h; // 9
        let total = y_size + c_size * 2; // 48

        let mut planar = vec![0u8; total];
        for (i, item) in planar.iter_mut().enumerate().take(total) {
            *item = u8::try_from(i % 251).unwrap();
        }

        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Ycbcr420,
        );
        assert_eq!(result.len(), total);

        let half_h = h.div_ceil(2); // 3
        let half_h_uv = uv_h.div_ceil(2); // 2

        // Y plane
        for y in 0..h {
            let src_off = y * w;
            let dst_off = if y % 2 == 0 { (y / 2) * w } else { (half_h + y / 2) * w };
            assert_eq!(&result[dst_off..dst_off + w], &planar[src_off..src_off + w]);
        }

        // Cb plane
        for y in 0..h / 2 {
            let src_off = y_size + y * uv_w;
            let dst_off = y_size
                + if y % 2 == 0 {
                    (y / 2) * uv_w
                } else {
                    (half_h_uv + y / 2) * uv_w
                };
            assert_eq!(&result[dst_off..dst_off + uv_w], &planar[src_off..src_off + uv_w]);
        }

        // Cr plane
        let cr_base = y_size + c_size;
        for y in 0..h / 2 {
            let src_off = cr_base + y * uv_w;
            let dst_off = cr_base
                + if y % 2 == 0 {
                    (y / 2) * uv_w
                } else {
                    (half_h_uv + y / 2) * uv_w
                };
            assert_eq!(&result[dst_off..dst_off + uv_w], &planar[src_off..src_off + uv_w]);
        }
    }

    // ---- Neutral chroma invariant ----

    #[test]
    fn neutral_chroma_always_128() {
        // For any gray input (R=G=B), Cb and Cr must be 128.
        for v in [0i32, 16, 64, 128, 192, 235, 255] {
            assert_eq!(bt601_cb(v, v, v), 128, "neutral Cb failed at gray={v}");
            assert_eq!(bt601_cr(v, v, v), 128, "neutral Cr failed at gray={v}");
        }
    }

    #[test]
    fn interlace_2bpp_odd_height() {
        // h=3 with a non-Ycbcr420 encoding: verify even/odd row sorting.
        let w = 2;
        let h = 3;
        let row_stride = w * 2;
        let mut planar = Vec::with_capacity(row_stride * h);
        for row in 0..h {
            for col in 0..row_stride {
                planar.push(u8::try_from(row * 16 + col).unwrap());
            }
        }
        let result = interlace_fields(
            &planar,
            i32::try_from(w).unwrap(),
            i32::try_from(h).unwrap(),
            Encoding::Yuv422,
        );
        assert_eq!(result.len(), planar.len());

        let half = h.div_ceil(2); // 2
        // Expected order: row0 (even→0), row2 (even→4), row1 (odd→8)
        for y in 0..h {
            let src_off = y * row_stride;
            let expected_dst = if y % 2 == 0 {
                (y / 2) * row_stride
            } else {
                (half + y / 2) * row_stride
            };
            assert_eq!(
                &result[expected_dst..expected_dst + row_stride],
                &planar[src_off..src_off + row_stride]
            );
        }
    }
}
