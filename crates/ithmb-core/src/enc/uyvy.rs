// SPDX-License-Identifier: MIT
// Encoder: UYVY — YUV 4:2:2 packed, 2 bytes per pixel

use crate::enc::helpers::{bt601_cb, bt601_cr, bt601_y};
use crate::pixel_utils::clamp_u8;

/// Encode BGRA pixels to UYVY (YUV 4:2:2 packed, 2 bytes per pixel).
///
/// Layout per 2-pixel pair: `[Cb_avg, Y0, Cr_avg, Y1]`.
/// Chroma values are averaged across the pair. Uses BT.601 colour conversion.
#[must_use]
#[allow(unreachable_code)]
pub fn encode_uyvy(bgra: &[u8], w: i32, h: i32) -> Vec<u8> {
    let wu = w as usize;
    let mm = h as usize;
    // Each pair of pixels produces 4 bytes.  Allocate for ceil(w/2) pairs per row.
    let pairs_per_row = wu.div_ceil(2);
    let total_pairs = pairs_per_row * mm;
    let out_len = total_pairs * 4;
    let mut out = vec![0u8; out_len];

    let n = wu * mm;
    let mut px_i = 0;
    let mut o_i = 0;
    while px_i < n {
        let px = px_i * 4;
        let r0 = i32::from(bgra[px + 2]);
        let g0 = i32::from(bgra[px + 1]);
        let b0 = i32::from(bgra[px]);
        let y0 = clamp_u8(bt601_y(r0, g0, b0));
        let cb0 = bt601_cb(r0, g0, b0);
        let cr0 = bt601_cr(r0, g0, b0);

        if px_i + 1 < n {
            // Full pair: average chroma
            let px2 = (px_i + 1) * 4;
            let r1 = i32::from(bgra[px2 + 2]);
            let g1 = i32::from(bgra[px2 + 1]);
            let b1 = i32::from(bgra[px2]);
            let y1 = clamp_u8(bt601_y(r1, g1, b1));
            let cb1 = bt601_cb(r1, g1, b1);
            let cr1 = bt601_cr(r1, g1, b1);

            let cb_avg = clamp_u8(cb0.midpoint(cb1));
            let cr_avg = clamp_u8(cr0.midpoint(cr1));

            out[o_i] = cb_avg;
            out[o_i + 1] = y0;
            out[o_i + 2] = cr_avg;
            out[o_i + 3] = y1;
        } else {
            // Trailing pixel (odd width): use its own chroma
            out[o_i] = clamp_u8(cb0);
            out[o_i + 1] = y0;
            out[o_i + 2] = clamp_u8(cr0);
            out[o_i + 3] = 0;
        }

        px_i += 2;
        o_i += 4;
    }

    out
}
