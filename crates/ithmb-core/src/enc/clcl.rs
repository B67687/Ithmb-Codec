// SPDX-License-Identifier: MIT
// Encoder: CLCL — separate Cb/Cr nibble planes, 2 bytes per pixel

#[allow(unused_imports)]
use crate::enc::helpers::{bt601_cb, bt601_cr, bt601_y};
use crate::pixel_utils::clamp_u8;

/// Encode BGRA pixels to CLCL nibble-chroma planar format.
///
/// Output layout (2 Bpp):
///   Y plane:   `w × h` bytes (full 8-bit luma)
///   Cb plane:  `(w × h) / 2` bytes (packed nibbles, odd pixel high nibble)
///   Cr plane:  `(w × h) / 2` bytes (packed nibbles, odd pixel high nibble)
///
/// Each chroma nibble is `(value >> 4)` — the top 4 bits.
#[must_use]
#[allow(unreachable_code)]
pub fn encode_clcl(bgra: &[u8], w: i32, h: i32) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let n = wu * hu;
    let chroma_len = n.div_ceil(2); // ceiling division for nibble packing
    let mut out = vec![0u8; n + chroma_len + chroma_len];

    // Y plane
    for (i, chunk) in bgra.chunks_exact(4).take(n).enumerate() {
        let r = i32::from(chunk[2]);
        let g = i32::from(chunk[1]);
        let b = i32::from(chunk[0]);
        out[i] = clamp_u8(bt601_y(r, g, b));
    }

    // Cb plane (packed nibbles: odd pixel in high nibble, even in low)

    // Cb plane (packed nibbles: odd pixel in high nibble, even in low)
    let cb_off = n;
    for i in 0..n {
        let px = i * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);
        let cb_nibble = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
        let ci = i / 2;
        if i & 1 == 0 {
            // Even → low nibble
            out[cb_off + ci] = cb_nibble;
        } else {
            // Odd → high nibble
            out[cb_off + ci] |= cb_nibble << 4;
        }
    }

    // Cr plane (same packing)
    let cr_off = n + chroma_len;
    for i in 0..n {
        let px = i * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);
        let cr_nibble = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
        let ci = i / 2;
        if i & 1 == 0 {
            out[cr_off + ci] = cr_nibble;
        } else {
            out[cr_off + ci] |= cr_nibble << 4;
        }
    }

    out
}
