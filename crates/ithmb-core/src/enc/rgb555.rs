// SPDX-License-Identifier: MIT
// Encoder: RGB555 — 2 bytes per pixel, 5 bits per channel, top bit unused

/// Encode BGRA pixels to RGB555 (2 bytes per pixel, top bit unused).
///
/// Default layout (swap_rgb = false): `xRRRRRGGGGGBBBBB`
/// BGR15 layout   (swap_rgb = true):  `xBBBBBGGGGGRRRRR`
///
/// `big_endian` controls per-pixel byte order.
#[must_use]
#[allow(unreachable_code)]
pub fn encode_rgb555(bgra: &[u8], w: i32, h: i32, big_endian: bool, swap_rgb: bool) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let n = wu * hu;
    let mut out = vec![0u8; n * 2];

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        crate::simd::enc::bgra_to_rgb555(bgra, &mut out, big_endian, swap_rgb);
        return out;
    }

    for i in 0..n {
        let px = i * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        // alpha at px+3 unused

        let r5 = r >> 3;
        let g5 = g >> 3;
        let b5 = b >> 3;

        let pixel: u16 = if swap_rgb {
            // BGR15: high 5 = B, mid 5 = G, low 5 = R
            ((b5 << 10) | (g5 << 5) | r5) as u16
        } else {
            // Default: high 5 = R, mid 5 = G, low 5 = B
            ((r5 << 10) | (g5 << 5) | b5) as u16
        };

        let bytes = if big_endian {
            pixel.to_be_bytes()
        } else {
            pixel.to_le_bytes()
        };
        let o = i * 2;
        out[o] = bytes[0];
        out[o + 1] = bytes[1];
    }

    out
}
