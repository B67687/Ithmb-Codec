// SPDX-License-Identifier: MIT
// Encoder: Reordered RGB555 — big-endian byte order with Z-order spatial reordering

use super::rgb555::encode_rgb555;
use crate::reordered_rgb555::morton_interleave;

/// Encode BGRA pixels to Reordered RGB555 (big-endian byte order, Z-order).
///
/// 1. Encodes each pixel as standard RGB555 with big-endian byte order.
/// 2. Deranges pixel positions via Morton Z-order curve for spatial locality.
///
/// The output is `w × h × 2` bytes for square power-of-2 dimensions (the
/// only dimensions this format is used with in practice). For other sizes
/// the output may contain gaps at unused Z-order positions.
#[must_use]
pub fn encode_reordered_rgb555(bgra: &[u8], w: i32, h: i32, big_endian: bool) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let n = wu * hu;

    // Step 1: encode as standard RGB555 (big-endian, no channel swap)
    let row_major = encode_rgb555(bgra, w, h, big_endian, false);

    // Step 2: determine bits needed for Z-order index.
    //   We need enough bits to cover both dimensions.
    let bits = (std::cmp::max(wu, hu) as f64).log2().ceil() as u32;

    // Compute the maximum Z-order index (in 2-byte words) to size the buffer.
    let max_z = if wu > 0 && hu > 0 {
        morton_interleave((wu - 1) as u32, (hu - 1) as u32, bits) as usize + 1
    } else {
        0
    };

    // Output buffer sized to fit the largest Z-order index.
    // For square power-of-2 images this equals w*h (compact).
    let out_words = std::cmp::max(max_z, n);
    let mut out = vec![0u8; out_words * 2];

    // Place each pixel at its Z-order position in the output.
    for y in 0..hu {
        for x in 0..wu {
            let z = morton_interleave(x as u32, y as u32, bits) as usize;
            let src = (y * wu + x) * 2;
            let dst = z * 2;
            if dst + 1 < out.len() {
                out[dst] = row_major[src];
                out[dst + 1] = row_major[src + 1];
            }
        }
    }
    out
}
