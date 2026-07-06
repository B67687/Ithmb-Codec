//! YCbCr 4:2:0 -> BGRA - SIMD-accelerated (SSE2, SSE4.1, AVX2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

use core::arch::x86_64::__m128i;

// ---- SSE2 quad (4× Y + 1× Cb + 1× Cr -> 16× BGRA) ----
/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
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
    // The SIMD arithmetic still wins on the 4-wide BT.601; the store+clamp
    // is scalar but avoids needing SSE4.1 (min/max_epi32) or SSSE3 (pshufb).
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

// ---- SSE4.1 quad helper (used by sse41 and avx2 row functions) ----
/// SSE4.1: Process one chroma quad (4 Y bytes, 1 Cb, 1 Cr) -> 2 BGRA pixels
/// stored to `dst` at offset `c*8` (top row) and `c*8 + w*4` (bottom row).
///
/// # Safety
/// - Must be called on `x86`/`x86_64` with SSE4.1 enabled.
/// - `dst` must have sufficient capacity for the writes.
/// - `y_row`, `cb_row`, `cr_row` must have valid indices at position `c`.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[inline]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::too_many_arguments, clippy::cast_sign_loss, unsafe_op_in_unsafe_fn)]
unsafe fn store_sse41_quad(
    y_row: &[u8],
    w: usize,
    c: usize,
    cb_row: &[u8],
    cr_row: &[u8],
    max_val: __m128i,
    zero: __m128i,
    a16: __m128i,
    dst: &mut [u8],
) {
    use core::arch::x86_64::{
        _mm_add_epi32, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_max_epi32, _mm_min_epi32, _mm_packus_epi16,
        _mm_packus_epi32, _mm_set1_epi32, _mm_storel_epi64, _mm_sub_epi32, _mm_unpackhi_epi64, _mm_unpacklo_epi8,
        _mm_unpacklo_epi16,
    };

    let y_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([
        y_row[c * 2],
        y_row[c * 2 + 1],
        y_row[w + c * 2],
        y_row[w + c * 2 + 1],
    ]));
    let y = _mm_cvtepu8_epi32(y_chunk);

    let cb = cb_row[c] as i32 - 128;
    let cr = cr_row[c] as i32 - 128;

    // BT.601 chroma contributions (Q8 fixed-point)
    let rc = (cr * 359) >> 8;
    let gb = (cb * 88) >> 8;
    let gr = (cr * 183) >> 8;
    let bc = (cb * 454) >> 8;

    // SSE4.1 packed R/G/B with splatted chroma
    let r = _mm_add_epi32(y, _mm_set1_epi32(rc));
    let g = _mm_sub_epi32(_mm_sub_epi32(y, _mm_set1_epi32(gb)), _mm_set1_epi32(gr));
    let b = _mm_add_epi32(y, _mm_set1_epi32(bc));

    // SSE4.1 packed clamp (PMAXSD + PMINSD)
    let r_c = _mm_max_epi32(_mm_min_epi32(r, max_val), zero);
    let g_c = _mm_max_epi32(_mm_min_epi32(g, max_val), zero);
    let b_c = _mm_max_epi32(_mm_min_epi32(b, max_val), zero);

    // Pack i32 -> u16 -> u8 -> BGRA interleave
    let b16 = _mm_packus_epi32(b_c, zero);
    let g16 = _mm_packus_epi32(g_c, zero);
    let r16 = _mm_packus_epi32(r_c, zero);
    let br = _mm_unpacklo_epi16(b16, r16);
    let ga = _mm_unpacklo_epi16(g16, a16);
    let br_u8 = _mm_packus_epi16(br, zero);
    let ga_u8 = _mm_packus_epi16(ga, zero);
    let result = _mm_unpacklo_epi8(br_u8, ga_u8);

    let q_off = c * 8;
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off).cast::<__m128i>(), result);
    let hi = _mm_unpackhi_epi64(result, zero);
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off + w * 4).cast::<__m128i>(), hi);
}

