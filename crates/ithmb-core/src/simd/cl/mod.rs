//! CL (per-pixel nibble chroma) -> BGRA - SIMD-accelerated (SSE2, SSE4.1, SSSE3, AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

#[cfg(target_arch = "x86_64")]
mod avx2;
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
mod sse;
#[cfg(target_arch = "x86_64")]
mod sse41;

/// SSSE3 `_mm_shuffle_epi8`-based CL quad -> BGRA.
///
/// Expands 4 packed nibble chroma bytes (`Cr<<4|Cb`) to full 8-bit Cb/Cr via
/// the *17 lookup table in a single `pshufb` instruction per nibble lane,
/// then yields to the scalar `yuv_to_bgra` for BT.601 conversion.
///
/// # Safety
///
/// Must only be called on `x86`/`x86_64` where SSSE3 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
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
#[cfg(target_arch = "x86_64")]
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
    let table_128 = _mm_loadu_si128(super::CL_NIBBLE_TABLE.as_ptr().cast::<__m128i>());
    let table = _mm256_broadcastsi128_si256(table_128);
    let mask_lo = _mm256_set1_epi8(0x0F);

    let chroma_128 = _mm_cvtsi32_si128(i32::from_le_bytes([quad[4], quad[5], quad[6], quad[7]]));
    let chroma = _mm256_broadcastsi128_si256(chroma_128);

    let cb_idx = _mm256_and_si256(chroma, mask_lo);
    let cb = _mm256_shuffle_epi8(table, cb_idx);

    let cr_idx = _mm256_and_si256(_mm256_srli_epi16(chroma, 4), mask_lo);
    let cr = _mm256_shuffle_epi8(table, cr_idx);

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
        out[i * 4] = crate::pixel_utils::clamp_u8(b_arr[i]);
        out[i * 4 + 1] = crate::pixel_utils::clamp_u8(g_arr[i]);
        out[i * 4 + 2] = crate::pixel_utils::clamp_u8(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

// ---------------------------------------------------------------------------
// Runtime dispatch
// ---------------------------------------------------------------------------

/// Convert 4 CL planar pixels to 16 BGRA bytes.
///
/// Input layout (8 bytes): `[Y0, Y1, Y2, Y3, CbCr0, CbCr1, CbCr2, CbCr3]`
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn cl_quad_to_bgra(quad: &[u8; 8]) -> [u8; 16] {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    unsafe {
        if is_x86_feature_detected!("sse4.1") {
            return sse::cl_quad_to_bgra_sse41(quad);
        }
        sse::cl_quad_to_bgra_sse2(quad)
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return super::neon::cl_quad_to_bgra_neon(quad);
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    super::scalar::cl_quad_to_bgra(*quad)
}

/// Convert one row of CL planar data to BGRA.
///
/// Input `src` layout (`w * 2` bytes):
///   `src[0..w]` = Y bytes (one per pixel)
///   `src[w..2*w]` = `CbCr` bytes (Cr in high nibble, Cb in low nibble)
///
/// Output `dst`: `w * 4` bytes BGRA.
///
/// # Panics
///
/// When `dst` is not exactly `src.len() * 2` bytes.
#[inline]
pub fn cl_row_to_bgra(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(dst.len(), src.len() * 2);

    // AVX2 path (runtime-detected -- fastest 256-bit arithmetic)
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("avx2") {
        unsafe {
            return avx2::cl_row_to_bgra_avx2(src, dst);
        }
    }

    // SSE4.1 packed YUV path (runtime-detected -- faster packed clamp + pack)
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("sse4.1") {
        unsafe {
            return sse41::cl_row_to_bgra_sse41(src, dst);
        }
    }

    // SSE2 path (compile-time guaranteed on x86_64/x86)
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        sse::cl_row_to_bgra_sse2(src, dst);
    }

    // NEON path (compile-time guaranteed on aarch64)
    #[cfg(target_arch = "aarch64")]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return super::neon::cl_row_to_bgra_neon(src, dst);
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    super::scalar::cl_row_to_bgra_scalar(src, dst);
}
