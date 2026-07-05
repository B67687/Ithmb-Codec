// SPDX-License-Identifier: MIT
// Encoder: RGB565 — 2 bytes per pixel, R(5) | G(6) | B(5)

/// Encode BGRA pixels to RGB565 (2 bytes per pixel).
///
/// Each pixel packs R(5) | G(6) | B(5) into a 16-bit word.
/// `big_endian` controls byte order within each 16-bit word.
#[must_use]
pub fn encode_rgb565(bgra: &[u8], w: i32, h: i32, big_endian: bool) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let n = wu * hu;
    let mut out = vec![0u8; n * 2];

    for i in 0..n {
        let px = i * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        // alpha at px+3 unused

        let pixel = (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16;
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