// ---- YCbCr 4:2:0 row pair -> BGRA (SSE4.1) ----
/// SAFETY: must only be called on `x86`/`x86_64` where SSE4.1 is guaranteed.
#[cfg(all(feature = "simd", any(target_arch = "x86_64", target_arch = "x86")))]
#[inline]
#[target_feature(enable = "sse4.1")]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn yuv420_row_pair_to_bgra_sse41(
    y_row: &[u8],
    cb_row: &[u8],
    cr_row: &[u8],
    dst: &mut [u8],
    w: usize,
    cb_w: usize,
) {
    use core::arch::x86_64::{_mm_set1_epi16, _mm_set1_epi32, _mm_setzero_si128};

    let max_val = _mm_set1_epi32(255);
    let zero = _mm_setzero_si128();
    let a16 = _mm_set1_epi16(255i16);
    let mut cx = 0usize;

    // Process 4 chroma positions (8 pixels = 2 rows x 4 cols) per iteration.
    while cx + 3 < cb_w {
        for q in 0..4 {
            store_sse41_quad(y_row, w, cx + q, cb_row, cr_row, max_val, zero, a16, dst);
        }
        cx += 4;
    }

    // Remainder: handle 1-3 quads when cb_w % 4 != 0
    if cx + 1 < cb_w {
        for q in 0..2 {
            store_sse41_quad(y_row, w, cx + q, cb_row, cr_row, max_val, zero, a16, dst);
        }
        cx += 2;
    }

    // Single quad remainder (odd cb_w or after pair remainder)
    if cx < cb_w {
        store_sse41_quad(y_row, w, cx, cb_row, cr_row, max_val, zero, a16, dst);
    }
}

