//! AArch64 NEON implementations for YCbCr (4:2:0 and UYVY) pixel conversions.

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
    // br.0 = [B0, R0, B1, R1],  br.1 = [B2, R2, B3, R3]
    // ga.0 = [G0, 255, G1, 255], ga.1 = [G2, 255, G3, 255]
    let br = vzip_s16(b16, r16);
    let ga = vzip_s16(g16, a16);

    // Second zip produces per-pixel BGRA quads:
    // lo.0 = [B0, G0, R0, 255], lo.1 = [B1, G1, R1, 255]
    // hi.0 = [B2, G2, R2, 255], hi.1 = [B3, G3, R3, 255]
    let lo = vzip_s16(br.0, ga.0);
    let hi = vzip_s16(br.1, ga.1);

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
    let r0 = crate::pixel_utils::clamp_u8(y0 + (((v - 128) * 359) >> 8));
    let g0 = crate::pixel_utils::clamp_u8(y0 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b0 = crate::pixel_utils::clamp_u8(y0 + (((u - 128) * 454) >> 8));

    let r1 = crate::pixel_utils::clamp_u8(y1 + (((v - 128) * 359) >> 8));
    let g1 = crate::pixel_utils::clamp_u8(y1 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b1 = crate::pixel_utils::clamp_u8(y1 + (((u - 128) * 454) >> 8));

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

    let br = vzip_s16(b16, r16);
    let ga = vzip_s16(g16, a16);

    let lo = vzip_s16(br.0, ga.0);
    let hi = vzip_s16(br.1, ga.1);

    let combined_lo = vcombine_s16(lo.0, lo.1);
    let combined_hi = vcombine_s16(hi.0, hi.1);

    let out_lo = vqmovun_s16(combined_lo);
    let out_hi = vqmovun_s16(combined_hi);

    let mut out = [0u8; 16];
    vst1_u8(out.as_mut_ptr(), out_lo);
    vst1_u8(out.as_mut_ptr().add(8), out_hi);
    out
}

/// Convert a 2-row YCbCr 4:2:0 macroblock to BGRA using AArch64 NEON.
///
/// Each chroma position covers 4 Y pixels (2×2 block). Delegates to
/// [`yuv420_quad_to_bgra_neon`] which handles the NEON BT.601 arithmetic.
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn yuv420_row_pair_to_bgra_neon(
    y_row: &[u8],
    cb_row: &[u8],
    cr_row: &[u8],
    dst: &mut [u8],
    w: usize,
    cb_w: usize,
) {
    for cx in 0..cb_w {
        let quad = [
            y_row[cx * 2],
            y_row[cx * 2 + 1],
            y_row[w + cx * 2],
            y_row[w + cx * 2 + 1],
            cb_row[cx],
            cr_row[cx],
        ];
        let out = yuv420_quad_to_bgra_neon(&quad);
        let off = cx * 8;
        dst[off..off + 4].copy_from_slice(&out[0..4]);
        dst[off + 4..off + 8].copy_from_slice(&out[4..8]);
        let off2 = off + w * 4;
        dst[off2..off2 + 4].copy_from_slice(&out[8..12]);
        dst[off2 + 4..off2 + 8].copy_from_slice(&out[12..16]);
    }
}
