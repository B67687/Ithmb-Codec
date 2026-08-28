//! CLCL row -> BGRA via AVX2 (16 pixels per iteration).

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
            __m128i, _mm_and_si128, _mm_loadl_epi64, _mm_loadu_si128, _mm_set1_epi8, _mm_slli_epi16, _mm_srli_epi16,
            _mm_srli_si128, _mm_storeu_si128, _mm_unpacklo_epi8, _mm256_add_epi32, _mm256_castsi256_si128,
            _mm256_cvtepu8_epi32, _mm256_extracti128_si256, _mm256_max_epi32, _mm256_min_epi32, _mm256_mullo_epi32,
            _mm256_packus_epi16, _mm256_packus_epi32, _mm256_set1_epi16, _mm256_set1_epi32, _mm256_setzero_si256,
            _mm256_srai_epi32, _mm256_sub_epi32, _mm256_unpacklo_epi16,
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
            super::clcl_row_to_bgra_sse2(
                y_ptr.add(i),
                cb_ptr.add(i / 2),
                cr_ptr.add(i / 2),
                (w - i) as u32,
                dst.add(i * 4),
            );
        }
    }
}
