//! AArch64 NEON SIMD implementations for pixel conversions.
//! Only compiled when `--features simd` is enabled and target is `aarch64`.

use core::arch::aarch64::*;

/// SAFETY: must only be called on `aarch64` where NEON is guaranteed.
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn rgb565_row_to_bgra_neon(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    debug_assert_eq!(dst.len(), n_pixels * 4);

    let n_eights = n_pixels / 8;
    for i in 0..n_eights {
        let input = src.as_ptr().add(i * 16);
        let output = dst.as_mut_ptr().add(i * 32);

        // Load 8 RGB565 pixels (16 bytes) as little-endian u16 pairs.
        let data = vld1q_u8(input);
        let pixels = vreinterpretq_u16_u8(data);

        // Extract R5 (bits 15-11), G6 (bits 10-5), B5 (bits 4-0).
        let r5 = vandq_u16(vshrq_n_u16(pixels, 11), vdupq_n_u16(0x1F));
        let g6 = vandq_u16(vshrq_n_u16(pixels, 5), vdupq_n_u16(0x3F));
        let b5 = vandq_u16(pixels, vdupq_n_u16(0x1F));

        // MSB replicate 5->8 bits: (v << 3) | (v >> 2)
        let r8 = vorrq_u16(vshlq_n_u16(r5, 3), vshrq_n_u16(r5, 2));
        // MSB replicate 6->8 bits: (v << 2) | (v >> 4)
        let g8 = vorrq_u16(vshlq_n_u16(g6, 2), vshrq_n_u16(g6, 4));
        let b8 = vorrq_u16(vshlq_n_u16(b5, 3), vshrq_n_u16(b5, 2));

        // Narrow 16-bit channels to bytes.
        let r = vmovn_u16(r8);
        let g = vmovn_u16(g8);
        let b = vmovn_u16(b8);
        let a = vdup_n_u8(255);

        // 4-way interleave store: [B, G, R, A, B, G, R, A, ...].
        vst4_u8(output, uint8x8x4_t(b, g, r, a));
    }

    let rem = n_pixels % 8;
    if rem != 0 {
        let rem_start = n_eights * 8;
        super::scalar::rgb565_row_to_bgra_scalar(&src[rem_start * 2..], &mut dst[rem_start * 4..]);
    }
}

/// SAFETY: must only be called on `aarch64` where NEON is guaranteed.
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn rgb555_row_to_bgra_neon(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    debug_assert_eq!(dst.len(), n_pixels * 4);

    let n_eights = n_pixels / 8;
    for i in 0..n_eights {
        let input = src.as_ptr().add(i * 16);
        let output = dst.as_mut_ptr().add(i * 32);

        let data = vld1q_u8(input);
        let pixels = vreinterpretq_u16_u8(data);

        // Extract R5 (bits 14-10), G5 (bits 9-5), B5 (bits 4-0).
        let r5 = vandq_u16(vshrq_n_u16(pixels, 10), vdupq_n_u16(0x1F));
        let g5 = vandq_u16(vshrq_n_u16(pixels, 5), vdupq_n_u16(0x1F));
        let b5 = vandq_u16(pixels, vdupq_n_u16(0x1F));

        // MSB replicate 5->8 bits: (v << 3) | (v >> 2)
        let r8 = vorrq_u16(vshlq_n_u16(r5, 3), vshrq_n_u16(r5, 2));
        let g8 = vorrq_u16(vshlq_n_u16(g5, 3), vshrq_n_u16(g5, 2));
        let b8 = vorrq_u16(vshlq_n_u16(b5, 3), vshrq_n_u16(b5, 2));

        let r = vmovn_u16(r8);
        let g = vmovn_u16(g8);
        let b = vmovn_u16(b8);
        let a = vdup_n_u8(255);

        vst4_u8(output, uint8x8x4_t(b, g, r, a));
    }

    let rem = n_pixels % 8;
    if rem != 0 {
        let rem_start = n_eights * 8;
        super::scalar::rgb555_row_to_bgra_scalar(&src[rem_start * 2..], &mut dst[rem_start * 4..]);
    }
}

/// SAFETY: must only be called on `aarch64` where NEON is guaranteed.
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
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

