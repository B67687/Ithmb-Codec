//! YCbCr 4:2:0 row pair -> BGRA via SSE4.1.

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use core::arch::x86_64::__m128i;

// ---- SSE4.1 quad helper (used by sse41 and avx2 row functions) ----
/// SSE4.1: Process one chroma quad (4 Y bytes, 1 Cb, 1 Cr) -> 2 BGRA pixels
/// stored to `dst` at offset `c*8` (top row) and `c*8 + w*4` (bottom row).
///
/// # Safety
/// - Must be called on `x86`/`x86_64` with SSE4.1 enabled.
/// - `dst` must have sufficient capacity for the writes.
/// - `y_row`, `cb_row`, `cr_row` must have valid indices at position `c`.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[target_feature(enable = "sse4.1")]
#[allow(clippy::too_many_arguments, clippy::cast_sign_loss, unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn store_sse41_quad(
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
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
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
