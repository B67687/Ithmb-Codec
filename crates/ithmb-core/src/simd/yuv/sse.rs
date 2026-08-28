//! YCbCr 4:2:0 -> BGRA SSE2 quad conversion.

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use core::arch::x86_64::__m128i;

// ---- SSE2 quad (4× Y + 1× Cb + 1× Cr -> 16× BGRA) ----
/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(clippy::similar_names, unsafe_op_in_unsafe_fn, clippy::trivially_copy_pass_by_ref)]
pub(crate) unsafe fn yuv420_quad_to_bgra_sse2(quad: &[u8; 6]) -> [u8; 16] {
    use core::arch::x86_64::{
        _mm_add_epi32, _mm_cvtsi32_si128, _mm_set1_epi32, _mm_setzero_si128, _mm_storeu_si128, _mm_sub_epi32,
        _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    // ---- Precompute chroma contributions (scalar, once for all 4 pixels) ----
    let cb = i32::from(quad[4]) - 128;
    let cr = i32::from(quad[5]) - 128;
    let rc = (cr * 359) >> 8; // Cr channel to R
    let gb = (cb * 88) >> 8; // Cb channel to G (green - cb)
    let gr = (cr * 183) >> 8; // Cr channel to G (green - cr)
    let bc = (cb * 454) >> 8; // Cb channel to B

    // ---- Load 4 Y values and zero-extend to 32-bit ----
    let y_bytes = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y_words = _mm_unpacklo_epi8(y_bytes, _mm_setzero_si128()); // 16-bit
    let y = _mm_unpacklo_epi16(y_words, _mm_setzero_si128()); // 4 x i32

    // ---- Compute R/G/B in parallel (pure SSE2) ----
    let rc_splat = _mm_set1_epi32(rc);
    let gb_splat = _mm_set1_epi32(gb);
    let gr_splat = _mm_set1_epi32(gr);
    let bc_splat = _mm_set1_epi32(bc);

    let r = _mm_add_epi32(y, rc_splat);
    let g = _mm_sub_epi32(_mm_sub_epi32(y, gb_splat), gr_splat);
    let b = _mm_add_epi32(y, bc_splat);

    // ---- Store via `__m128i` temporaries (16-byte aligned, no cast_alignment) ----
    let mut r_arr = [0i32; 4];
    let mut g_arr = [0i32; 4];
    let mut b_arr = [0i32; 4];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast::<__m128i>(), r);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast::<__m128i>(), g);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast::<__m128i>(), b);

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::pixel_utils::clamp_u8(b_arr[i]);
        out[i * 4 + 1] = crate::pixel_utils::clamp_u8(g_arr[i]);
        out[i * 4 + 2] = crate::pixel_utils::clamp_u8(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}
