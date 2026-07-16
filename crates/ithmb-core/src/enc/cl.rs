// SPDX-License-Identifier: MIT
// Encoder: CL — per-pixel nibble chroma, 2 bytes per pixel

use crate::enc::helpers::{bt601_cb, bt601_cr, bt601_y};
use crate::pixel_utils::clamp_u8;

/// Encode BGRA pixels to CL per-pixel nibble chroma format.
///
/// Output layout (2 Bpp):
///   Y plane: `w × h` bytes (full 8-bit luma)
///   CbCr plane: `w × h` bytes (Cr in high nibble, Cb in low nibble)
#[must_use]
#[allow(unreachable_code)]
pub fn encode_cl(bgra: &[u8], w: i32, h: i32) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let n = wu * hu;
    let mut out = vec![0u8; n * 2];

    for i in 0..n {
        let px = i * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);

        // Y
        out[i] = clamp_u8(bt601_y(r, g, b));

        // CbCr byte: high nibble = Cr, low nibble = Cb
        let cb_nibble = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
        let cr_nibble = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
        out[n + i] = (cr_nibble << 4) | cb_nibble;
    }

    out
}