/// SAFETY: must only be called on `aarch64` where NEON is guaranteed.
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
        out[i * 4] = crate::yuv::clamp(b_arr[i]);
        out[i * 4 + 1] = crate::yuv::clamp(g_arr[i]);
        out[i * 4 + 2] = crate::yuv::clamp(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

/// SAFETY: must only be called on `aarch64` where NEON is guaranteed.
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

/// Convert 4 YCbCr 4:2:0 pixels sharing Cb/Cr to 4 BGRA pixels (16 bytes)
/// using AArch64 NEON intrinsics.
///
/// Chroma contributions are precomputed (scalar once) then splatted into 4-wide
/// vectors.  Final interleave uses `vzip_s16` + `vqmovun_s16` for saturated pack.
#[inline]
#[must_use]
#[allow(clippy::similar_names)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn yuv420_quad_to_bgra_neon(quad: &[u8; 6]) -> [u8; 16] {
    use core::arch::aarch64::{
        vaddq_s32, vcombine_s16, vdup_n_s16, vdupq_n_s32, vget_low_u16, vld1_u8, vmovl_u8, vmovl_u16, vqmovn_s32,
        vqmovun_s16, vreinterpretq_s32_u32, vst1_u8, vsubq_s32, vzip_s16,
    };

    // ---- Precompute chroma contributions (scalar, once for all 4 pixels) ----
    let cb = i32::from(quad[4]) - 128;
    let cr = i32::from(quad[5]) - 128;
    let rc = (cr * 359) >> 8;
    let gb = (cb * 88) >> 8;
    let gr = (cr * 183) >> 8;
    let bc = (cb * 454) >> 8;

    // ---- Load 4 Y values and zero-extend to 32-bit ----
    // Pad to 8 bytes so vld1_u8 does not read past the logical input.
    let y_arr: [u8; 8] = [quad[0], quad[1], quad[2], quad[3], 0, 0, 0, 0];
    let y8 = vld1_u8(y_arr.as_ptr());
    let y16 = vmovl_u8(y8);
    let y32 = vmovl_u16(vget_low_u16(y16));
    let y = vreinterpretq_s32_u32(y32);

    // ---- Splat chroma contributions and compute R/G/B in parallel ----
    let rc_splat = vdupq_n_s32(rc);
    let gb_splat = vdupq_n_s32(gb);
    let gr_splat = vdupq_n_s32(gr);
    let bc_splat = vdupq_n_s32(bc);

    let r = vaddq_s32(y, rc_splat);
    let g = vsubq_s32(vsubq_s32(y, gb_splat), gr_splat);
    let b = vaddq_s32(y, bc_splat);

    // ---- Narrow i32 -> i16 (saturating) and interleave to BGRA ----
    let r16 = vqmovn_s32(r);
    let g16 = vqmovn_s32(g);
    let b16 = vqmovn_s32(b);
    let a16 = vdup_n_s16(255);

    // vzip interleaves two int16x4_t into (even, odd) halves.
    // bg.0 = [B0, G0, B1, G1],  bg.1 = [B2, G2, B3, G3]
    // ra.0 = [R0, 255, R1, 255], ra.1 = [R2, 255, R3, 255]
    let bg = vzip_s16(b16, g16);
    let ra = vzip_s16(r16, a16);

    // Second zip produces per-pixel BGRA quads:
    // lo.0 = [B0, G0, R0, 255], lo.1 = [B1, G1, R1, 255]
    // hi.0 = [B2, G2, R2, 255], hi.1 = [B3, G3, R3, 255]
    let lo = vzip_s16(bg.0, ra.0);
    let hi = vzip_s16(bg.1, ra.1);

    // Combine into 128-bit vectors and saturate-narrow to u8.
    let combined_lo = vcombine_s16(lo.0, lo.1);
    let combined_hi = vcombine_s16(hi.0, hi.1);

    let out_lo = vqmovun_s16(combined_lo);
    let out_hi = vqmovun_s16(combined_hi);

    // ---- Store ----
    let mut out = [0u8; 16];
    vst1_u8(out.as_mut_ptr(), out_lo);
    vst1_u8(out.as_mut_ptr().add(8), out_hi);
    out
}

