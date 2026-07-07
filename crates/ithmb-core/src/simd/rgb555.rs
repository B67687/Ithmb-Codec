//! RGB555 row -> BGRA - SIMD-accelerated (SSE2/AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn rgb555_row_to_bgra_sse2(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_loadu_si128, _mm_or_si128, _mm_packus_epi16, _mm_set1_epi8, _mm_set1_epi16,
        _mm_setzero_si128, _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128, _mm_unpackhi_epi8, _mm_unpacklo_epi8,
    };

    let n = src.len();
    debug_assert_eq!(dst.len(), (n / 2) * 4);

    let mask5 = _mm_set1_epi16(0x1F);
    let zero = _mm_setzero_si128();
    let alpha = _mm_set1_epi8(-1i8); // 0xFF

    let mut i = 0usize;
    while i + 16 <= n {
        let src_ptr = src.as_ptr().add(i);
        let dst_ptr = dst.as_mut_ptr().add(i * 2);

        // Load 8 pixels (16 bytes) as 8 x u16.
        let data = _mm_loadu_si128(src_ptr.cast::<__m128i>());

        // Extract R5 (bits 14-10), G5 (bits 9-5), B5 (bits 4-0).
        let r5 = _mm_and_si128(_mm_srli_epi16(data, 10), mask5);
        let g5 = _mm_and_si128(_mm_srli_epi16(data, 5), mask5);
        let b5 = _mm_and_si128(data, mask5);

        // MSB replicate 5->8 bits: (v << 3) | (v >> 2)
        let r8 = _mm_or_si128(_mm_slli_epi16(r5, 3), _mm_srli_epi16(r5, 2));
        let g8 = _mm_or_si128(_mm_slli_epi16(g5, 3), _mm_srli_epi16(g5, 2));
        let b8 = _mm_or_si128(_mm_slli_epi16(b5, 3), _mm_srli_epi16(b5, 2));

        // Pack to bytes (unsigned saturate from 16-bit).
        let r_u8 = _mm_packus_epi16(r8, zero);
        let g_u8 = _mm_packus_epi16(g8, zero);
        let b_u8 = _mm_packus_epi16(b8, zero);

        // Interleave to BGRA order.
        let br = _mm_unpacklo_epi8(b_u8, r_u8);
        let ga = _mm_unpacklo_epi8(g_u8, alpha);
        let lo_bgra = _mm_unpacklo_epi8(br, ga);
        let hi_bgra = _mm_unpackhi_epi8(br, ga);

        _mm_storeu_si128(dst_ptr.cast::<__m128i>(), lo_bgra);
        _mm_storeu_si128(dst_ptr.add(16).cast::<__m128i>(), hi_bgra);

        i += 16;
    }

    // Remainder pixels (scalar fallback).
    if i < n {
        super::scalar::rgb555_row_to_bgra_scalar(&src[i..], &mut dst[i * 2..]);
    }
}

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed
/// (caller must check `is_x86_feature_detected!("avx2")`).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment, clippy::similar_names)]
pub(crate) unsafe fn rgb555_row_to_bgra_avx2(src: &[u8], dst: &mut [u8]) {
    use core::arch::x86_64::{
        __m128i, __m256i, _mm_set1_epi8, _mm_storeu_si128, _mm_unpackhi_epi8, _mm_unpacklo_epi8, _mm256_and_si256,
        _mm256_extracti128_si256, _mm256_loadu_si256, _mm256_or_si256, _mm256_packus_epi16, _mm256_set1_epi16,
        _mm256_setzero_si256, _mm256_slli_epi16, _mm256_srli_epi16,
    };

    let n = src.len();
    debug_assert_eq!(dst.len(), (n / 2) * 4);

    let mask5 = _mm256_set1_epi16(0x1F);
    let zero256 = _mm256_setzero_si256();
    let alpha = _mm_set1_epi8(-1i8);

    let mut i = 0usize;
    while i + 32 <= n {
        let src_ptr = src.as_ptr().add(i);
        let dst_ptr = dst.as_mut_ptr().add(i * 2);

        // Load 16 pixels (32 bytes) as 16 x u16.
        let data = _mm256_loadu_si256(src_ptr.cast::<__m256i>());

        // Extract R5 (bits 14-10), G5 (bits 9-5), B5 (bits 4-0).
        let r5 = _mm256_and_si256(_mm256_srli_epi16(data, 10), mask5);
        let g5 = _mm256_and_si256(_mm256_srli_epi16(data, 5), mask5);
        let b5 = _mm256_and_si256(data, mask5);

        // MSB replicate 5->8 bits: (v << 3) | (v >> 2)
        let r8 = _mm256_or_si256(_mm256_slli_epi16(r5, 3), _mm256_srli_epi16(r5, 2));
        let g8 = _mm256_or_si256(_mm256_slli_epi16(g5, 3), _mm256_srli_epi16(g5, 2));
        let b8 = _mm256_or_si256(_mm256_slli_epi16(b5, 3), _mm256_srli_epi16(b5, 2));

        // Pack to bytes.
        let r_u8 = _mm256_packus_epi16(r8, zero256);
        let g_u8 = _mm256_packus_epi16(g8, zero256);
        let b_u8 = _mm256_packus_epi16(b8, zero256);

        // Extract 128-bit lanes.
        let r_lo = _mm256_extracti128_si256(r_u8, 0);
        let r_hi = _mm256_extracti128_si256(r_u8, 1);
        let g_lo = _mm256_extracti128_si256(g_u8, 0);
        let g_hi = _mm256_extracti128_si256(g_u8, 1);
        let b_lo = _mm256_extracti128_si256(b_u8, 0);
        let b_hi = _mm256_extracti128_si256(b_u8, 1);

        // Lower 8 pixels -> BGRA.
        let br_lo = _mm_unpacklo_epi8(b_lo, r_lo);
        let ga_lo = _mm_unpacklo_epi8(g_lo, alpha);
        let px0 = _mm_unpacklo_epi8(br_lo, ga_lo); // pixels 0-3
        let px1 = _mm_unpackhi_epi8(br_lo, ga_lo); // pixels 4-7

        // Upper 8 pixels -> BGRA.
        let br_hi = _mm_unpacklo_epi8(b_hi, r_hi);
        let ga_hi = _mm_unpacklo_epi8(g_hi, alpha);
        let px2 = _mm_unpacklo_epi8(br_hi, ga_hi); // pixels 8-11
        let px3 = _mm_unpackhi_epi8(br_hi, ga_hi); // pixels 12-15

        _mm_storeu_si128(dst_ptr.cast::<__m128i>(), px0);
        _mm_storeu_si128(dst_ptr.add(16).cast::<__m128i>(), px1);
        _mm_storeu_si128(dst_ptr.add(32).cast::<__m128i>(), px2);
        _mm_storeu_si128(dst_ptr.add(48).cast::<__m128i>(), px3);

        i += 32;
    }

    // Remainder pixels (scalar fallback).
    if i < n {
        super::scalar::rgb555_row_to_bgra_scalar(&src[i..], &mut dst[i * 2..]);
    }
}
