//! CLCL (separate Cb/Cr nibble-chroma planes) -> BGRA - SIMD-accelerated (SSE2/AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

/// Process one row of CLCL data via SSE2.
///
/// Reads `width` Y bytes, `width/2` Cb bytes (nibble-packed, 2 pixels/byte),
/// `width/2` Cr bytes (same) and writes `width*4` BGRA bytes.
///
/// # Safety
///
/// - `y_ptr` must point to `width` valid bytes.
/// - `cb_ptr` must point to `width / 2` valid bytes.
/// - `cr_ptr` must point to `width / 2` valid bytes.
/// - Requires `x86_64` target.
#[cfg(target_arch = "x86_64")]
#[allow(clippy::many_single_char_names)]
pub unsafe fn clcl_row_to_bgra_sse2(y_ptr: *const u8, cb_ptr: *const u8, cr_ptr: *const u8, width: u32, dst: *mut u8) {
    unsafe {
        use core::arch::x86_64::{
            __m128i, _mm_and_si128, _mm_cvtsi32_si128, _mm_loadl_epi64, _mm_set1_epi8, _mm_slli_epi16, _mm_srli_epi16,
            _mm_storeu_si128, _mm_unpacklo_epi8,
        };

        let w = width as usize;
        let full_batches = (w / 8) * 8;
        let mut i: usize = 0;
        let low_mask = _mm_set1_epi8(0x0F);

        while i < full_batches {
            // ---- Load 8 Y bytes ----
            let y_val = _mm_loadl_epi64(y_ptr.add(i).cast::<__m128i>());

            // ---- Load 4 Cb/Cr bytes (covering 8 pixels: 2 pixels per byte) ----
            let cb_word = core::ptr::read_unaligned(cb_ptr.add(i / 2).cast::<i32>());
            let cr_word = core::ptr::read_unaligned(cr_ptr.add(i / 2).cast::<i32>());
            let cb_raw = _mm_cvtsi32_si128(cb_word);
            let cr_raw = _mm_cvtsi32_si128(cr_word);

            // ---- Extract low/high nibbles ----
            // cb_raw = [Cb0, Cb1, Cb2, Cb3, 0...]
            // cb_lo  = [Cb0&0x0F, Cb1&0x0F, Cb2&0x0F, Cb3&0x0F, 0...]
            // cb_hi  = [Cb0>>4,   Cb1>>4,   Cb2>>4,   Cb3>>4,   0...]
            let cb_lo = _mm_and_si128(cb_raw, low_mask);
            let cb_hi = _mm_and_si128(_mm_srli_epi16(cb_raw, 4), low_mask);
            let cr_lo = _mm_and_si128(cr_raw, low_mask);
            let cr_hi = _mm_and_si128(_mm_srli_epi16(cr_raw, 4), low_mask);

            // ---- Interleave: [lo0, hi0, lo1, hi1, lo2, hi2, lo3, hi3] ----
            let cb_unpacked = _mm_unpacklo_epi8(cb_lo, cb_hi);
            let cr_unpacked = _mm_unpacklo_epi8(cr_lo, cr_hi);

            // ---- Expand nibbles to 8-bit: nibble << 4 ----
            // _mm_slli_epi16 shifts each 16-bit lane left by 4.
            // Each nibble value is 0-15, so 4-bit shift never overflows into
            // the next byte (max 15*16 = 240 < 256).
            let cb_exp = _mm_slli_epi16(cb_unpacked, 4);
            let cr_exp = _mm_slli_epi16(cr_unpacked, 4);

            // ---- Store to stack arrays for per-pixel YUV→BGRA ----
            let mut y_arr: [u8; 8] = [0u8; 8];
            let mut cb_arr: [u8; 8] = [0u8; 8];
            let mut cr_arr: [u8; 8] = [0u8; 8];
            _mm_storeu_si128(y_arr.as_mut_ptr().cast::<__m128i>(), y_val);
            _mm_storeu_si128(cb_arr.as_mut_ptr().cast::<__m128i>(), cb_exp);
            _mm_storeu_si128(cr_arr.as_mut_ptr().cast::<__m128i>(), cr_exp);

            // ---- BT.601 YUV→BGRA (8 pixels, each with own Cb/Cr) ----
            for px in 0..8usize {
                let y = i32::from(y_arr[px]);
                let cb = i32::from(cb_arr[px]).wrapping_sub(128);
                let cr = i32::from(cr_arr[px]).wrapping_sub(128);
                let r = (y + ((cr * 359) >> 8)).clamp(0, 255) as u8;
                let g = (y - ((cb * 88) >> 8) - ((cr * 183) >> 8)).clamp(0, 255) as u8;
                let b = (y + ((cb * 454) >> 8)).clamp(0, 255) as u8;
                let out = dst.add(i * 4 + px * 4);
                *out = b;
                *out.add(1) = g;
                *out.add(2) = r;
                *out.add(3) = 255;
            }

            i += 8;
        }

        // ---- Scalar remainder (< 8 pixels) ----
        while i < w {
            let y = i32::from(*y_ptr.add(i));
            let cb_byte = *cb_ptr.add(i / 2);
            let cr_byte = *cr_ptr.add(i / 2);
            let n_cb = if i & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
            let n_cr = if i & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
            let cb8 = i32::from(n_cb) << 4;
            let cr8 = i32::from(n_cr) << 4;
            let cb = cb8.wrapping_sub(128);
            let cr = cr8.wrapping_sub(128);
            let r = (y + ((cr * 359) >> 8)).clamp(0, 255) as u8;
            let g = (y - ((cb * 88) >> 8) - ((cr * 183) >> 8)).clamp(0, 255) as u8;
            let b = (y + ((cb * 454) >> 8)).clamp(0, 255) as u8;
            let out = dst.add(i * 4);
            *out = b;
            *out.add(1) = g;
            *out.add(2) = r;
            *out.add(3) = 255;
            i += 1;
        }
    }
}

