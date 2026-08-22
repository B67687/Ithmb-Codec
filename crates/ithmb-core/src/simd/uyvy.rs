//! UYVY 4:2:2 -> BGRA - SIMD-accelerated (SSE2, SSE4.1, AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use crate::error::DecodeError;

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::trivially_copy_pass_by_ref)]
pub(crate) unsafe fn uyvy_quad_to_bgra_sse2(quad: &[u8; 4]) -> [u8; 8] {
    use core::arch::x86_64::{_mm_cvtsi32_si128, _mm_extract_epi16, _mm_setzero_si128, _mm_unpacklo_epi8};

    // Load 4 UYVY bytes as a 32-bit integer: [U, Y0, V, Y1]
    let data = _mm_cvtsi32_si128(i32::from_le_bytes(*quad));
    // Zero-extend bytes to 16-bit words: [U, Y0, V, Y1, 0, 0, 0, 0]
    let w = _mm_unpacklo_epi8(data, _mm_setzero_si128());

    // Extract via _mm_extract_epi16 (returns i32, value 0..255).
    let u = _mm_extract_epi16(w, 0);
    let y0 = _mm_extract_epi16(w, 1);
    let v = _mm_extract_epi16(w, 2);
    let y1 = _mm_extract_epi16(w, 3);

    // BT.601 with Q8 fixed-point (coeffs x 256, shift >> 8).
    //
    //   R = Y + (Cr - 128) x  359 / 256
    //   G = Y - (Cb - 128) x   88 / 256 - (Cr - 128) x 183 / 256
    //   B = Y + (Cb - 128) x  454 / 256
    //
    // Chroma is centered (subtract 128) before multiplication; Y uses
    // the full 0-255 range (no offset), matching `yuv::yuv_to_bgra`.
    let r0 = crate::pixel_utils::clamp_u8(y0 + (((v - 128) * 359) >> 8));
    let g0 = crate::pixel_utils::clamp_u8(y0 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b0 = crate::pixel_utils::clamp_u8(y0 + (((u - 128) * 454) >> 8));

    let r1 = crate::pixel_utils::clamp_u8(y1 + (((v - 128) * 359) >> 8));
    let g1 = crate::pixel_utils::clamp_u8(y1 - (((u - 128) * 88) >> 8) - (((v - 128) * 183) >> 8));
    let b1 = crate::pixel_utils::clamp_u8(y1 + (((u - 128) * 454) >> 8));

    [b0, g0, r0, 255, b1, g1, r1, 255]
}

/// SAFETY: see [`uyvy_quad_to_bgra_sse2`].
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::trivially_copy_pass_by_ref)]
pub(crate) unsafe fn uyvy_double_quad_to_bgra_sse2(quads: &[u8; 8]) -> Result<[u8; 16], DecodeError> {
    let left_arr: [u8; 4] = quads[..4].try_into().map_err(|_| DecodeError::BufferTooShort {
        expected: 4,
        actual: quads[..4].len(),
    })?;
    let right_arr: [u8; 4] = quads[4..].try_into().map_err(|_| DecodeError::BufferTooShort {
        expected: 4,
        actual: quads[4..].len(),
    })?;
    let left = uyvy_quad_to_bgra_sse2(&left_arr);
    let right = uyvy_quad_to_bgra_sse2(&right_arr);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&left);
    out[8..].copy_from_slice(&right);
    Ok(out)
}

#[cfg(target_arch = "x86_64")]
// ---- UYVY row -> BGRA (SSE4.1) ----
/// SAFETY: must only be called on `x86`/`x86_64` where SSE4.1 / SSSE3 is guaranteed.
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
    // C# reference: ShufY, ShufU, ShufV in IthmbCodecPlugin.SimdConstants.
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

        // pmaddwd: multiply adjacent i16 pairs, sum to i32.
        // cb = [c0,c0,c2,c2,c4,c4,c6,c6] so pairs sum 2*c*coef.
        // >> 9 gives (c*coef >> 8).
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

// ---- UYVY row -> BGRA (AVX2, 16 px/iter) ----
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

        // pmaddwd: multiply adjacent i16 pairs, sum to i32.
        // cb = [c0,c0,c2,c2,...] so pairs sum 2*c*coef. >>9 gives (c*coef >> 8).
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

// ---------------------------------------------------------------------------
// Runtime dispatch — selects the best SIMD path at run time
// ---------------------------------------------------------------------------

/// Convert one UYVY quad (4 bytes) to two BGRA pixels (8 bytes).
///
/// Input layout: `[U (Cb), Y0, V (Cr), Y1]`
/// Output layout: `[B0, G0, R0, A0, B1, G1, R1, A1]` (alpha = 255).
///
/// On `x86_64` with SSE2 this processes the quad with 16-bit fixed-point
/// arithmetic in a single SSE register pass, retiring both pixels in ~10
/// instructions (versus ~40 for two scalar calls).
#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn uyvy_quad_to_bgra(quad: &[u8; 4]) -> [u8; 8] {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        uyvy_quad_to_bgra_sse2(quad)
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return super::neon::uyvy_quad_to_bgra_neon(quad);
    }

    #[cfg(not(any(any(target_arch = "x86_64", target_arch = "x86"), target_arch = "aarch64",)))]
    super::scalar::uyvy_quad_to_bgra(quad)
}

