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

    let mut r_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    let mut g_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    let mut b_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    _mm_storeu_si128(r_tmp.as_mut_ptr(), r);
    _mm_storeu_si128(g_tmp.as_mut_ptr(), g);
    _mm_storeu_si128(b_tmp.as_mut_ptr(), b);
    let r_arr: [i32; 4] = core::mem::transmute(r_tmp.assume_init());
    let g_arr: [i32; 4] = core::mem::transmute(g_tmp.assume_init());
    let b_arr: [i32; 4] = core::mem::transmute(b_tmp.assume_init());

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
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_row_to_bgra_sse41(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_and_si128, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_max_epi32, _mm_min_epi32,
        _mm_mullo_epi32, _mm_packus_epi16, _mm_packus_epi32, _mm_set1_epi8, _mm_set1_epi16, _mm_set1_epi32,
        _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srai_epi32, _mm_srli_epi16, _mm_storeu_si128,
        _mm_sub_epi32, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    let full_end = (n_pixels / 4) * 4;

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

    let mut i = 0usize;
    while i < full_end {
        // Load 4 Y bytes and zero-extend to i32x4.
        let y_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([y[i], y[i + 1], y[i + 2], y[i + 3]]));
        let y_vals = _mm_cvtepu8_epi32(y_chunk);

        // Load 4 CbCr bytes; each packed byte = (Cr<<4)|Cb.
        let c_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ]));

        // Cb: mask low nibble -> pshufb (*16) -> zero-extend to i32x4.
        let cb_idx = _mm_and_si128(c_chunk, mask_lo);
        let cb_bytes = _mm_shuffle_epi8(tbl, cb_idx);
        let cb = _mm_cvtepu8_epi32(cb_bytes);

        // Cr: shift high nibble to low nibble position (via 16-bit shift)
        //   -> mask -> pshufb (*16) -> zero-extend to i32x4.
        let cr_idx = _mm_and_si128(_mm_srli_epi16(c_chunk, 4), mask_lo);
        let cr_bytes = _mm_shuffle_epi8(tbl, cr_idx);
        let cr = _mm_cvtepu8_epi32(cr_bytes);

        // Center chroma around 128.
        let cb_c = _mm_sub_epi32(cb, cent);
        let cr_c = _mm_sub_epi32(cr, cent);

        // Per-pixel chroma contributions (BT.601 fixed-point: (c * coeff) >> 8).
        let rc = _mm_srai_epi32(_mm_mullo_epi32(cr_c, coef_359), 8);
        let gb = _mm_srai_epi32(_mm_mullo_epi32(cb_c, coef_88), 8);
        let gr = _mm_srai_epi32(_mm_mullo_epi32(cr_c, coef_183), 8);
        let bc = _mm_srai_epi32(_mm_mullo_epi32(cb_c, coef_454), 8);

        // R = Y + rc
        // G = Y - gb - gr
        // B = Y + bc
        let r = _mm_add_epi32(y_vals, rc);
        let g = _mm_sub_epi32(_mm_sub_epi32(y_vals, gb), gr);
        let b = _mm_add_epi32(y_vals, bc);

        // Clamp to [0, 255].
        let r_c = _mm_max_epi32(_mm_min_epi32(r, max_val), zero);
        let g_c = _mm_max_epi32(_mm_min_epi32(g, max_val), zero);
        let b_c = _mm_max_epi32(_mm_min_epi32(b, max_val), zero);

        // ---- Pack i32 -> u16 -> u8 with interleave to BGRA order ----
        let b16 = _mm_packus_epi32(b_c, zero);
        let g16 = _mm_packus_epi32(g_c, zero);
        let r16 = _mm_packus_epi32(r_c, zero);

        let br = _mm_unpacklo_epi16(b16, r16);
        let ga = _mm_unpacklo_epi16(g16, a16);

        let br_u8 = _mm_packus_epi16(br, zero);
        let ga_u8 = _mm_packus_epi16(ga, zero);

        let result = _mm_unpacklo_epi8(br_u8, ga_u8);

        // Store 16 bytes (4 BGRA pixels) directly — no copy_from_slice.
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

    let mut r_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    let mut g_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    let mut b_tmp = core::mem::MaybeUninit::<__m128i>::uninit();
    _mm_storeu_si128(r_tmp.as_mut_ptr(), r_lo);
    _mm_storeu_si128(g_tmp.as_mut_ptr(), g_lo);
    _mm_storeu_si128(b_tmp.as_mut_ptr(), b_lo);
    let r_arr: [i32; 4] = core::mem::transmute(r_tmp.assume_init());
    let g_arr: [i32; 4] = core::mem::transmute(g_tmp.assume_init());
    let b_arr: [i32; 4] = core::mem::transmute(b_tmp.assume_init());

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::yuv::clamp(b_arr[i]);
        out[i * 4 + 1] = crate::yuv::clamp(g_arr[i]);
        out[i * 4 + 2] = crate::yuv::clamp(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}
