//! UYVY row -> BGRA via SSE4.1/SSSE3 (8 pixels per iteration).

#[cfg(target_arch = "x86_64")]
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[target_feature(enable = "ssse3")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_possible_truncation, clippy::similar_names)]
pub(crate) unsafe fn uyvy_row_to_bgra_sse41(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi16, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_madd_epi16, _mm_max_epi16, _mm_min_epi16,
        _mm_packs_epi32, _mm_packus_epi16, _mm_set_epi8, _mm_set1_epi16, _mm_setzero_si128, _mm_shuffle_epi8,
        _mm_srai_epi32, _mm_storeu_si128, _mm_sub_epi16, _mm_unpackhi_epi8, _mm_unpackhi_epi32, _mm_unpacklo_epi8,
        _mm_unpacklo_epi32,
    };
    let n = src.len();
    let full_end = (n / 16) * 16;
    let mut i = 0usize;

    // SSSE3 pshufb masks for deinterleaving 16 UYVY bytes into u16 lanes.
    let shuf_y = _mm_set_epi8(
        -128, 15, -128, 13, -128, 11, -128, 9, -128, 7, -128, 5, -128, 3, -128, 1,
    );
    let shuf_u = _mm_set_epi8(-128, 12, -128, 12, -128, 8, -128, 8, -128, 4, -128, 4, -128, 0, -128, 0);
    let shuf_v = _mm_set_epi8(
        -128, 14, -128, 14, -128, 10, -128, 10, -128, 6, -128, 6, -128, 2, -128, 2,
    );

    let zero = _mm_setzero_si128();
    let max255 = _mm_set1_epi16(255);
    let offset128 = _mm_set1_epi16(128);
    let alpha8 = _mm_cmpeq_epi8(zero, zero);

    // BT.601 coefficients (i16, fits all products via pmaddwd i32 accumulation)
    let coeff_rc = _mm_set1_epi16(359);
    let coeff_gb = _mm_set1_epi16(88);
    let coeff_gr = _mm_set1_epi16(183);
    let coeff_bc = _mm_set1_epi16(454);

    while i < full_end {
        // Load 16 bytes = 8 UYVY pixels
        let v = _mm_loadu_si128(src.as_ptr().add(i).cast::<__m128i>());

        // SSSE3 pshufb: deinterleave into u16 lanes (byte, 0 per lane)
        let y_vals = _mm_shuffle_epi8(v, shuf_y);
        let u_vals = _mm_shuffle_epi8(v, shuf_u);
        let v_vals = _mm_shuffle_epi8(v, shuf_v);

        // Signed chroma offsets: Cb = U - 128, Cr = V - 128
        let cb = _mm_sub_epi16(u_vals, offset128);
        let cr = _mm_sub_epi16(v_vals, offset128);

        let rc_i32 = _mm_srai_epi32(_mm_madd_epi16(cr, coeff_rc), 9);
        let gb_i32 = _mm_srai_epi32(_mm_madd_epi16(cb, coeff_gb), 9);
        let gr_i32 = _mm_srai_epi32(_mm_madd_epi16(cr, coeff_gr), 9);
        let bc_i32 = _mm_srai_epi32(_mm_madd_epi16(cb, coeff_bc), 9);

        // Unpack 4 i32 results -> 8 i16, duplicating chroma for each pixel pair
        let rc = _mm_packs_epi32(_mm_unpacklo_epi32(rc_i32, rc_i32), _mm_unpackhi_epi32(rc_i32, rc_i32));
        let gb = _mm_packs_epi32(_mm_unpacklo_epi32(gb_i32, gb_i32), _mm_unpackhi_epi32(gb_i32, gb_i32));
        let gr = _mm_packs_epi32(_mm_unpacklo_epi32(gr_i32, gr_i32), _mm_unpackhi_epi32(gr_i32, gr_i32));
        let bc = _mm_packs_epi32(_mm_unpacklo_epi32(bc_i32, bc_i32), _mm_unpackhi_epi32(bc_i32, bc_i32));

        // YUV -> RGB (all i16, ranges stay well within i16)
        let r = _mm_add_epi16(y_vals, rc);
        let g = _mm_sub_epi16(_mm_sub_epi16(y_vals, gb), gr);
        let b = _mm_add_epi16(y_vals, bc);

        // Clamp to [0, 255]
        let r_c = _mm_max_epi16(_mm_min_epi16(r, max255), zero);
        let g_c = _mm_max_epi16(_mm_min_epi16(g, max255), zero);
        let b_c = _mm_max_epi16(_mm_min_epi16(b, max255), zero);

        // Pack i16 -> u8 (unsigned saturate, already clamped)
        let b_u8 = _mm_packus_epi16(b_c, zero);
        let g_u8 = _mm_packus_epi16(g_c, zero);
        let r_u8 = _mm_packus_epi16(r_c, zero);

        // Interleave to BGRA: two-level unpack
        let bg = _mm_unpacklo_epi8(b_u8, r_u8);
        let ga = _mm_unpacklo_epi8(g_u8, alpha8);

        let lo = _mm_unpacklo_epi8(bg, ga); // pixels 0-3
        let hi = _mm_unpackhi_epi8(bg, ga); // pixels 4-7

        // Store 32 bytes (8 BGRA pixels)
        let d_off = i * 2;
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off).cast::<__m128i>(), lo);
        _mm_storeu_si128(dst.as_mut_ptr().add(d_off + 16).cast::<__m128i>(), hi);

        i += 16;
    }

    // Tail: remaining 0-15 bytes processed one quad at a time (scalar)
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