/// Convert two UYVY quads (8 bytes) to four BGRA pixels (16 bytes).
///
/// Twice as wide as [`uyvy_quad_to_bgra`] -- better amortises SSE register
/// setup when callers have at least 8 bytes of input (the common case).
#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref, clippy::missing_panics_doc)]
pub fn uyvy_double_quad_to_bgra(quads: &[u8; 8]) -> [u8; 16] {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        uyvy_double_quad_to_bgra_sse2(quads).expect("UYVY double quad SSE2 conversion infallible")
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return super::neon::uyvy_double_quad_to_bgra_neon(quads);
    }

    #[cfg(not(any(any(target_arch = "x86_64", target_arch = "x86"), target_arch = "aarch64",)))]
    super::scalar::uyvy_double_quad_to_bgra(quads)
}

/// Convert a full row of UYVY data (4-byte quads) to BGRA.
///
/// Input: `src` contains `(w/2) * 4` bytes of UYVY quads (no odd-width trailing pixel).
/// Output: `dst` contains `(w/2) * 8` bytes of BGRA pixels.
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] when `src` does not contain a whole number of quads.
#[inline]
#[allow(clippy::too_many_lines)]
pub fn uyvy_row_to_bgra(src: &[u8], dst: &mut [u8]) -> Result<(), crate::error::DecodeError> {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("avx2") {
        return {
            unsafe { uyvy_row_to_bgra_avx2(src, dst) };
            Ok(())
        };
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("sse4.1") {
        return {
            unsafe { uyvy_row_to_bgra_sse41(src, dst) };
            Ok(())
        };
    }
    let n = src.len();
    debug_assert_eq!(dst.len(), (n / 4) * 8);
    let full_end = (n / 16) * 16;
    let mut i = 0usize;

    // Process 4 quads (8 pixels = 16 input bytes) per iteration.
    while i < full_end {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        // SAFETY: x86_64/x86 guarantees SSE2.
        unsafe {
            let q0 = uyvy_double_quad_to_bgra_sse2(&src[i..i + 8].try_into().map_err(|_| {
                crate::error::DecodeError::BufferTooShort {
                    expected: 8,
                    actual: src[i..i + 8].len(),
                }
            })?)?;
            let q1 = uyvy_double_quad_to_bgra_sse2(&src[i + 8..i + 16].try_into().map_err(|_| {
                crate::error::DecodeError::BufferTooShort {
                    expected: 8,
                    actual: src[i + 8..i + 16].len(),
                }
            })?)?;
            let d_off = i * 2;
            dst[d_off..d_off + 16].copy_from_slice(&q0);
            dst[d_off + 16..d_off + 32].copy_from_slice(&q1);
        }

        #[cfg(target_arch = "aarch64")]
        // SAFETY: aarch64 guarantees NEON.
        unsafe {
            let arr0: [u8; 8] = src[i..i + 8]
                .try_into()
                .map_err(|_| crate::error::DecodeError::BufferTooShort {
                    expected: 8,
                    actual: src[i..i + 8].len(),
                })?;
            let arr1: [u8; 8] =
                src[i + 8..i + 16]
                    .try_into()
                    .map_err(|_| crate::error::DecodeError::BufferTooShort {
                        expected: 8,
                        actual: src[i + 8..i + 16].len(),
                    })?;
            let q0 = super::neon::uyvy_double_quad_to_bgra_neon(&arr0);
            let q1 = super::neon::uyvy_double_quad_to_bgra_neon(&arr1);
            let d_off = i * 2;
            dst[d_off..d_off + 16].copy_from_slice(&q0);
            dst[d_off + 16..d_off + 32].copy_from_slice(&q1);
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
        {
            let arr0: [u8; 8] = src[i..i + 8]
                .try_into()
                .map_err(|_| crate::error::DecodeError::BufferTooShort {
                    expected: 8,
                    actual: src[i..i + 8].len(),
                })?;
            let arr1: [u8; 8] =
                src[i + 8..i + 16]
                    .try_into()
                    .map_err(|_| crate::error::DecodeError::BufferTooShort {
                        expected: 8,
                        actual: src[i + 8..i + 16].len(),
                    })?;
            let q0 = super::scalar::uyvy_double_quad_to_bgra(&arr0);
            let q1 = super::scalar::uyvy_double_quad_to_bgra(&arr1);
            let d_off = i * 2;
            dst[d_off..d_off + 16].copy_from_slice(&q0);
            dst[d_off + 16..d_off + 32].copy_from_slice(&q1);
        }

        i += 16;
    }

    // Remainder: 0-3 quads processed individually.
    while i < n {
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        // SAFETY: x86_64/x86 guarantees SSE2.
        unsafe {
            let arr: [u8; 4] = src[i..i + 4]
                .try_into()
                .map_err(|_| crate::error::DecodeError::BufferTooShort {
                    expected: 4,
                    actual: src[i..i + 4].len(),
                })?;
            let px = uyvy_quad_to_bgra_sse2(&arr);
            let d_off = i * 2;
            dst[d_off..d_off + 8].copy_from_slice(&px);
        }

        #[cfg(target_arch = "aarch64")]
        // SAFETY: aarch64 guarantees NEON.
        unsafe {
            let arr: [u8; 4] = src[i..i + 4]
                .try_into()
                .map_err(|_| crate::error::DecodeError::BufferTooShort {
                    expected: 4,
                    actual: src[i..i + 4].len(),
                })?;
            let px = super::neon::uyvy_quad_to_bgra_neon(&arr);
            let d_off = i * 2;
            dst[d_off..d_off + 8].copy_from_slice(&px);
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
        {
            let arr: [u8; 4] = src[i..i + 4]
                .try_into()
                .map_err(|_| crate::error::DecodeError::BufferTooShort {
                    expected: 4,
                    actual: src[i..i + 4].len(),
                })?;
            let px = super::scalar::uyvy_quad_to_bgra(&arr);
            let d_off = i * 2;
            dst[d_off..d_off + 8].copy_from_slice(&px);
        }

        i += 4;
    }

    Ok(())
}