/// Convert one UYVY quad (4 bytes) to two BGRA pixels (8 bytes).
///
/// Uses NEON for the load + zero-extend, then scalar BT.601 (same algorithm as
/// the SSE2 variant).  The 2-pixel width does not justify a full 4-lane NEON
/// pipeline, but the function is kept in the NEON module for symmetry.
#[inline]
#[must_use]
#[allow(clippy::similar_names)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn uyvy_quad_to_bgra_neon(quad: &[u8; 4]) -> [u8; 8] {
    use core::arch::aarch64::{vgetq_lane_u16, vld1_u8, vmovl_u8};

    // Load 4 UYVY bytes padded to 8 for safe vld1_u8.
    let padded: [u8; 8] = [quad[0], quad[1], quad[2], quad[3], 0, 0, 0, 0];
    let data = vld1_u8(padded.as_ptr());
    // Zero-extend bytes to 16-bit words: [U, Y0, V, Y1, 0, 0, 0, 0]
    let w = vmovl_u8(data);

    // Extract via vgetq_lane_u16.
    let u = vgetq_lane_u16(w, 0) as i32;
    let y0 = vgetq_lane_u16(w, 1) as i32;
    let v = vgetq_lane_u16(w, 2) as i32;
    let y1 = vgetq_lane_u16(w, 3) as i32;

    // BT.601 with Q8 fixed-point.
    let r0 = crate::yuv::clamp(y0 + (((v - 128) * 359) >> 8));
    let g0 = crate::yuv::clamp(y0 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b0 = crate::yuv::clamp(y0 + (((u - 128) * 454) >> 8));

    let r1 = crate::yuv::clamp(y1 + (((v - 128) * 359) >> 8));
    let g1 = crate::yuv::clamp(y1 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b1 = crate::yuv::clamp(y1 + (((u - 128) * 454) >> 8));

    [b0, g0, r0, 255, b1, g1, r1, 255]
}

/// Convert two UYVY quads (8 bytes) to four BGRA pixels (16 bytes).
///
/// Processes all 4 pixels in parallel with 32-bit NEON arithmetic.
/// Uses `vtbl1_u8` for byte-gather and `vmulq_s32`/`vshrq_n_s32` for the BT.601
/// multiply-shift steps directly in NEON registers.
#[inline]
#[must_use]
#[allow(clippy::similar_names)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn uyvy_double_quad_to_bgra_neon(quads: &[u8; 8]) -> [u8; 16] {
    use core::arch::aarch64::{
        vaddq_s32, vcombine_s16, vdup_n_s16, vdupq_n_s32, vget_low_u16, vld1_u8, vmovl_u8, vmovl_u16, vmulq_s32,
        vqmovn_s32, vqmovun_s16, vreinterpretq_s32_u32, vshrq_n_s32, vst1_u8, vsubq_s32, vtbl1_u8, vzip_s16,
    };

    // Load 8 UYVY bytes: [U0, Y0_0, V0, Y0_1, U1, Y1_0, V1, Y1_1]
    let data = vld1_u8(quads.as_ptr());

    // Table-lookup to gather Y at indices [1, 3, 5, 7].
    let ys = {
        let idx: [u8; 8] = [1, 3, 5, 7, 0, 0, 0, 0];
        let tbl = vld1_u8(idx.as_ptr());
        let ys8 = vtbl1_u8(data, tbl); // low 4 = [Y0_0, Y0_1, Y1_0, Y1_1]
        let ys16 = vmovl_u8(ys8);
        let ys32 = vmovl_u16(vget_low_u16(ys16));
        vreinterpretq_s32_u32(ys32)
    };

    // Table-lookup to gather U (Cb) at indices [0, 0, 4, 4], then centre.
    let us = {
        let idx: [u8; 8] = [0, 0, 4, 4, 0, 0, 0, 0];
        let tbl = vld1_u8(idx.as_ptr());
        let us8 = vtbl1_u8(data, tbl);
        let us16 = vmovl_u8(us8);
        let us32 = vmovl_u16(vget_low_u16(us16));
        vsubq_s32(vreinterpretq_s32_u32(us32), vdupq_n_s32(128))
    };

    // Table-lookup to gather V (Cr) at indices [2, 2, 6, 6], then centre.
    let vs = {
        let idx: [u8; 8] = [2, 2, 6, 6, 0, 0, 0, 0];
        let tbl = vld1_u8(idx.as_ptr());
        let vs8 = vtbl1_u8(data, tbl);
        let vs16 = vmovl_u8(vs8);
        let vs32 = vmovl_u16(vget_low_u16(vs16));
        vsubq_s32(vreinterpretq_s32_u32(vs32), vdupq_n_s32(128))
    };

    // BT.601 with Q8 fixed-point:  R = Y + (Cr * 359) >> 8
    let r = vaddq_s32(ys, vshrq_n_s32(vmulq_s32(vs, vdupq_n_s32(359)), 8));

    // G = Y - (Cb * 88) >> 8 - (Cr * 183) >> 8
    let g = vsubq_s32(
        vsubq_s32(ys, vshrq_n_s32(vmulq_s32(us, vdupq_n_s32(88)), 8)),
        vshrq_n_s32(vmulq_s32(vs, vdupq_n_s32(183)), 8),
    );

    // B = Y + (Cb * 454) >> 8
    let b = vaddq_s32(ys, vshrq_n_s32(vmulq_s32(us, vdupq_n_s32(454)), 8));

    // ---- Narrow i32 -> i16 (saturating) and interleave to BGRA ----
    let r16 = vqmovn_s32(r);
    let g16 = vqmovn_s32(g);
    let b16 = vqmovn_s32(b);
    let a16 = vdup_n_s16(255);

    let bg = vzip_s16(b16, g16);
    let ra = vzip_s16(r16, a16);

    let lo = vzip_s16(bg.0, ra.0);
    let hi = vzip_s16(bg.1, ra.1);

    let combined_lo = vcombine_s16(lo.0, lo.1);
    let combined_hi = vcombine_s16(hi.0, hi.1);

    let out_lo = vqmovun_s16(combined_lo);
    let out_hi = vqmovun_s16(combined_hi);

    let mut out = [0u8; 16];
    vst1_u8(out.as_mut_ptr(), out_lo);
    vst1_u8(out.as_mut_ptr().add(8), out_hi);
    out
}
