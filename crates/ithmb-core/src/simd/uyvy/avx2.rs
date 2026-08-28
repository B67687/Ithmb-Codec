//! UYVY row -> BGRA via AVX2 (16 pixels per iteration).

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(
    unsafe_op_in_unsafe_fn,
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::too_many_lines
)]
pub(crate) unsafe fn uyvy_row_to_bgra_avx2(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_set1_epi8, _mm_storeu_si128, _mm_unpackhi_epi8, _mm_unpacklo_epi8, _mm256_add_epi16,
        _mm256_extracti128_si256, _mm256_loadu_si256, _mm256_madd_epi16, _mm256_max_epi16, _mm256_min_epi16,
        _mm256_packs_epi32, _mm256_packus_epi16, _mm256_set_epi8, _mm256_set1_epi16, _mm256_setzero_si256,
        _mm256_shuffle_epi8, _mm256_srai_epi32, _mm256_sub_epi16, _mm256_unpackhi_epi32, _mm256_unpacklo_epi32,
    };
    let n = src.len();
    let full_end = (n / 32) * 32;
    let mut i = 0usize;

    // pshufb masks for 256-bit — same pattern per 128-bit lane.
    let shuf_y = _mm256_set_epi8(
        -128i8, 15, -128, 13, -128, 11, -128, 9, -128, 7, -128, 5, -128, 3, -128, 1, -128i8, 15, -128, 13, -128, 11,
        -128, 9, -128, 7, -128, 5, -128, 3, -128, 1,
    );
    let shuf_u = _mm256_set_epi8(
        -128i8, 12, -128, 12, -128, 8, -128, 8, -128, 4, -128, 4, -128, 0, -128, 0, -128i8, 12, -128, 12, -128, 8,
        -128, 8, -128, 4, -128, 4, -128, 0, -128, 0,
    );
    let shuf_v = _mm256_set_epi8(
        -128i8, 14, -128, 14, -128, 10, -128, 10, -128, 6, -128, 6, -128, 2, -128, 2, -128i8, 14, -128, 14, -128, 10,
        -128, 10, -128, 6, -128, 6, -128, 2, -128, 2,
    );

    let zero256 = _mm256_setzero_si256();
    let max255 = _mm256_set1_epi16(255);
    let offset128 = _mm256_set1_epi16(128);
    let alpha = _mm_set1_epi8(-1i8);

    // BT.601 coefficients (i16, fits all products via pmaddwd i32 accumulation)
    let coeff_rc = _mm256_set1_epi16(359);
    let coeff_gb = _mm256_set1_epi16(88);
    let coeff_gr = _mm256_set1_epi16(183);
    let coeff_bc = _mm256_set1_epi16(454);

    while i < full_end {
        // Load 32 bytes = 16 UYVY pixels
        let v = _mm256_loadu_si256(src.as_ptr().add(i).cast::<__m256i>());

        // SSSE3/AVX2 vpshufb: deinterleave into u16 lanes (byte, 0 per lane)
        let y_vals = _mm256_shuffle_epi8(v, shuf_y);
        let u_vals = _mm256_shuffle_epi8(v, shuf_u);
        let v_vals = _mm256_shuffle_epi8(v, shuf_v);

        // Signed chroma offsets: Cb = U - 128, Cr = V - 128
        let cb = _mm256_sub_epi16(u_vals, offset128);
        let cr = _mm256_sub_epi16(v_vals, offset128);

        let rc_i32 = _mm256_srai_epi32(_mm256_madd_epi16(cr, coeff_rc), 9);
        let gb_i32 = _mm256_srai_epi32(_mm256_madd_epi16(cb, coeff_gb), 9);
        let gr_i32 = _mm256_srai_epi32(_mm256_madd_epi16(cr, coeff_gr), 9);
        let bc_i32 = _mm256_srai_epi32(_mm256_madd_epi16(cb, coeff_bc), 9);

        // Unpack 4 i32 results -> 8 i16, duplicating chroma for each pixel pair
        let rc = _mm256_packs_epi32(
            _mm256_unpacklo_epi32(rc_i32, rc_i32),
            _mm256_unpackhi_epi32(rc_i32, rc_i32),
        );
        let gb = _mm256_packs_epi32(
            _mm256_unpacklo_epi32(gb_i32, gb_i32),
            _mm256_unpackhi_epi32(gb_i32, gb_i32),
        );
        let gr = _mm256_packs_epi32(
            _mm256_unpacklo_epi32(gr_i32, gr_i32),
            _mm256_unpackhi_epi32(gr_i32, gr_i32),
        );
        let bc = _mm256_packs_epi32(
            _mm256_unpacklo_epi32(bc_i32, bc_i32),
            _mm256_unpackhi_epi32(bc_i32, bc_i32),
        );

        // YUV -> RGB (all i16, ranges stay well within i16)
        let r = _mm256_add_epi16(y_vals, rc);
        let g = _mm256_sub_epi16(_mm256_sub_epi16(y_vals, gb), gr);
        let b = _mm256_add_epi16(y_vals, bc);

        // Clamp to [0, 255]
        let r_c = _mm256_max_epi16(_mm256_min_epi16(r, max255), zero256);
        let g_c = _mm256_max_epi16(_mm256_min_epi16(g, max255), zero256);
        let b_c = _mm256_max_epi16(_mm256_min_epi16(b, max255), zero256);

        // Pack i16 -> u8 (unsigned saturate, already clamped)
        let b_u8 = _mm256_packus_epi16(b_c, zero256);
        let g_u8 = _mm256_packus_epi16(g_c, zero256);
        let r_u8 = _mm256_packus_epi16(r_c, zero256);

        // Extract 128-bit lanes for BGRA interleave
        let b_lo = _mm256_extracti128_si256(b_u8, 0);
        let b_hi = _mm256_extracti128_si256(b_u8, 1);
        let g_lo = _mm256_extracti128_si256(g_u8, 0);
        let g_hi = _mm256_extracti128_si256(g_u8, 1);
        let r_lo = _mm256_extracti128_si256(r_u8, 0);
        let r_hi = _mm256_extracti128_si256(r_u8, 1);

        // Lower 8 pixels -> BGRA via two-level unpack
        let br_lo = _mm_unpacklo_epi8(b_lo, r_lo);
        let ga_lo = _mm_unpacklo_epi8(g_lo, alpha);
        let px0 = _mm_unpacklo_epi8(br_lo, ga_lo);
        let px1 = _mm_unpackhi_epi8(br_lo, ga_lo);

        // Upper 8 pixels -> BGRA
        let br_hi = _mm_unpacklo_epi8(b_hi, r_hi);
        let ga_hi = _mm_unpacklo_epi8(g_hi, alpha);
        let px2 = _mm_unpacklo_epi8(br_hi, ga_hi);
        let px3 = _mm_unpackhi_epi8(br_hi, ga_hi);

        // Store 64 bytes (16 BGRA pixels) — direct, no copy_from_slice
        let d_off = i * 2;
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off).cast::<__m128i>(), px0);
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 16).cast::<__m128i>(), px1);
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 32).cast::<__m128i>(), px2);
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 48).cast::<__m128i>(), px3);

        i += 32;
    }

    // Tail: remaining 0-31 bytes processed one quad at a time.
    while i < n {
        let u = i32::from(src[i]) - 128;
        let y0_val = i32::from(src[i + 1]);
        let v = i32::from(src[i + 2]) - 128;
        let y1_val = i32::from(src[i + 3]);
        let rc = (v * 359) >> 8;
        let gb = (u * 88) >> 8;
        let gr = (v * 183) >> 8;
        let bc = (u * 454) >> 8;
        let r0 = (y0_val + rc).clamp(0, 255) as u8;
        let g0 = (y0_val - gb - gr).clamp(0, 255) as u8;
        let b0 = (y0_val + bc).clamp(0, 255) as u8;
        let r1 = (y1_val + rc).clamp(0, 255) as u8;
        let g1 = (y1_val - gb - gr).clamp(0, 255) as u8;
        let b1 = (y1_val + bc).clamp(0, 255) as u8;
        let d_off = i * 2;
        dst[d_off..d_off + 8].copy_from_slice(&[b0, g0, r0, 255, b1, g1, r1, 255]);
        i += 4;
    }
}