/// Process one row of CLCL data via AVX2.
///
/// Reads `width` Y bytes, `width/2` Cb bytes (nibble-packed, 2 pixels/byte),
/// `width/2` Cr bytes (same) and writes `width*4` BGRA bytes.
/// Processes 16 pixels per iteration using 256-bit arithmetic.
///
/// # Safety
///
/// - `y_ptr` must point to `width` valid bytes.
/// - `cb_ptr` must point to `width / 2` valid bytes.
/// - `cr_ptr` must point to `width / 2` valid bytes.
/// - `dst` must point to `width * 4` valid bytes.
/// - Requires `x86_64` target and AVX2 at runtime.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
pub unsafe fn clcl_row_to_bgra_avx2(y_ptr: *const u8, cb_ptr: *const u8, cr_ptr: *const u8, width: u32, dst: *mut u8) {
    unsafe {
        use core::arch::x86_64::{
            __m128i, _mm256_add_epi32, _mm256_castsi256_si128, _mm256_cvtepu8_epi32, _mm256_extracti128_si256,
            _mm256_max_epi32, _mm256_min_epi32, _mm256_mullo_epi32, _mm256_packus_epi16, _mm256_packus_epi32,
            _mm256_set1_epi16, _mm256_set1_epi32, _mm256_setzero_si256, _mm256_srai_epi32, _mm256_sub_epi32,
            _mm256_unpacklo_epi16, _mm_and_si128, _mm_loadl_epi64, _mm_loadu_si128, _mm_set1_epi8, _mm_slli_epi16,
            _mm_srli_epi16, _mm_srli_si128, _mm_storeu_si128, _mm_unpacklo_epi8,
        };

        let w = width as usize;
        let full_16 = (w / 16) * 16;
        let mut i: usize = 0;
        let low_mask = _mm_set1_epi8(0x0F);

        // 256-bit BT.601 constants
        let zero = _mm256_setzero_si256();
        let max_val = _mm256_set1_epi32(255);
        let cent = _mm256_set1_epi32(128);
        let coef_359 = _mm256_set1_epi32(359);
        let coef_88 = _mm256_set1_epi32(88);
        let coef_183 = _mm256_set1_epi32(183);
        let coef_454 = _mm256_set1_epi32(454);
        let a16 = _mm256_set1_epi16(255i16);

        while i < full_16 {
            // ---- Load 16 Y bytes ----
            let y_all = _mm_loadu_si128(y_ptr.add(i).cast::<__m128i>());

            // ---- Load 8 Cb/Cr bytes (nibble-packed, 2 pixels per byte) ----
            let cb_all = _mm_loadl_epi64(cb_ptr.add(i / 2).cast::<__m128i>());
            let cr_all = _mm_loadl_epi64(cr_ptr.add(i / 2).cast::<__m128i>());

            // ---- Extract low/high nibbles ----
            let cb_lo = _mm_and_si128(cb_all, low_mask);
            let cb_hi = _mm_and_si128(_mm_srli_epi16(cb_all, 4), low_mask);
            let cr_lo = _mm_and_si128(cr_all, low_mask);
            let cr_hi = _mm_and_si128(_mm_srli_epi16(cr_all, 4), low_mask);

            // ---- Interleave: [lo0, hi0, lo1, hi1, ..., lo7, hi7] (16 bytes) ----
            let cb_unpacked = _mm_unpacklo_epi8(cb_lo, cb_hi);
            let cr_unpacked = _mm_unpacklo_epi8(cr_lo, cr_hi);

            // ---- Expand nibbles to 8-bit: nibble << 4 ----
            let cb_exp = _mm_slli_epi16(cb_unpacked, 4);
            let cr_exp = _mm_slli_epi16(cr_unpacked, 4);

            // ============ Lower 8 pixels (bytes 0-7 of each vector) ============
            let y_lo = _mm256_cvtepu8_epi32(y_all);
            let cb_lo_v = _mm256_cvtepu8_epi32(cb_exp);
            let cr_lo_v = _mm256_cvtepu8_epi32(cr_exp);

            let cb_c_lo = _mm256_sub_epi32(cb_lo_v, cent);
            let cr_c_lo = _mm256_sub_epi32(cr_lo_v, cent);
            let rc_lo = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c_lo, coef_359), 8);
            let gb_lo = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c_lo, coef_88), 8);
            let gr_lo = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c_lo, coef_183), 8);
            let bc_lo = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c_lo, coef_454), 8);

            let r_lo = _mm256_add_epi32(y_lo, rc_lo);
            let g_lo = _mm256_sub_epi32(_mm256_sub_epi32(y_lo, gb_lo), gr_lo);
            let b_lo = _mm256_add_epi32(y_lo, bc_lo);

            let r_c_lo = _mm256_max_epi32(_mm256_min_epi32(r_lo, max_val), zero);
            let g_c_lo = _mm256_max_epi32(_mm256_min_epi32(g_lo, max_val), zero);
            let b_c_lo = _mm256_max_epi32(_mm256_min_epi32(b_lo, max_val), zero);

            // Pack i32->u16->u8 with BGRA interleave for lower 8 pixels
            let b16_lo = _mm256_packus_epi32(b_c_lo, zero);
            let g16_lo = _mm256_packus_epi32(g_c_lo, zero);
            let r16_lo = _mm256_packus_epi32(r_c_lo, zero);
            let br_lo = _mm256_unpacklo_epi16(b16_lo, r16_lo);
            let ga_lo = _mm256_unpacklo_epi16(g16_lo, a16);
            let packed_lo = _mm256_packus_epi16(br_lo, ga_lo);

            // Lower lane of packed_lo = pixels 0-3 interleaved as B,R,G,A
            let lo0 = _mm256_castsi256_si128(packed_lo);
            let lo0_shift = _mm_srli_si128(lo0, 8);
            let bgra0 = _mm_unpacklo_epi8(lo0, lo0_shift);

            // Upper lane of packed_lo = pixels 4-7 interleaved as B,R,G,A
            let hi0 = _mm256_extracti128_si256(packed_lo, 1);
            let hi0_shift = _mm_srli_si128(hi0, 8);
            let bgra1 = _mm_unpacklo_epi8(hi0, hi0_shift);

            // ============ Upper 8 pixels (bytes 8-15 of each vector) ============
            let y_hi8 = _mm_srli_si128(y_all, 8);
            let y_hi = _mm256_cvtepu8_epi32(y_hi8);
            let cb_hi8 = _mm_srli_si128(cb_exp, 8);
            let cb_hi_v = _mm256_cvtepu8_epi32(cb_hi8);
            let cr_hi8 = _mm_srli_si128(cr_exp, 8);
            let cr_hi_v = _mm256_cvtepu8_epi32(cr_hi8);

            let cb_c_hi = _mm256_sub_epi32(cb_hi_v, cent);
            let cr_c_hi = _mm256_sub_epi32(cr_hi_v, cent);
            let rc_hi = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c_hi, coef_359), 8);
            let gb_hi = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c_hi, coef_88), 8);
            let gr_hi = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c_hi, coef_183), 8);
            let bc_hi = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c_hi, coef_454), 8);

            let r_hi = _mm256_add_epi32(y_hi, rc_hi);
            let g_hi = _mm256_sub_epi32(_mm256_sub_epi32(y_hi, gb_hi), gr_hi);
            let b_hi = _mm256_add_epi32(y_hi, bc_hi);

            let r_c_hi = _mm256_max_epi32(_mm256_min_epi32(r_hi, max_val), zero);
            let g_c_hi = _mm256_max_epi32(_mm256_min_epi32(g_hi, max_val), zero);
            let b_c_hi = _mm256_max_epi32(_mm256_min_epi32(b_hi, max_val), zero);

            let b16_hi = _mm256_packus_epi32(b_c_hi, zero);
            let g16_hi = _mm256_packus_epi32(g_c_hi, zero);
            let r16_hi = _mm256_packus_epi32(r_c_hi, zero);
            let br_hi = _mm256_unpacklo_epi16(b16_hi, r16_hi);
            let ga_hi = _mm256_unpacklo_epi16(g16_hi, a16);
            let packed_hi = _mm256_packus_epi16(br_hi, ga_hi);

            // Lower lane of packed_hi = pixels 8-11
            let lo2 = _mm256_castsi256_si128(packed_hi);
            let lo2_shift = _mm_srli_si128(lo2, 8);
            let bgra2 = _mm_unpacklo_epi8(lo2, lo2_shift);

            // Upper lane of packed_hi = pixels 12-15
            let hi2 = _mm256_extracti128_si256(packed_hi, 1);
            let hi2_shift = _mm_srli_si128(hi2, 8);
            let bgra3 = _mm_unpacklo_epi8(hi2, hi2_shift);

            // Store 4 groups of 4 BGRA pixels = 16 pixels = 64 bytes
            let off = i * 4;
            _mm_storeu_si128(dst.add(off).cast::<__m128i>(), bgra0);
            _mm_storeu_si128(dst.add(off + 16).cast::<__m128i>(), bgra1);
            _mm_storeu_si128(dst.add(off + 32).cast::<__m128i>(), bgra2);
            _mm_storeu_si128(dst.add(off + 48).cast::<__m128i>(), bgra3);

            i += 16;
        }

        // ---- SSE2 remainder (handles remaining 0-15 pixels) ----
        if i < w {
            clcl_row_to_bgra_sse2(
                y_ptr.add(i),
                cb_ptr.add(i / 2),
                cr_ptr.add(i / 2),
                (w - i) as u32,
                dst.add(i * 4),
            );
        }
    }
}

