//! YCbCr 4:2:0 row pair -> BGRA via AVX2 (16 px/iter).
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::__m128i;

/// AVX2: Process two chroma positions (8 Y bytes, 2 Cb, 2 Cr) -> 4 BGRA pixels
/// stored to `dst` at offsets `c*8`, `c*8 + w*4`, `(c+1)*8`, `(c+1)*8 + w*4`.
///
/// # Safety
/// - Must be called on `x86_64` with AVX2 enabled.
/// - `dst` must have sufficient capacity for the writes.
#[cfg(target_arch = "x86_64")]
#[inline]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments, unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn store_avx2_chroma_pair(
    y_row: &[u8],
    w: usize,
    c: usize,
    cb_row: &[u8],
    cr_row: &[u8],
    max_val: core::arch::x86_64::__m128i,
    zero: core::arch::x86_64::__m128i,
    a16: core::arch::x86_64::__m128i,
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
#[cfg(target_arch = "x86_64")]
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
            super::sse41::store_sse41_quad(y_row, w, cx + q, cb_row, cr_row, max_val, zero, a16, dst);
        }
        cx += 2;
    }
    if cx < cb_w {
        super::sse41::store_sse41_quad(y_row, w, cx, cb_row, cr_row, max_val, zero, a16, dst);
    }
}