// ---- AVX2 chroma-pair helper (2 positions at once via 256-bit ops) ----
/// AVX2: Process two chroma positions (8 Y bytes, 2 Cb, 2 Cr) -> 4 BGRA pixels
/// stored to `dst` at offsets `c*8`, `c*8 + w*4`, `(c+1)*8`, `(c+1)*8 + w*4`.
///
/// # Safety
/// - Must be called on `x86_64` with AVX2 enabled.
/// - `dst` must have sufficient capacity for the writes.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments, unsafe_op_in_unsafe_fn)]
unsafe fn store_avx2_chroma_pair(
    y_row: &[u8],
    w: usize,
    c: usize,
    cb_row: &[u8],
    cr_row: &[u8],
    max_val: __m128i,
    zero: __m128i,
    a16: __m128i,
    dst: &mut [u8],
) {
    use core::arch::x86_64::{
        _mm_cvtsi32_si128, _mm_max_epi32, _mm_min_epi32, _mm_packus_epi16, _mm_packus_epi32, _mm_storel_epi64,
        _mm_unpackhi_epi64, _mm_unpacklo_epi8, _mm_unpacklo_epi16, _mm_unpacklo_epi32, _mm256_add_epi32,
        _mm256_cvtepu8_epi32, _mm256_extracti128_si256, _mm256_setr_epi32, _mm256_sub_epi32,
    };

    // ---- Load 8 Y bytes for 2 chroma positions (4 Y per position) ----
    let y0_quad = _mm_cvtsi32_si128(i32::from_le_bytes([
        y_row[c * 2],
        y_row[c * 2 + 1],
        y_row[w + c * 2],
        y_row[w + c * 2 + 1],
    ]));
    let y1_quad = _mm_cvtsi32_si128(i32::from_le_bytes([
        y_row[(c + 1) * 2],
        y_row[(c + 1) * 2 + 1],
        y_row[w + (c + 1) * 2],
        y_row[w + (c + 1) * 2 + 1],
    ]));
    let y_combined = _mm_unpacklo_epi32(y0_quad, y1_quad);
    let y = _mm256_cvtepu8_epi32(y_combined);

    // ---- Chroma contributions for positions c and c+1 ----
    let cb0 = cb_row[c] as i32 - 128;
    let cr0 = cr_row[c] as i32 - 128;
    let rc0 = (cr0 * 359) >> 8;
    let gb0 = (cb0 * 88) >> 8;
    let gr0 = (cr0 * 183) >> 8;
    let bc0 = (cb0 * 454) >> 8;

    let cb1 = cb_row[c + 1] as i32 - 128;
    let cr1 = cr_row[c + 1] as i32 - 128;
    let rc1 = (cr1 * 359) >> 8;
    let gb1 = (cb1 * 88) >> 8;
    let gr1 = (cr1 * 183) >> 8;
    let bc1 = (cb1 * 454) >> 8;

    // ---- Splat chroma contributions: first 4 lanes for c, next 4 for c+1 ----
    let rc = _mm256_setr_epi32(rc0, rc0, rc0, rc0, rc1, rc1, rc1, rc1);
    let gb = _mm256_setr_epi32(gb0, gb0, gb0, gb0, gb1, gb1, gb1, gb1);
    let gr = _mm256_setr_epi32(gr0, gr0, gr0, gr0, gr1, gr1, gr1, gr1);
    let bc = _mm256_setr_epi32(bc0, bc0, bc0, bc0, bc1, bc1, bc1, bc1);

    // ---- Compute R/G/B with 256-bit packed arithmetic ----
    let r = _mm256_add_epi32(y, rc);
    let g = _mm256_sub_epi32(_mm256_sub_epi32(y, gb), gr);
    let b = _mm256_add_epi32(y, bc);

    // ---- Extract per-chroma-position 128-bit lanes ----
    let r0 = _mm256_extracti128_si256(r, 0);
    let r1 = _mm256_extracti128_si256(r, 1);
    let g0 = _mm256_extracti128_si256(g, 0);
    let g1 = _mm256_extracti128_si256(g, 1);
    let b0 = _mm256_extracti128_si256(b, 0);
    let b1 = _mm256_extracti128_si256(b, 1);

    // ---- Position c: clamp + pack + interleave + store ----
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
    let q_off = c * 8;
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off).cast::<__m128i>(), result0);
    let hi0 = _mm_unpackhi_epi64(result0, zero);
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off + w * 4).cast::<__m128i>(), hi0);

    // ---- Position c+1: clamp + pack + interleave + store ----
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
    let q_off1 = (c + 1) * 8;
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off1).cast::<__m128i>(), result1);
    let hi1 = _mm_unpackhi_epi64(result1, zero);
    _mm_storel_epi64(dst.as_mut_ptr().add(q_off1 + w * 4).cast::<__m128i>(), hi1);
}

// ---- YCbCr 4:2:0 row pair -> BGRA (AVX2, 16 px/iter) ----
/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn yuv420_row_pair_to_bgra_avx2(
    y_row: &[u8],
    cb_row: &[u8],
    cr_row: &[u8],
    dst: &mut [u8],
    w: usize,
    cb_w: usize,
) {
    use core::arch::x86_64::{_mm_set1_epi16, _mm_set1_epi32, _mm_setzero_si128};

    let max_val = _mm_set1_epi32(255);
    let zero = _mm_setzero_si128();
    let a16 = _mm_set1_epi16(255i16);
    let mut cx = 0usize;

    // Process 8 chroma positions (16 pixels = 2 rows x 8 cols) per outer iteration,
    // working in pairs (2 chroma positions at a time via 256-bit ops).
    while cx + 7 < cb_w {
        for q in 0..4 {
            store_avx2_chroma_pair(y_row, w, cx + q * 2, cb_row, cr_row, max_val, zero, a16, dst);
        }
        cx += 8;
    }

    // Remainder: handle remaining quads as pairs + possible single
    while cx + 1 < cb_w {
        for q in 0..2 {
            store_sse41_quad(y_row, w, cx + q, cb_row, cr_row, max_val, zero, a16, dst);
        }
        cx += 2;
    }
    if cx < cb_w {
        store_sse41_quad(y_row, w, cx, cb_row, cr_row, max_val, zero, a16, dst);
    }
}
