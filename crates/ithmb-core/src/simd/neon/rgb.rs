//! AArch64 NEON implementations for grayscale and RGB565/RGB555 pixel conversions.

use core::arch::aarch64::*;

/// # Safety
/// Loop-guard proof: `i + 16 <= n` bounds every iteration — the 16-byte
/// `vld1q_u8` read ends at `i + 16 <= n == gray.len()`, and the 64-byte
/// `vst4q_u8` write at `i * 4` ends at `i * 4 + 64 <= 4 * n == dst.len()`.
/// `vld1q`/`vst4q` accept unaligned addresses; the scalar remainder is checked
/// indexing. `dst` is owned (`vec![0u8; n * 4]`) — no caller size contract.
/// NEON mandatory on aarch64; aarch64-only caller.
#[inline]
#[allow(unsafe_op_in_unsafe_fn, reason = "strict migration")]
pub(crate) unsafe fn fill_gray_row_neon(gray: &[u8]) -> Vec<u8> {
    let n = gray.len();
    let mut dst = vec![0u8; n * 4];
    let mut i = 0;

    let alpha = vdupq_n_u8(255);
    while i + 16 <= n {
        let v = vld1q_u8(gray.as_ptr().add(i));
        // vst4q_u8 interleaves 4 channels of 16 elements:
        // each gray byte -> [g, g, g, 255].
        vst4q_u8(dst.as_mut_ptr().add(i * 4), uint8x16x4_t(v, v, v, alpha));
        i += 16;
    }

    for (j, &g) in gray.iter().enumerate().skip(i) {
        let o = j * 4;
        dst[o] = g;
        dst[o + 1] = g;
        dst[o + 2] = g;
        dst[o + 3] = 255;
    }
    dst
}

/// # Safety
/// Caller contract: `dst.len() == (n / 2) * 4` where `n = src.len()` (debug
/// assertion only — release relies on the caller, same as the SSE2 variant).
/// Loop proof: `i + 16 <= n` bounds the 16-byte load; the 32-byte store at
/// `i * 2` ends at `2 * i + 32 <= 2 * n == dst.len()`. The `cast::<u16>()`
/// load needs no alignment (ARM permits unaligned; `vld1q` has no align
/// requirement). The scalar remainder re-slices exactly (`src[i..]` = n-i
/// bytes, `dst[i * 2..]` = 2*(n-i) bytes) for the matching scalar contract.
/// NEON mandatory on aarch64; aarch64-only caller.
///
/// Convert one row of RGB565 pixels to BGRA8 using AArch64 NEON.
///
/// Processes 8 pixels per iteration: loads 16 bytes, extracts R5/G6/B5,
/// MSB-replicates to 8-bit, and stores 32 bytes of interleaved BGRA.
#[inline]
#[allow(unsafe_op_in_unsafe_fn, reason = "strict migration")]
pub(crate) unsafe fn rgb565_row_to_bgra_neon(src: &[u8], dst: &mut [u8]) {
    let n = src.len();
    debug_assert_eq!(dst.len(), (n / 2) * 4);

    let mask5 = vdupq_n_u16(0x1F);
    let mask6 = vdupq_n_u16(0x3F);
    let alpha = vdup_n_u8(255);

    let mut i = 0usize;
    while i + 16 <= n {
        let src_ptr = src.as_ptr().add(i);
        let dst_ptr = dst.as_mut_ptr().add(i * 2);

        // Load 8 pixels (16 bytes) as 8 × u16.
        let data = vld1q_u16(src_ptr.cast::<u16>());

        // Extract R5 (bits 15-11), G6 (bits 10-5), B5 (bits 4-0).
        let r5 = vandq_u16(vshrq_n_u16(data, 11), mask5);
        let g6 = vandq_u16(vshrq_n_u16(data, 5), mask6);
        let b5 = vandq_u16(data, mask5);

        // MSB replicate: 5-bit -> (v<<3)|(v>>2), 6-bit -> (v<<2)|(v>>4).
        let r8 = vorrq_u16(vshlq_n_u16(r5, 3), vshrq_n_u16(r5, 2));
        let g8 = vorrq_u16(vshlq_n_u16(g6, 2), vshrq_n_u16(g6, 4));
        let b8 = vorrq_u16(vshlq_n_u16(b5, 3), vshrq_n_u16(b5, 2));

        // Narrow u16 -> u8 (saturating).
        let r_u8 = vqmovn_u16(r8);
        let g_u8 = vqmovn_u16(g8);
        let b_u8 = vqmovn_u16(b8);

        // Interleave and store: BGRA for 8 pixels (32 bytes).
        vst4_u8(dst_ptr, uint8x8x4_t(b_u8, g_u8, r_u8, alpha));

        i += 16;
    }

    // Remainder pixels (scalar fallback).
    if i < n {
        super::super::scalar::rgb565_row_to_bgra_scalar(&src[i..], &mut dst[i * 2..]);
    }
}