/// Decode one row of CLCL data (separate Y/Cb/Cr planes) to BGRA.
///
/// On `x86_64` uses AVX2 (16 px/iter, runtime-detected), SSE2 (8 px/iter), or scalar fallback.
/// On other platforms uses the scalar fallback.
#[inline]
pub fn clcl_row_to_bgra(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: checked by is_x86_feature_detected! below.
        if is_x86_feature_detected!("avx2") {
            unsafe {
                clcl_row_to_bgra_avx2(y.as_ptr(), cb.as_ptr(), cr.as_ptr(), width as u32, dst.as_mut_ptr());
            }
            return;
        }
        // SSE2 is guaranteed on x86_64 (baseline).
        // SAFETY: The raw pointers are derived from valid slices with correct lengths.
        unsafe {
            clcl_row_to_bgra_sse2(y.as_ptr(), cb.as_ptr(), cr.as_ptr(), width as u32, dst.as_mut_ptr());
        }
        return;
    }

    #[cfg(all(target_arch = "aarch64", not(target_os = "macos")))]
    {
        // SAFETY: aarch64 guarantees NEON.
        unsafe {
            super::neon::clcl_row_to_bgra_neon(y, cb, cr, width, dst);
        }
        return;
    }
    // Fallback (non-x86_64) — scalar
    #[allow(unreachable_code)]
    scalar_fallback(y, cb, cr, width, dst);
}

/// Scalar fallback for CLCL row decode.
#[allow(dead_code)]
fn scalar_fallback(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    for i in 0..width {
        let cb_byte = cb[i / 2];
        let cr_byte = cr[i / 2];
        let n_cb = if i & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
        let n_cr = if i & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
        let pixel = crate::yuv::yuv_to_bgra(y[i], n_cb << 4, n_cr << 4);
        let out = i * 4;
        dst[out..out + 4].copy_from_slice(&pixel);
    }
}
