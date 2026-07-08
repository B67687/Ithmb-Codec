//! CL (per-pixel nibble chroma) -> BGRA - SIMD-accelerated (SSE2, SSE4.1, SSSE3, AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn cl_quad_to_bgra_sse2(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_cvtsi32_si128, _mm_set_epi32, _mm_setzero_si128, _mm_storeu_si128, _mm_sub_epi32,
        _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

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

    let y_bytes = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y_words = _mm_unpacklo_epi8(y_bytes, _mm_setzero_si128());
    let y = _mm_unpacklo_epi16(y_words, _mm_setzero_si128());

    let rc = _mm_set_epi32(rc_arr[3], rc_arr[2], rc_arr[1], rc_arr[0]);
    let gb = _mm_set_epi32(gb_arr[3], gb_arr[2], gb_arr[1], gb_arr[0]);
    let gr = _mm_set_epi32(gr_arr[3], gr_arr[2], gr_arr[1], gr_arr[0]);
    let bc = _mm_set_epi32(bc_arr[3], bc_arr[2], bc_arr[1], bc_arr[0]);

    let r = _mm_add_epi32(y, rc);
    let g = _mm_sub_epi32(_mm_sub_epi32(y, gb), gr);
    let b = _mm_add_epi32(y, bc);

    let mut r_arr = [0i32; 4];
    let mut g_arr = [0i32; 4];
    let mut b_arr = [0i32; 4];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast::<__m128i>(), r);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast::<__m128i>(), g);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast::<__m128i>(), b);

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::yuv::clamp(b_arr[i]);
        out[i * 4 + 1] = crate::yuv::clamp(g_arr[i]);
        out[i * 4 + 2] = crate::yuv::clamp(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_row_to_bgra_sse2(src: &[u8], dst: &mut [u8]) {
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
        let out0 = cl_quad_to_bgra_sse2(&q0);
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
        let out1 = cl_quad_to_bgra_sse2(&q1);
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
        let out = cl_quad_to_bgra_sse2(&q);
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

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "sse4.1")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn cl_quad_to_bgra_sse41(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_max_epi32, _mm_min_epi32, _mm_packus_epi16,
        _mm_packus_epi32, _mm_set_epi32, _mm_set1_epi16, _mm_set1_epi32, _mm_setzero_si128, _mm_storeu_si128,
        _mm_sub_epi32, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    // ---- Precompute per-pixel chroma contributions ----
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

    // ---- Load 4 Y values and SSE4.1 zero-extend to 32-bit ----
    let y_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y = _mm_cvtepu8_epi32(y_chunk);

    // ---- Load per-pixel chroma contributions ----
    let rc = _mm_set_epi32(rc_arr[3], rc_arr[2], rc_arr[1], rc_arr[0]);
    let gb = _mm_set_epi32(gb_arr[3], gb_arr[2], gb_arr[1], gb_arr[0]);
    let gr = _mm_set_epi32(gr_arr[3], gr_arr[2], gr_arr[1], gr_arr[0]);
    let bc = _mm_set_epi32(bc_arr[3], bc_arr[2], bc_arr[1], bc_arr[0]);

    // ---- Compute R/G/B with packed arithmetic ----
    let r = _mm_add_epi32(y, rc);
    let g = _mm_sub_epi32(_mm_sub_epi32(y, gb), gr);
    let b = _mm_add_epi32(y, bc);

    // ---- Clamp to [0, 255] with packed min/max ----
    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi32(255);
    let r_c = _mm_max_epi32(_mm_min_epi32(r, max_val), zero);
    let g_c = _mm_max_epi32(_mm_min_epi32(g, max_val), zero);
    let b_c = _mm_max_epi32(_mm_min_epi32(b, max_val), zero);

    // ---- Pack i32 -> u16 -> u8 with interleave to BGRA order ----
    let b16 = _mm_packus_epi32(b_c, zero);
    let g16 = _mm_packus_epi32(g_c, zero);
    let r16 = _mm_packus_epi32(r_c, zero);
    let a16 = _mm_set1_epi16(255i16);

    let br = _mm_unpacklo_epi16(b16, r16);
    let ga = _mm_unpacklo_epi16(g16, a16);

    let br_u8 = _mm_packus_epi16(br, zero);
    let ga_u8 = _mm_packus_epi16(ga, zero);

    let result = _mm_unpacklo_epi8(br_u8, ga_u8);

    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), result);
    out
}