/// # Safety
/// Caller contract: `dst.len() == (n / 2) * 4` where `n = src.len()` (debug
/// assertion only — release relies on the caller, same as the SSE2 variant).
/// Loop proof: `i + 16 <= n` bounds the 16-byte load; the 32-byte store at
/// `i * 2` ends at `2 * i + 32 <= 2 * n == dst.len()`. The `cast::<u16>()`
/// load needs no alignment (ARM permits unaligned; `vld1q` has no align
/// requirement). The scalar remainder re-slices exactly for the matching
/// scalar contract. NEON mandatory on aarch64; aarch64-only caller.
///
/// Convert one row of RGB555 pixels to BGRA8 using AArch64 NEON.
///
/// Processes 8 pixels per iteration: loads 16 bytes, extracts R5/G5/B5,
/// MSB-replicates to 8-bit, and stores 32 bytes of interleaved BGRA.
#[inline]
#[allow(unsafe_op_in_unsafe_fn, reason = "strict migration")]
pub(crate) unsafe fn rgb555_row_to_bgra_neon(src: &[u8], dst: &mut [u8]) {
    let n = src.len();
    debug_assert_eq!(dst.len(), (n / 2) * 4);

    let mask5 = vdupq_n_u16(0x1F);
    let alpha = vdup_n_u8(255);

    let mut i = 0usize;
    while i + 16 <= n {
        let src_ptr = src.as_ptr().add(i);
        let dst_ptr = dst.as_mut_ptr().add(i * 2);

        // Load 8 pixels (16 bytes) as 8 × u16.
        let data = vld1q_u16(src_ptr.cast::<u16>());

        // Extract R5 (bits 14-10), G5 (bits 9-5), B5 (bits 4-0).
        let r5 = vandq_u16(vshrq_n_u16(data, 10), mask5);
        let g5 = vandq_u16(vshrq_n_u16(data, 5), mask5);
        let b5 = vandq_u16(data, mask5);

        // MSB replicate 5->8 bits: (v << 3) | (v >> 2).
        let r8 = vorrq_u16(vshlq_n_u16(r5, 3), vshrq_n_u16(r5, 2));
        let g8 = vorrq_u16(vshlq_n_u16(g5, 3), vshrq_n_u16(g5, 2));
        let b8 = vorrq_u16(vshlq_n_u16(b5, 3), vshrq_n_u16(b5, 2));

        // Narrow u16 -> u8 (saturating).
        let r_u8 = vqmovn_u16(r8);
        let g_u8 = vqmovn_u16(g8);
        let b_u8 = vqmovn_u16(b8);

        // Interleave and store: BGRA for 8 pixels (32 bytes).
        vst4_u8(dst_ptr, uint8x8x4_t(b_u8, g_u8, r_u8, alpha));

        i += 16;
    }

    // Remainder pixels (scalar fallback).
    if i < n {
        super::super::scalar::rgb555_row_to_bgra_scalar(&src[i..], &mut dst[i * 2..]);
    }
}
