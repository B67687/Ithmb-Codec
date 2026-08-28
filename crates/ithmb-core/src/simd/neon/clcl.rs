//! AArch64 NEON implementations for CLCL (separate Cb/Cr nibble-chroma planes) pixel conversions.

use core::arch::aarch64::*;

/// Convert one CLCL row (separate Y/Cb/Cr nibble planes) to BGRA.
///
/// Uses NEON for nibble expansion and BT.601 YUV→RGB conversion in parallel.
#[inline]
#[allow(clippy::similar_names)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn clcl_row_to_bgra_neon(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    let full_batches = (width / 8) * 8;
    let mut i = 0usize;

    // BT.601 constants (shared across all batches)
    let cent = vdupq_n_s32(128);
    let c359 = vdupq_n_s32(359);
    let c88 = vdupq_n_s32(88);
    let c183 = vdupq_n_s32(183);
    let c454 = vdupq_n_s32(454);
    let low_mask = vdup_n_u8(0x0F);

    while i < full_batches {
        // ---- Load 8 Y bytes ----
        let y8 = vld1_u8(y.as_ptr().add(i));

        // ---- Load 4 Cb/Cr bytes (nibble-packed, 2 pixels/byte) into padded arrays ----
        let mut cb_tmp = [0u8; 8];
        cb_tmp[..4].copy_from_slice(&cb[i / 2..i / 2 + 4]);
        let mut cr_tmp = [0u8; 8];
        cr_tmp[..4].copy_from_slice(&cr[i / 2..i / 2 + 4]);
        let cb4 = vld1_u8(cb_tmp.as_ptr());
        let cr4 = vld1_u8(cr_tmp.as_ptr());

        // ---- Extract low/high nibbles ----
        let cb_lo = vand_u8(cb4, low_mask);
        let cb_hi = vshr_n_u8(cb4, 4);
        let cr_lo = vand_u8(cr4, low_mask);
        let cr_hi = vshr_n_u8(cr4, 4);

        // Interleave nibbles: [lo0, hi0, lo1, hi1, ..., lo3, hi3]
        let cb_zip = vzip_u8(cb_lo, cb_hi);
        let cr_zip = vzip_u8(cr_lo, cr_hi);

        // Expand nibbles to 8-bit: nibble << 4
        let cb8 = vshl_n_u8(cb_zip.0, 4);
        let cr8 = vshl_n_u8(cr_zip.0, 4);

        // ---- Widen Y, Cb, Cr to u16 then split for BT.601 ----
        let y16 = vmovl_u8(y8);
        let cb16 = vmovl_u8(cb8);
        let cr16 = vmovl_u8(cr8);

        // ======== Batch 0: pixels i..i+3 ========
        let y0_32 = vmovl_u16(vget_low_u16(y16));
        let cb0_32 = vmovl_u16(vget_low_u16(cb16));
        let cr0_32 = vmovl_u16(vget_low_u16(cr16));

        let y0 = vreinterpretq_s32_u32(y0_32);
        let cb0_c = vsubq_s32(vreinterpretq_s32_u32(cb0_32), cent);
        let cr0_c = vsubq_s32(vreinterpretq_s32_u32(cr0_32), cent);

        let rc0 = vshrq_n_s32(vmulq_s32(cr0_c, c359), 8);
        let gb0 = vshrq_n_s32(vmulq_s32(cb0_c, c88), 8);
        let gr0 = vshrq_n_s32(vmulq_s32(cr0_c, c183), 8);
        let bc0 = vshrq_n_s32(vmulq_s32(cb0_c, c454), 8);

        let r0 = vaddq_s32(y0, rc0);
        let g0 = vsubq_s32(vsubq_s32(y0, gb0), gr0);
        let b0 = vaddq_s32(y0, bc0);

        // Narrow and interleave batch 0
        let r16_0 = vqmovn_s32(r0);
        let g16_0 = vqmovn_s32(g0);
        let b16_0 = vqmovn_s32(b0);
        let a16 = vdup_n_s16(255);

        let br0 = vzip_s16(b16_0, r16_0);
        let ga0 = vzip_s16(g16_0, a16);
        let lo0 = vzip_s16(br0.0, ga0.0);
        let hi0 = vzip_s16(br0.1, ga0.1);

        let combined_lo0 = vcombine_s16(lo0.0, lo0.1);
        let combined_hi0 = vcombine_s16(hi0.0, hi0.1);

        let out_lo0 = vqmovun_s16(combined_lo0);
        let out_hi0 = vqmovun_s16(combined_hi0);

        // Store batch 0 (pixels i..i+3)
        let off = i * 4;
        vst1_u8(dst.as_mut_ptr().add(off), out_lo0);
        vst1_u8(dst.as_mut_ptr().add(off + 8), out_hi0);

        // ======== Batch 1: pixels i+4..i+7 ========
        let y1_32 = vmovl_u16(vget_high_u16(y16));
        let cb1_32 = vmovl_u16(vget_high_u16(cb16));
        let cr1_32 = vmovl_u16(vget_high_u16(cr16));

        let y1 = vreinterpretq_s32_u32(y1_32);
        let cb1_c = vsubq_s32(vreinterpretq_s32_u32(cb1_32), cent);
        let cr1_c = vsubq_s32(vreinterpretq_s32_u32(cr1_32), cent);

        let rc1 = vshrq_n_s32(vmulq_s32(cr1_c, c359), 8);
        let gb1 = vshrq_n_s32(vmulq_s32(cb1_c, c88), 8);
        let gr1 = vshrq_n_s32(vmulq_s32(cr1_c, c183), 8);
        let bc1 = vshrq_n_s32(vmulq_s32(cb1_c, c454), 8);

        let r1 = vaddq_s32(y1, rc1);
        let g1 = vsubq_s32(vsubq_s32(y1, gb1), gr1);
        let b1 = vaddq_s32(y1, bc1);

        let r16_1 = vqmovn_s32(r1);
        let g16_1 = vqmovn_s32(g1);
        let b16_1 = vqmovn_s32(b1);

        let br1 = vzip_s16(b16_1, r16_1);
        let ga1 = vzip_s16(g16_1, a16);
        let lo1 = vzip_s16(br1.0, ga1.0);
        let hi1 = vzip_s16(br1.1, ga1.1);

        let combined_lo1 = vcombine_s16(lo1.0, lo1.1);
        let combined_hi1 = vcombine_s16(hi1.0, hi1.1);

        let out_lo1 = vqmovun_s16(combined_lo1);
        let out_hi1 = vqmovun_s16(combined_hi1);

        // Store batch 1 (pixels i+4..i+7)
        vst1_u8(dst.as_mut_ptr().add(off + 16), out_lo1);
        vst1_u8(dst.as_mut_ptr().add(off + 24), out_hi1);

        i += 8;
    }

    // Scalar remainder (0-7 pixels)
    for j in i..width {
        let cb_byte = cb[j / 2];
        let cr_byte = cr[j / 2];
        let n_cb = if j & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
        let n_cr = if j & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
        let pixel = crate::yuv::yuv_to_bgra(y[j], n_cb << 4, n_cr << 4);
        let out = j * 4;
        dst[out..out + 4].copy_from_slice(&pixel);
    }
}
