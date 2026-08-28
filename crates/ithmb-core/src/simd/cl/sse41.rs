//! CL row -> BGRA via SSE4.1+SSSE3 (16 pixels per iteration).

/// SAFETY: must only be called on `x86`/`x86_64` where SSE4.1+SSSE3 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[target_feature(enable = "sse4.1,ssse3")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::too_many_lines)]
pub(crate) unsafe fn cl_row_to_bgra_sse41(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_and_si128, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_loadu_si128, _mm_max_epi32,
        _mm_min_epi32, _mm_mullo_epi32, _mm_packus_epi16, _mm_packus_epi32, _mm_set1_epi8, _mm_set1_epi16,
        _mm_set1_epi32, _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_epi16,
        _mm_storeu_si128, _mm_sub_epi32, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    // Process 16 pixels (4 quads) per iteration of the fast loop.
    let full_end_16 = (n_pixels / 16) * 16;

    // Nibble-to-byte*16 lookup table for SSSE3 pshufb
    let tbl = _mm_setr_epi8(
        0i8, 16i8, 32i8, 48i8, 64i8, 80i8, 96i8, 112i8, -128i8, -112i8, -96i8, -80i8, -64i8, -48i8, -32i8, -16i8,
    );
    let mask_lo = _mm_set1_epi8(0x0F);
    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi32(255);
    let cent = _mm_set1_epi32(128);
    let coef_359 = _mm_set1_epi32(359);
    let coef_88 = _mm_set1_epi32(88);
    let coef_183 = _mm_set1_epi32(183);
    let coef_454 = _mm_set1_epi32(454);
    let a16 = _mm_set1_epi16(255i16);

    // Shuffle masks to extract quad k (4 bytes) from a 16-byte vector into lower 32 bits.
    let mask_q0 = _mm_setr_epi8(0, 1, 2, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);
    let mask_q1 = _mm_setr_epi8(4, 5, 6, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);
    let mask_q2 = _mm_setr_epi8(8, 9, 10, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);
    let mask_q3 = _mm_setr_epi8(12, 13, 14, 15, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1);

    let mut i = 0usize;
    while i < full_end_16 {
        // Load 16 Y bytes and 16 chroma bytes at once.
        let y16 = _mm_loadu_si128(y.as_ptr().add(i).cast::<__m128i>());
        let c16 = _mm_loadu_si128(chroma.as_ptr().add(i).cast::<__m128i>());

        // Nibble expansion on all 16 chroma bytes at once
        let cb_idx = _mm_and_si128(c16, mask_lo);
        let cb_all = _mm_shuffle_epi8(tbl, cb_idx);
        let cr_idx = _mm_and_si128(_mm_srli_epi16(c16, 4), mask_lo);
        let cr_all = _mm_shuffle_epi8(tbl, cr_idx);

        // ---- Quad 0: pixels i..i+4 ----
        let y_q0 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(y16, mask_q0));
        let cb_q0 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cb_all, mask_q0));
        let cr_q0 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cr_all, mask_q0));

        let cb_c0 = _mm_sub_epi32(cb_q0, cent);
        let cr_c0 = _mm_sub_epi32(cr_q0, cent);

        let rc0 = _mm_srai_epi32(_mm_mullo_epi32(cr_c0, coef_359), 8);
        let gb0 = _mm_srai_epi32(_mm_mullo_epi32(cb_c0, coef_88), 8);
        let gr0 = _mm_srai_epi32(_mm_mullo_epi32(cr_c0, coef_183), 8);
        let bc0 = _mm_srai_epi32(_mm_mullo_epi32(cb_c0, coef_454), 8);

        let r0 = _mm_add_epi32(y_q0, rc0);
        let g0 = _mm_sub_epi32(_mm_sub_epi32(y_q0, gb0), gr0);
        let b0 = _mm_add_epi32(y_q0, bc0);

        let r_c0 = _mm_max_epi32(_mm_min_epi32(r0, max_val), zero);
        let g_c0 = _mm_max_epi32(_mm_min_epi32(g0, max_val), zero);
        let b_c0 = _mm_max_epi32(_mm_min_epi32(b0, max_val), zero);

        let b16_0 = _mm_packus_epi32(b_c0, zero);
        let g16_0 = _mm_packus_epi32(g_c0, zero);
        let r16_0 = _mm_packus_epi32(r_c0, zero);
        let br0 = _mm_unpacklo_epi16(b16_0, r16_0);
        let ga0 = _mm_unpacklo_epi16(g16_0, a16);
        let br_u8_0 = _mm_packus_epi16(br0, zero);
        let ga_u8_0 = _mm_packus_epi16(ga0, zero);
        let result0 = _mm_unpacklo_epi8(br_u8_0, ga_u8_0);

        let d_off = i * 4;
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off).cast::<__m128i>(), result0);

        // ---- Quad 1: pixels i+4..i+8 ----
        let y_q1 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(y16, mask_q1));
        let cb_q1 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cb_all, mask_q1));
        let cr_q1 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cr_all, mask_q1));

        let cb_c1 = _mm_sub_epi32(cb_q1, cent);
        let cr_c1 = _mm_sub_epi32(cr_q1, cent);

        let rc1 = _mm_srai_epi32(_mm_mullo_epi32(cr_c1, coef_359), 8);
        let gb1 = _mm_srai_epi32(_mm_mullo_epi32(cb_c1, coef_88), 8);
        let gr1 = _mm_srai_epi32(_mm_mullo_epi32(cr_c1, coef_183), 8);
        let bc1 = _mm_srai_epi32(_mm_mullo_epi32(cb_c1, coef_454), 8);

        let r1 = _mm_add_epi32(y_q1, rc1);
        let g1 = _mm_sub_epi32(_mm_sub_epi32(y_q1, gb1), gr1);
        let b1 = _mm_add_epi32(y_q1, bc1);

        let r_c1 = _mm_max_epi32(_mm_min_epi32(r1, max_val), zero);
        let g_c1 = _mm_max_epi32(_mm_min_epi32(g1, max_val), zero);
        let b_c1 = _mm_max_epi32(_mm_min_epi32(b1, max_val), zero);

        let b16_1 = _mm_packus_epi32(b_c1, zero);
        let g16_1 = _mm_packus_epi32(g_c1, zero);
        let r16_1 = _mm_packus_epi32(r_c1, zero);
        let br1 = _mm_unpacklo_epi16(b16_1, r16_1);
        let ga1 = _mm_unpacklo_epi16(g16_1, a16);
        let br_u8_1 = _mm_packus_epi16(br1, zero);
        let ga_u8_1 = _mm_packus_epi16(ga1, zero);
        let result1 = _mm_unpacklo_epi8(br_u8_1, ga_u8_1);

        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 16).cast::<__m128i>(), result1);

        // ---- Quad 2: pixels i+8..i+12 ----
        let y_q2 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(y16, mask_q2));
        let cb_q2 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cb_all, mask_q2));
        let cr_q2 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cr_all, mask_q2));

        let cb_c2 = _mm_sub_epi32(cb_q2, cent);
        let cr_c2 = _mm_sub_epi32(cr_q2, cent);

        let rc2 = _mm_srai_epi32(_mm_mullo_epi32(cr_c2, coef_359), 8);
        let gb2 = _mm_srai_epi32(_mm_mullo_epi32(cb_c2, coef_88), 8);
        let gr2 = _mm_srai_epi32(_mm_mullo_epi32(cr_c2, coef_183), 8);
        let bc2 = _mm_srai_epi32(_mm_mullo_epi32(cb_c2, coef_454), 8);

        let r2 = _mm_add_epi32(y_q2, rc2);
        let g2 = _mm_sub_epi32(_mm_sub_epi32(y_q2, gb2), gr2);
        let b2 = _mm_add_epi32(y_q2, bc2);

        let r_c2 = _mm_max_epi32(_mm_min_epi32(r2, max_val), zero);
        let g_c2 = _mm_max_epi32(_mm_min_epi32(g2, max_val), zero);
        let b_c2 = _mm_max_epi32(_mm_min_epi32(b2, max_val), zero);

        let b16_2 = _mm_packus_epi32(b_c2, zero);
        let g16_2 = _mm_packus_epi32(g_c2, zero);
        let r16_2 = _mm_packus_epi32(r_c2, zero);
        let br2 = _mm_unpacklo_epi16(b16_2, r16_2);
        let ga2 = _mm_unpacklo_epi16(g16_2, a16);
        let br_u8_2 = _mm_packus_epi16(br2, zero);
        let ga_u8_2 = _mm_packus_epi16(ga2, zero);
        let result2 = _mm_unpacklo_epi8(br_u8_2, ga_u8_2);

        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 32).cast::<__m128i>(), result2);

        // ---- Quad 3: pixels i+12..i+16 ----
        let y_q3 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(y16, mask_q3));
        let cb_q3 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cb_all, mask_q3));
        let cr_q3 = _mm_cvtepu8_epi32(_mm_shuffle_epi8(cr_all, mask_q3));

        let cb_c3 = _mm_sub_epi32(cb_q3, cent);
        let cr_c3 = _mm_sub_epi32(cr_q3, cent);

        let rc3 = _mm_srai_epi32(_mm_mullo_epi32(cr_c3, coef_359), 8);
        let gb3 = _mm_srai_epi32(_mm_mullo_epi32(cb_c3, coef_88), 8);
        let gr3 = _mm_srai_epi32(_mm_mullo_epi32(cr_c3, coef_183), 8);
        let bc3 = _mm_srai_epi32(_mm_mullo_epi32(cb_c3, coef_454), 8);

        let r3 = _mm_add_epi32(y_q3, rc3);
        let g3 = _mm_sub_epi32(_mm_sub_epi32(y_q3, gb3), gr3);
        let b3 = _mm_add_epi32(y_q3, bc3);

        let r_c3 = _mm_max_epi32(_mm_min_epi32(r3, max_val), zero);
        let g_c3 = _mm_max_epi32(_mm_min_epi32(g3, max_val), zero);
        let b_c3 = _mm_max_epi32(_mm_min_epi32(b3, max_val), zero);

        let b16_3 = _mm_packus_epi32(b_c3, zero);
        let g16_3 = _mm_packus_epi32(g_c3, zero);
        let r16_3 = _mm_packus_epi32(r_c3, zero);
        let br3 = _mm_unpacklo_epi16(b16_3, r16_3);
        let ga3 = _mm_unpacklo_epi16(g16_3, a16);
        let br_u8_3 = _mm_packus_epi16(br3, zero);
        let ga_u8_3 = _mm_packus_epi16(ga3, zero);
        let result3 = _mm_unpacklo_epi8(br_u8_3, ga_u8_3);

        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 48).cast::<__m128i>(), result3);

        i += 16;
    }

    // Remainder: process 4 pixels at a time (handles 0-15 remaining pixels).
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

        let cb_c = _mm_sub_epi32(cb, cent);
        let cr_c = _mm_sub_epi32(cr, cent);

        let rc = _mm_srai_epi32(_mm_mullo_epi32(cr_c, coef_359), 8);
        let gb = _mm_srai_epi32(_mm_mullo_epi32(cb_c, coef_88), 8);
        let gr = _mm_srai_epi32(_mm_mullo_epi32(cr_c, coef_183), 8);
        let bc = _mm_srai_epi32(_mm_mullo_epi32(cb_c, coef_454), 8);

        let r = _mm_add_epi32(y_vals, rc);
        let g = _mm_sub_epi32(_mm_sub_epi32(y_vals, gb), gr);
        let b = _mm_add_epi32(y_vals, bc);

        let r_c = _mm_max_epi32(_mm_min_epi32(r, max_val), zero);
        let g_c = _mm_max_epi32(_mm_min_epi32(g, max_val), zero);
        let b_c = _mm_max_epi32(_mm_min_epi32(b, max_val), zero);

        let b16 = _mm_packus_epi32(b_c, zero);
        let g16 = _mm_packus_epi32(g_c, zero);
        let r16 = _mm_packus_epi32(r_c, zero);

        let br = _mm_unpacklo_epi16(b16, r16);
        let ga = _mm_unpacklo_epi16(g16, a16);

        let br_u8 = _mm_packus_epi16(br, zero);
        let ga_u8 = _mm_packus_epi16(ga, zero);

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