/// SAFETY: must only be called on `x86`/`x86_64` where SSE4.1+SSSE3 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
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

    // Nibble-to-byte*16 lookup table for SSSE3 pshufb:
    // maps nibble n -> n*16 (expands 4-bit value to Cb/Cr byte position).
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
    // Process 16 pixels per iteration: load 16-wide Y and chroma, expand nibbles once
    // via SSSE3 pshufb, split into 4 quads for existing YUV arithmetic, store 64 bytes.
    while i < full_end_16 {
        // Load 16 Y bytes and 16 chroma bytes at once.
        let y16 = _mm_loadu_si128(y.as_ptr().add(i).cast::<__m128i>());
        let c16 = _mm_loadu_si128(chroma.as_ptr().add(i).cast::<__m128i>());

        // Nibble expansion on all 16 chroma bytes at once (saves 6 pshufb vs per-quad).
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

/// SSSE3 `_mm_shuffle_epi8`-based CL quad -> BGRA.
///
/// Expands 4 packed nibble chroma bytes (`Cr<<4|Cb`) to full 8-bit Cb/Cr via
/// the *17 lookup table in a single `pshufb` instruction per nibble lane,
/// then yields to the scalar `yuv_to_bgra` for BT.601 conversion.
///
/// # Safety
///
/// Must only be called on `x86`/`x86_64` where SSSE3 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[target_feature(enable = "ssse3")]
#[cfg(test)]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_quad_to_bgra_ssse3(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_cvtsi32_si128, _mm_loadu_si128, _mm_set1_epi8, _mm_shuffle_epi8, _mm_srli_epi16,
        _mm_storeu_si128,
    };

    let table = _mm_loadu_si128(super::CL_NIBBLE_TABLE.as_ptr().cast::<__m128i>());
    let mask_lo = _mm_set1_epi8(0x0F);

    // Load 4 chroma bytes into the lower 32 bits (bytes 4-7 = Ch0..Ch3).
    let chroma = _mm_cvtsi32_si128(i32::from_le_bytes([quad[4], quad[5], quad[6], quad[7]]));

    // Low nibble = Cb.  Index via mask -> pshufb -> Cb * 17.
    let cb_idx = _mm_and_si128(chroma, mask_lo);
    let cb = _mm_shuffle_epi8(table, cb_idx);

    // High nibble = Cr.  Shift-right 4 (via 16-bit shift), mask -> pshufb -> Cr * 17.
    let cr_idx = _mm_and_si128(_mm_srli_epi16(chroma, 4), mask_lo);
    let cr = _mm_shuffle_epi8(table, cr_idx);

    // Store expanded Cb/Cr back to scalar arrays for YUV conversion.
    let mut cb_vals = [0u8; 16];
    let mut cr_vals = [0u8; 16];
    _mm_storeu_si128(cb_vals.as_mut_ptr().cast::<__m128i>(), cb);
    _mm_storeu_si128(cr_vals.as_mut_ptr().cast::<__m128i>(), cr);

    // Per-pixel BT.601 YUV->BGRA (scalar path).
    let mut out = [0u8; 16];
    for i in 0..4 {
        let px = crate::yuv::yuv_to_bgra(quad[i], cb_vals[i], cr_vals[i]);
        out[i * 4..][..4].copy_from_slice(&px);
    }
    out
}

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline]
#[cfg(test)]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn cl_quad_to_bgra_avx2(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_cvtsi32_si128, _mm_loadu_si128, _mm_storeu_si128, _mm256_add_epi32, _mm256_and_si256,
        _mm256_broadcastsi128_si256, _mm256_cvtepu8_epi32, _mm256_extracti128_si256, _mm256_set1_epi8,
        _mm256_setr_epi32, _mm256_shuffle_epi8, _mm256_srli_epi16, _mm256_sub_epi32,
    };

    // ---- pshufb nibble expansion (*17) ----
    // vpshufb operates per 128-bit lane; we broadcast the 4 chroma bytes across
    // both lanes so the same shuffle extracts Cb and Cr for all 8 lanes.
    // Load *17 table as 128-bit and broadcast to both 256-bit lanes.
    let table_128 = _mm_loadu_si128(super::CL_NIBBLE_TABLE.as_ptr().cast::<__m128i>());
    let table = _mm256_broadcastsi128_si256(table_128);
    let mask_lo = _mm256_set1_epi8(0x0F);

    // Load 4 chroma bytes and broadcast to both 128-bit lanes.
    let chroma_128 = _mm_cvtsi32_si128(i32::from_le_bytes([quad[4], quad[5], quad[6], quad[7]]));
    let chroma = _mm256_broadcastsi128_si256(chroma_128);

    // Cb = low nibble -> *17 (both lanes simultaneously via vpshufb).
    let cb_idx = _mm256_and_si256(chroma, mask_lo);
    let cb = _mm256_shuffle_epi8(table, cb_idx);

    // Cr = high nibble -> shift right 4 -> mask -> *17.
    let cr_idx = _mm256_and_si256(_mm256_srli_epi16(chroma, 4), mask_lo);
    let cr = _mm256_shuffle_epi8(table, cr_idx);

    // Extract lower 128-bit lane (4 expanded Cb/Cr values).
    let cb_128 = _mm256_extracti128_si256(cb, 0);
    let cr_128 = _mm256_extracti128_si256(cr, 0);

    // ---- Compute chroma contributions (scalar, once per pixel) ----
    let mut cb_vals = [0u8; 16];
    let mut cr_vals = [0u8; 16];
    _mm_storeu_si128(cb_vals.as_mut_ptr().cast::<__m128i>(), cb_128);
    _mm_storeu_si128(cr_vals.as_mut_ptr().cast::<__m128i>(), cr_128);

    let mut rc_arr = [0i32; 4];
    let mut gb_arr = [0i32; 4];
    let mut gr_arr = [0i32; 4];
    let mut bc_arr = [0i32; 4];
    for i in 0..4 {
        let cb_c = i32::from(cb_vals[i]) - 128;
        let cr_c = i32::from(cr_vals[i]) - 128;
        rc_arr[i] = (cr_c * 359) >> 8;
        gb_arr[i] = (cb_c * 88) >> 8;
        gr_arr[i] = (cr_c * 183) >> 8;
        bc_arr[i] = (cb_c * 454) >> 8;
    }

    // ---- AVX2 YUV arithmetic ----
    let y_bytes = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y = _mm256_cvtepu8_epi32(y_bytes);

    let rc = _mm256_setr_epi32(rc_arr[0], rc_arr[1], rc_arr[2], rc_arr[3], 0, 0, 0, 0);
    let gb = _mm256_setr_epi32(gb_arr[0], gb_arr[1], gb_arr[2], gb_arr[3], 0, 0, 0, 0);
    let gr = _mm256_setr_epi32(gr_arr[0], gr_arr[1], gr_arr[2], gr_arr[3], 0, 0, 0, 0);
    let bc = _mm256_setr_epi32(bc_arr[0], bc_arr[1], bc_arr[2], bc_arr[3], 0, 0, 0, 0);

    let r = _mm256_add_epi32(y, rc);
    let g = _mm256_sub_epi32(_mm256_sub_epi32(y, gb), gr);
    let b = _mm256_add_epi32(y, bc);

    // Extract lower 128 bits (4 x i32 per channel).
    let r_lo = _mm256_extracti128_si256(r, 0);
    let g_lo = _mm256_extracti128_si256(g, 0);
    let b_lo = _mm256_extracti128_si256(b, 0);

    let mut r_arr = [0i32; 4];
    let mut g_arr = [0i32; 4];
    let mut b_arr = [0i32; 4];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast::<__m128i>(), r_lo);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast::<__m128i>(), g_lo);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast::<__m128i>(), b_lo);

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::yuv::clamp(b_arr[i]);
        out[i * 4 + 1] = crate::yuv::clamp(g_arr[i]);
        out[i * 4 + 2] = crate::yuv::clamp(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

// ---------------------------------------------------------------------------
// AVX2 row decoder — 8 pixels per iteration, 256-bit arithmetic throughout
// ---------------------------------------------------------------------------

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
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
        // per lane: [b0,r0,b1,r1,b2,r2,b3,r3, g0,255,g1,255,g2,255,g3,255]

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
