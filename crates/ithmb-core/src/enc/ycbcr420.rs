// SPDX-License-Identifier: MIT
// Encoder: YCbCr 4:2:0 — planar, 3 bytes per pixel effective

#[allow(unused_imports)]
use crate::enc::helpers::{bt601_cb, bt601_cr, bt601_y};
use crate::pixel_utils::clamp_u8;

/// Encode BGRA pixels to planar YCbCr 4:2:0.
///
/// Output layout: Y plane (w×h), then Cb plane (uvW×uvH), then Cr plane (uvW×uvH).
/// Chroma is averaged over 2×2 blocks. Uses BT.601 colour conversion.
///
/// When `swap_chroma` is true the output order is Y, Cr, Cb.
#[must_use]
#[allow(unreachable_code)]
pub fn encode_ycbcr420(bgra: &[u8], w: i32, h: i32, swap_chroma: bool) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let uv_w = wu.div_ceil(2); // ceiling division
    let uv_h = hu.div_ceil(2);

    let y_size = wu * hu;
    let c_size = uv_w * uv_h;
    let mut out = vec![0u8; y_size + c_size * 2];

    // Fill Y plane (SIMD-accelerated)
    for y in 0..hu {
        for x in 0..wu {
            let px = (y * wu + x) * 4;
            let r = i32::from(bgra[px + 2]);
            let g = i32::from(bgra[px + 1]);
            let b = i32::from(bgra[px]);
            let y_val = clamp_u8(bt601_y(r, g, b));
            out[y * wu + x] = y_val;
        }
    }

    let (first_chroma_off, second_chroma_off) = if swap_chroma {
        (y_size + c_size, y_size) // Cr first, then Cb
    } else {
        (y_size, y_size + c_size) // Cb first, then Cr
    };
    for cy in 0..uv_h {
        for cx in 0..uv_w {
            let mut sum_cb: i32 = 0;
            let mut sum_cr: i32 = 0;
            let mut count: i32 = 0;

            for dy in 0..2 {
                for dx in 0..2 {
                    let py = cy * 2 + dy;
                    let px = cx * 2 + dx;
                    if py < hu && px < wu {
                        let bgra_idx = (py * wu + px) * 4;
                        let r = i32::from(bgra[bgra_idx + 2]);
                        let g = i32::from(bgra[bgra_idx + 1]);
                        let b = i32::from(bgra[bgra_idx]);
                        sum_cb += bt601_cb(r, g, b);
                        sum_cr += bt601_cr(r, g, b);
                        count += 1;
                    }
                }
            }

            let cb_val = clamp_u8(sum_cb / count);
            let cr_val = clamp_u8(sum_cr / count);

            let ci = cy * uv_w + cx;
            if swap_chroma {
                out[first_chroma_off + ci] = cr_val; // Cr first
                out[second_chroma_off + ci] = cb_val; // Cb second
            } else {
                out[first_chroma_off + ci] = cb_val; // Cb first
                out[second_chroma_off + ci] = cr_val; // Cr second
            }
        }
    }

    out
}
