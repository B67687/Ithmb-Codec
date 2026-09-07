//! AArch64 NEON implementations for CL (nibble-chroma interleaved) pixel conversions.

use core::arch::aarch64::*;

/// # Safety
/// Exact-size-by-type: `quad: &[u8; 8]` bounds the Y load (exactly 8 bytes)
/// and all `quad[4 + i]` chroma reads (`i < 4`). The `[i32; 4]` scratch arrays
/// are exactly 16 bytes for each `vld1q_s32` load / `vst1q_s32` store.
/// Output is assembled by scalar stores into `[0u8; 16]` — in-bounds by type.
/// NEON mandatory on aarch64; aarch64-only caller.
#[inline]
#[must_use]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_quad_to_bgra_neon(quad: &[u8; 8]) -> [u8; 16] {
    // ---- Pre-compute chroma contributions (scalar, one per pixel) ----
    // Matching SSE2 convention: low nibble = Cb, high nibble = Cr.
    let mut rc_arr = [0i32; 4];
    let mut gb_arr = [0i32; 4];
    let mut gr_arr = [0i32; 4];
    let mut bc_arr = [0i32; 4];
    for i in 0..4 {
        let raw = quad[4 + i];
        let cb = i32::from((raw & 0x0F) << 4) - 128;
        let cr = i32::from(raw & 0xF0) - 128;
        rc_arr[i] = (cr * 359) >> 8;
        gb_arr[i] = (cb * 88) >> 8;
        gr_arr[i] = (cr * 183) >> 8;
        bc_arr[i] = (cb * 454) >> 8;
    }

    // ---- Load 4 Y bytes and zero-extend to 32-bit (NEON) ----
    let y_bytes = vld1_u8(quad.as_ptr());
    let y_16 = vmovl_u8(y_bytes);
    let y_32 = vmovl_u16(vget_low_u16(y_16));
    let y = vreinterpretq_s32_u32(y_32);

    let rc = vld1q_s32(rc_arr.as_ptr());
    let gb = vld1q_s32(gb_arr.as_ptr());
    let gr = vld1q_s32(gr_arr.as_ptr());
    let bc = vld1q_s32(bc_arr.as_ptr());

    // ---- BT.601 in NEON ----
    // R = Y + rc
    // G = Y - gb - gr
    // B = Y + bc
    let r = vaddq_s32(y, rc);
    let g = vsubq_s32(vsubq_s32(y, gb), gr);
    let b = vaddq_s32(y, bc);

    let mut r_arr = [0i32; 4];
    let mut g_arr = [0i32; 4];
    let mut b_arr = [0i32; 4];
    vst1q_s32(r_arr.as_mut_ptr(), r);
    vst1q_s32(g_arr.as_mut_ptr(), g);
    vst1q_s32(b_arr.as_mut_ptr(), b);

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::pixel_utils::clamp_u8(b_arr[i]);
        out[i * 4 + 1] = crate::pixel_utils::clamp_u8(g_arr[i]);
        out[i * 4 + 2] = crate::pixel_utils::clamp_u8(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

/// # Safety
/// Caller contract: `src.len() >= 2 * n_pixels` (Y + chroma halves),
/// `dst.len() >= n_pixels * 4`. All indexing is checked (panics, never
/// corrupts): the main loop reads `y`/`chroma` at `i + 7` max with
/// `i + 8 <= n_pixels`, and writes `d_off + 32` max with `i * 4 + 32 <=
/// `n_pixels * 4`. Remainder loops re-check bounds per quad/pixel.
/// NEON mandatory on aarch64; aarch64-only caller.
#[inline]
#[allow(clippy::similar_names)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_row_to_bgra_neon(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    // Process 2 quads (8 pixels) per iteration.
    let full_end = (n_pixels / 8) * 8;
    let mut i = 0usize;
    while i < full_end {
        // Quad 0: pixels i..i+4
        let q0 = [
            y[i],
            y[i + 1],
            y[i + 2],
            y[i + 3],
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ];
        let out0 = cl_quad_to_bgra_neon(&q0);
        // Quad 1: pixels i+4..i+8
        let q1 = [
            y[i + 4],
            y[i + 5],
            y[i + 6],
            y[i + 7],
            chroma[i + 4],
            chroma[i + 5],
            chroma[i + 6],
            chroma[i + 7],
        ];
        let out1 = cl_quad_to_bgra_neon(&q1);
        let d_off = i * 4;
        dst[d_off..d_off + 16].copy_from_slice(&out0);
        dst[d_off + 16..d_off + 32].copy_from_slice(&out1);
        i += 8;
    }
    // Remainder: process one final quad if 4+ pixels remain.
    while i + 4 <= n_pixels {
        let q = [
            y[i],
            y[i + 1],
            y[i + 2],
            y[i + 3],
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ];
        let out = cl_quad_to_bgra_neon(&q);
        let d_off = i * 4;
        dst[d_off..d_off + 16].copy_from_slice(&out);
        i += 4;
    }
    // Remaining 0-3 pixels via scalar (can't form a full quad).
    for j in i..n_pixels {
        let cr = chroma[j] & 0xF0; // high nibble → Cr
        let cb = (chroma[j] & 0x0F) << 4; // low nibble → Cb
        let px = crate::yuv::yuv_to_bgra(y[j], cb, cr);
        let o = j * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
}
