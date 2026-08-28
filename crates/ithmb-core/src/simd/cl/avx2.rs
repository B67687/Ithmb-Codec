//! CL row -> BGRA via AVX2 (8 pixels per iteration, 256-bit arithmetic throughout).

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::too_many_lines)]
pub(crate) unsafe fn cl_row_to_bgra_avx2(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_add_epi32, _mm_and_si128, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_loadl_epi64,
        _mm_max_epi32, _mm_min_epi32, _mm_mullo_epi32, _mm_packus_epi16, _mm_packus_epi32, _mm_set1_epi8,
        _mm_set1_epi16, _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_epi16,
        _mm_srli_si128, _mm_storeu_si128, _mm_sub_epi32, _mm_unpacklo_epi8, _mm_unpacklo_epi16, _mm256_add_epi32,
        _mm256_castsi256_si128, _mm256_cvtepu8_epi32, _mm256_extracti128_si256, _mm256_max_epi32, _mm256_min_epi32,
        _mm256_mullo_epi32, _mm256_packus_epi16, _mm256_packus_epi32, _mm256_set_m128i, _mm256_set1_epi16,
        _mm256_set1_epi32, _mm256_setzero_si256, _mm256_srai_epi32, _mm256_storeu_si256, _mm256_sub_epi32,
        _mm256_unpacklo_epi16,
    };

    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    let full_end_8 = (n_pixels / 8) * 8;

    // nibble→byte*16 lookup table (128-bit — cvtepu8 reads bytes 0-7)
    let tbl = _mm_setr_epi8(
        0i8, 16i8, 32i8, 48i8, 64i8, 80i8, 96i8, 112i8, -128i8, -112i8, -96i8, -80i8, -64i8, -48i8, -32i8, -16i8,
    );
    let mask_lo = _mm_set1_epi8(0x0F);
    let zero_128 = _mm_setzero_si128();
    let zero = _mm256_setzero_si256();
    let max_val = _mm256_set1_epi32(255);
    let cent = _mm256_set1_epi32(128);
    let coef_359 = _mm256_set1_epi32(359);
    let coef_88 = _mm256_set1_epi32(88);
    let coef_183 = _mm256_set1_epi32(183);
    let coef_454 = _mm256_set1_epi32(454);
    let a16 = _mm256_set1_epi16(255i16);

    let mut i = 0usize;
    while i < full_end_8 {
        // Load 8 Y bytes → _mm256_cvtepu8_epi32 reads bytes 0-7 → i32x8
        let y_8 = _mm_loadl_epi64(y.as_ptr().add(i).cast::<__m128i>());
        let y_vals = _mm256_cvtepu8_epi32(y_8);

        // Load 8 chroma bytes, expand Cb and Cr via pshufb (128-bit)
        let c_8 = _mm_loadl_epi64(chroma.as_ptr().add(i).cast::<__m128i>());

        // Cb: low nibble → pshufb (*16) → cvtepu8_epi32 → i32x8
        let cb_idx = _mm_and_si128(c_8, mask_lo);
        let cb_bytes = _mm_shuffle_epi8(tbl, cb_idx);
        let cb = _mm256_cvtepu8_epi32(cb_bytes);

        // Cr: srli_epi16(4) → mask → pshufb → cvtepu8_epi32 → i32x8
        let cr_idx = _mm_and_si128(_mm_srli_epi16(c_8, 4), mask_lo);
        let cr_bytes = _mm_shuffle_epi8(tbl, cr_idx);
        let cr = _mm256_cvtepu8_epi32(cr_bytes);

        // ---- AVX2 BT.601 arithmetic on 8 pixels at once ----
        let cb_c = _mm256_sub_epi32(cb, cent);
        let cr_c = _mm256_sub_epi32(cr, cent);

        let rc = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c, coef_359), 8);
        let gb = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c, coef_88), 8);
        let gr = _mm256_srai_epi32(_mm256_mullo_epi32(cr_c, coef_183), 8);
        let bc = _mm256_srai_epi32(_mm256_mullo_epi32(cb_c, coef_454), 8);

        let r = _mm256_add_epi32(y_vals, rc);
        let g = _mm256_sub_epi32(_mm256_sub_epi32(y_vals, gb), gr);
        let b = _mm256_add_epi32(y_vals, bc);

        let r_c = _mm256_max_epi32(_mm256_min_epi32(r, max_val), zero);
        let g_c = _mm256_max_epi32(_mm256_min_epi32(g, max_val), zero);
        let b_c = _mm256_max_epi32(_mm256_min_epi32(b, max_val), zero);

        // ---- Pack i32→u16→u8 with BGRA interleave (per lane: 4 pixels each) ----
        let b16 = _mm256_packus_epi32(b_c, zero);
        let g16 = _mm256_packus_epi32(g_c, zero);
        let r16 = _mm256_packus_epi32(r_c, zero);

        let br = _mm256_unpacklo_epi16(b16, r16);
        let ga = _mm256_unpacklo_epi16(g16, a16);

        let packed = _mm256_packus_epi16(br, ga);

        // Interleave br and ga halves: extract each 128-bit lane, shift right 8, unpack
        let lo_128 = _mm256_castsi256_si128(packed);
        let lo_shift = _mm_srli_si128(lo_128, 8);
        let lane0 = _mm_unpacklo_epi8(lo_128, lo_shift);

        let hi_128 = _mm256_extracti128_si256(packed, 1);
        let hi_shift = _mm_srli_si128(hi_128, 8);
        let lane1 = _mm_unpacklo_epi8(hi_128, hi_shift);

        // Combine lanes and store 32 bytes (8 BGRA pixels)
        let result = _mm256_set_m128i(lane1, lane0);
        let d_off = i * 4;
        _mm256_storeu_si256(dst.as_mut_ptr().add(d_off).cast::<__m256i>(), result);

        i += 8;
    }

    // Remainder: 0-7 pixels via SSE4.1-style (4-pixel blocks, then 0-3 scalar)
    while i + 4 <= n_pixels {
        let y_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([y[i], y[i + 1], y[i + 2], y[i + 3]]));
        let y_vals = _mm_cvtepu8_epi32(y_chunk);

        let c_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ]));

        let cb_idx = _mm_and_si128(c_chunk, mask_lo);
        let cb_bytes = _mm_shuffle_epi8(tbl, cb_idx);
        let cb = _mm_cvtepu8_epi32(cb_bytes);

        let cr_idx = _mm_and_si128(_mm_srli_epi16(c_chunk, 4), mask_lo);
        let cr_bytes = _mm_shuffle_epi8(tbl, cr_idx);
        let cr = _mm_cvtepu8_epi32(cr_bytes);

        let cb_c = _mm_sub_epi32(cb, _mm256_castsi256_si128(cent));
        let cr_c = _mm_sub_epi32(cr, _mm256_castsi256_si128(cent));

        let rc = _mm_srai_epi32(_mm_mullo_epi32(cr_c, _mm256_castsi256_si128(coef_359)), 8);
        let gb = _mm_srai_epi32(_mm_mullo_epi32(cb_c, _mm256_castsi256_si128(coef_88)), 8);
        let gr = _mm_srai_epi32(_mm_mullo_epi32(cr_c, _mm256_castsi256_si128(coef_183)), 8);
        let bc = _mm_srai_epi32(_mm_mullo_epi32(cb_c, _mm256_castsi256_si128(coef_454)), 8);

        let r = _mm_add_epi32(y_vals, rc);
        let g = _mm_sub_epi32(_mm_sub_epi32(y_vals, gb), gr);
        let b = _mm_add_epi32(y_vals, bc);

        let r_c = _mm_max_epi32(
            _mm_min_epi32(r, _mm256_castsi256_si128(max_val)),
            _mm256_castsi256_si128(zero),
        );
        let g_c = _mm_max_epi32(
            _mm_min_epi32(g, _mm256_castsi256_si128(max_val)),
            _mm256_castsi256_si128(zero),
        );
        let b_c = _mm_max_epi32(
            _mm_min_epi32(b, _mm256_castsi256_si128(max_val)),
            _mm256_castsi256_si128(zero),
        );

        let b16 = _mm_packus_epi32(b_c, zero_128);
        let g16 = _mm_packus_epi32(g_c, zero_128);
        let r16 = _mm_packus_epi32(r_c, zero_128);
        let a16_128 = _mm_set1_epi16(255i16);

        let br = _mm_unpacklo_epi16(b16, r16);
        let ga = _mm_unpacklo_epi16(g16, a16_128);

        let br_u8 = _mm_packus_epi16(br, zero_128);
        let ga_u8 = _mm_packus_epi16(ga, zero_128);
        let result = _mm_unpacklo_epi8(br_u8, ga_u8);

        let d_off = i * 4;
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off).cast::<__m128i>(), result);
        i += 4;
    }

    // Remaining 0-3 pixels via scalar.
    for j in i..n_pixels {
        let cr = chroma[j] & 0xF0;
        let cb = (chroma[j] & 0x0F) << 4;
        let px = crate::yuv::yuv_to_bgra(y[j], cb, cr);
        let o = j * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
}
