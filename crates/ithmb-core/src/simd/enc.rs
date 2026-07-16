//! SIMD-accelerated encoder functions — BGRA → all 7 ithmb pixel formats.
//!
//! SSE2 baseline (compile-time gate on `x86_64`/`x86`), AVX2 with runtime
//! dispatch via `is_x86_feature_detected!("avx2")`.
//!
//! Each section provides:
//! - An SSE2 chunk function (4 or 8 pixels per call)
//! - An AVX2 chunk function (8 or 16 pixels per call)
//! - A public dispatch function that picks the fastest path

#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use core::arch::x86_64::__m128i;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::__m256i;

// ===========================================================================
// Helper: SSSE3 deinterleave 4 BGRA pixels to planar i16 R/G/B
// ===========================================================================

/// Deinterleave 4 BGRA pixels (16 bytes) into R, G, B zero-extended to i16.
///
/// # Safety
///
/// Must only be called on `x86`/`x86_64` where SSSE3 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn deint_bgra4_to_i16(v: __m128i) -> (__m128i, __m128i, __m128i, __m128i) {
    use core::arch::x86_64::{_mm_set_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_unpacklo_epi8};
    let shuf_b = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 12, 8, 4, 0);
    let shuf_g = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 13, 9, 5, 1);
    let shuf_r = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 14, 10, 6, 2);

    let zero = _mm_setzero_si128();
    let b16 = _mm_unpacklo_epi8(_mm_shuffle_epi8(v, shuf_b), zero);
    let g16 = _mm_unpacklo_epi8(_mm_shuffle_epi8(v, shuf_g), zero);
    let r16 = _mm_unpacklo_epi8(_mm_shuffle_epi8(v, shuf_r), zero);
    (r16, g16, b16, _mm_setzero_si128())
}

// ===========================================================================
// Forward BT.601: RGB → Y, Cb, Cr  (SSE4.1 i32 arithmetic, 4 pixels)
// ===========================================================================

/// Compute Y, Cb, Cr for 4 BGRA pixels using SSE4.1 packed i32 arithmetic.
/// Returns (y16, cb16, cr16) — each is [v0,v1,v2,v3, 0,0,0,0] as i16.
///
/// # Safety
///
/// Must only be called where SSE4.1 is available.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn forward_bt601_4px_sse41(r16: __m128i, g16: __m128i, b16: __m128i) -> (__m128i, __m128i, __m128i) {
    use core::arch::x86_64::{
        _mm_add_epi32, _mm_cvtepu16_epi32, _mm_mullo_epi32, _mm_packus_epi32, _mm_set1_epi32, _mm_setzero_si128,
        _mm_srai_epi32,
    };

    let zero = _mm_setzero_si128();
    let r = _mm_cvtepu16_epi32(r16);
    let g = _mm_cvtepu16_epi32(g16);
    let b = _mm_cvtepu16_epi32(b16);

    let c77 = _mm_set1_epi32(77);
    let c150 = _mm_set1_epi32(150);
    let c29 = _mm_set1_epi32(29);
    let cn43 = _mm_set1_epi32(-43);
    let cn85 = _mm_set1_epi32(-85);
    let c128 = _mm_set1_epi32(128);
    let cn107 = _mm_set1_epi32(-107);
    let cn21 = _mm_set1_epi32(-21);

    // Y  = (77*R + 150*G +  29*B) >> 8
    let y = _mm_srai_epi32(
        _mm_add_epi32(
            _mm_add_epi32(_mm_mullo_epi32(r, c77), _mm_mullo_epi32(g, c150)),
            _mm_mullo_epi32(b, c29),
        ),
        8,
    );
    // Cb = ((-43*R - 85*G + 128*B) >> 8) + 128
    let cb = _mm_add_epi32(
        _mm_srai_epi32(
            _mm_add_epi32(
                _mm_add_epi32(_mm_mullo_epi32(r, cn43), _mm_mullo_epi32(g, cn85)),
                _mm_mullo_epi32(b, c128),
            ),
            8,
        ),
        c128,
    );
    // Cr = ((128*R - 107*G - 21*B) >> 8) + 128
    let cr = _mm_add_epi32(
        _mm_srai_epi32(
            _mm_add_epi32(
                _mm_add_epi32(_mm_mullo_epi32(r, c128), _mm_mullo_epi32(g, cn107)),
                _mm_mullo_epi32(b, cn21),
            ),
            8,
        ),
        c128,
    );

    (
        _mm_packus_epi32(y, zero),
        _mm_packus_epi32(cb, zero),
        _mm_packus_epi32(cr, zero),
    )
}

// ===========================================================================
// 1. RGB565 encode — BGRA → RGB565 (2 bytes per pixel)
// ===========================================================================

/// Encode 4 BGRA pixels (16 bytes) to 4 RGB565 pixels (8 bytes, LE).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_rgb565_4px_ssse3(bgra_quad: &[u8; 16]) -> [u8; 8] {
    use core::arch::x86_64::{_mm_loadu_si128, _mm_or_si128, _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128};
    let v = _mm_loadu_si128(bgra_quad.as_ptr().cast::<__m128i>());
    let (r16, g16, b16, _) = deint_bgra4_to_i16(v);

    // R5 = R >> 3, G6 = G >> 2, B5 = B >> 3
    // pixel = (R5 << 11) | (G6 << 5) | B5
    let pixel = _mm_or_si128(
        _mm_or_si128(
            _mm_slli_epi16(_mm_srli_epi16(r16, 3), 11),
            _mm_slli_epi16(_mm_srli_epi16(g16, 2), 5),
        ),
        _mm_srli_epi16(b16, 3),
    );

    // Store 128-bit vector as bytes; low 8 bytes = 4 LE u16 pixel values
    let mut tmp = [0u8; 16];
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), pixel);
    let mut out = [0u8; 8];
    out.copy_from_slice(&tmp[..8]);
    out
}

/// Encode 8 BGRA pixels (32 bytes) to 8 RGB565 pixels (16 bytes, LE).
#[cfg(target_arch = "x86_64")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_rgb565_8px_avx2(bgra_oct: &[u8; 32]) -> [u8; 16] {
    use core::arch::x86_64::{
        _mm256_extracti128_si256, _mm256_loadu_si256, _mm_or_si128, _mm_set_epi8, _mm_setzero_si128, _mm_shuffle_epi8,
        _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128, _mm_unpacklo_epi8,
    };

    let v = _mm256_loadu_si256(bgra_oct.as_ptr().cast::<__m256i>());
    let lo = _mm256_extracti128_si256(v, 0);
    let hi = _mm256_extracti128_si256(v, 1);

    let shuf_r = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 14, 10, 6, 2);
    let shuf_g = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 13, 9, 5, 1);
    let shuf_b = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 12, 8, 4, 0);
    let z = _mm_setzero_si128();

    let decode_half = |half: __m128i| -> __m128i {
        let r = _mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_r), z);
        let g = _mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_g), z);
        let b = _mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_b), z);
        _mm_or_si128(
            _mm_or_si128(
                _mm_slli_epi16(_mm_srli_epi16(r, 3), 11),
                _mm_slli_epi16(_mm_srli_epi16(g, 2), 5),
            ),
            _mm_srli_epi16(b, 3),
        )
    };

    let p_lo = decode_half(lo);
    let p_hi = decode_half(hi);
    // Store 4 LE u16 values from each half into 16 output bytes
    let mut out = [0u8; 16];
    let mut tmp = [0u8; 16];
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), p_lo);
    out[..8].copy_from_slice(&tmp[..8]);
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), p_hi);
    out[8..].copy_from_slice(&tmp[..8]);
    out
}

/// Process RGB565 encoding in blocks (LE only, SSSE3 path).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn encode_rgb565_ssse3(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 4 <= n_pixels {
        let quad: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let result = unsafe { encode_rgb565_4px_ssse3(quad) };
        out[i * 2..i * 2 + 8].copy_from_slice(&result);
        i += 4;
    }
    // Tail
    for j in i..n_pixels {
        let px = j * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        let pixel = (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16;
        let bytes = pixel.to_le_bytes();
        let o = j * 2;
        out[o] = bytes[0];
        out[o + 1] = bytes[1];
    }
}

/// Dispatch: encode BGRA to RGB565 with runtime SIMD selection.
fn encode_rgb565_impl(bgra: &[u8], out: &mut [u8], big_endian: bool) {
    let n_pixels = bgra.len() / 4;
    debug_assert_eq!(out.len(), n_pixels * 2);

    if !big_endian {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            return encode_rgb565_avx2(bgra, out);
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        if is_x86_feature_detected!("ssse3") {
            return encode_rgb565_ssse3(bgra, out);
        }
    }

    // Scalar fallback
    for i in 0..n_pixels {
        let px = i * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        let pixel = (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16;
        let bytes = if big_endian {
            pixel.to_be_bytes()
        } else {
            pixel.to_le_bytes()
        };
        let o = i * 2;
        out[o] = bytes[0];
        out[o + 1] = bytes[1];
    }
}

#[cfg(target_arch = "x86_64")]
fn encode_rgb565_avx2(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 8 <= n_pixels {
        let oct: &[u8; 32] = (&bgra[i * 4..][..32]).try_into().unwrap();
        let result = unsafe { encode_rgb565_8px_avx2(oct) };
        out[i * 2..i * 2 + 16].copy_from_slice(&result);
        i += 8;
    }
    if i < n_pixels {
        encode_rgb565_ssse3(&bgra[i * 4..], &mut out[i * 2..]);
    }
}

// ===========================================================================
// 2. RGB555 encode — BGRA → RGB555 (2 bytes per pixel)
// ===========================================================================

/// Encode 4 BGRA pixels to 4 RGB555 LE (SSSE3).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_rgb555_4px_ssse3(bgra_quad: &[u8; 16], swap_rgb: bool) -> [u8; 8] {
    use core::arch::x86_64::{_mm_loadu_si128, _mm_or_si128, _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128};
    let v = _mm_loadu_si128(bgra_quad.as_ptr().cast::<__m128i>());
    let (r16, g16, b16, _) = deint_bgra4_to_i16(v);

    let r5 = _mm_srli_epi16(r16, 3);
    let g5 = _mm_srli_epi16(g16, 3);
    let b5 = _mm_srli_epi16(b16, 3);

    let pixel = if swap_rgb {
        // BGR15: (B5 << 10) | (G5 << 5) | R5
        _mm_or_si128(_mm_or_si128(_mm_slli_epi16(b5, 10), _mm_slli_epi16(g5, 5)), r5)
    } else {
        // RGB555: (R5 << 10) | (G5 << 5) | B5
        _mm_or_si128(_mm_or_si128(_mm_slli_epi16(r5, 10), _mm_slli_epi16(g5, 5)), b5)
    };

    let mut tmp = [0u8; 16];
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), pixel);
    let mut out = [0u8; 8];
    out.copy_from_slice(&tmp[..8]);
    out
}

/// Encode 8 BGRA pixels to 8 RGB555 LE (AVX2).
#[cfg(target_arch = "x86_64")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_rgb555_8px_avx2(bgra_oct: &[u8; 32], swap_rgb: bool) -> [u8; 16] {
    use core::arch::x86_64::{
        _mm256_extracti128_si256, _mm256_loadu_si256, _mm_or_si128, _mm_set_epi8, _mm_setzero_si128, _mm_shuffle_epi8,
        _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128, _mm_unpacklo_epi8,
    };

    let v = _mm256_loadu_si256(bgra_oct.as_ptr().cast::<__m256i>());
    let lo = _mm256_extracti128_si256(v, 0);
    let hi = _mm256_extracti128_si256(v, 1);

    let shuf_r = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 14, 10, 6, 2);
    let shuf_g = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 13, 9, 5, 1);
    let shuf_b = _mm_set_epi8(-1i8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 12, 8, 4, 0);
    let z = _mm_setzero_si128();

    let decode_half = |half: __m128i, swp: bool| -> __m128i {
        let r = _mm_srli_epi16(_mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_r), z), 3);
        let g = _mm_srli_epi16(_mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_g), z), 3);
        let b = _mm_srli_epi16(_mm_unpacklo_epi8(_mm_shuffle_epi8(half, shuf_b), z), 3);
        if swp {
            _mm_or_si128(_mm_or_si128(_mm_slli_epi16(b, 10), _mm_slli_epi16(g, 5)), r)
        } else {
            _mm_or_si128(_mm_or_si128(_mm_slli_epi16(r, 10), _mm_slli_epi16(g, 5)), b)
        }
    };

    let p_lo = decode_half(lo, swap_rgb);
    let p_hi = decode_half(hi, swap_rgb);
    let mut out = [0u8; 16];
    let mut tmp = [0u8; 16];
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), p_lo);
    out[..8].copy_from_slice(&tmp[..8]);
    _mm_storeu_si128(tmp.as_mut_ptr().cast::<__m128i>(), p_hi);
    out[8..].copy_from_slice(&tmp[..8]);
    out
}

/// SSSE3 block path for RGB555 LE.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn encode_rgb555_ssse3(bgra: &[u8], out: &mut [u8], swap_rgb: bool) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 4 <= n_pixels {
        let quad: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let result = unsafe { encode_rgb555_4px_ssse3(quad, swap_rgb) };
        out[i * 2..i * 2 + 8].copy_from_slice(&result);
        i += 4;
    }
    for j in i..n_pixels {
        let px = j * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        let r5 = r >> 3;
        let g5 = g >> 3;
        let b5 = b >> 3;
        let pixel: u16 = if swap_rgb {
            ((b5 << 10) | (g5 << 5) | r5) as u16
        } else {
            ((r5 << 10) | (g5 << 5) | b5) as u16
        };
        let bytes = pixel.to_le_bytes();
        let o = j * 2;
        out[o] = bytes[0];
        out[o + 1] = bytes[1];
    }
}

/// Dispatch: encode BGRA to RGB555.
fn encode_rgb555_impl(bgra: &[u8], out: &mut [u8], big_endian: bool, swap_rgb: bool) {
    let n_pixels = bgra.len() / 4;
    debug_assert_eq!(out.len(), n_pixels * 2);

    if !big_endian {
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            return encode_rgb555_avx2(bgra, out, swap_rgb);
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        if is_x86_feature_detected!("ssse3") {
            return encode_rgb555_ssse3(bgra, out, swap_rgb);
        }
    }

    for i in 0..n_pixels {
        let px = i * 4;
        let b = u32::from(bgra[px]);
        let g = u32::from(bgra[px + 1]);
        let r = u32::from(bgra[px + 2]);
        let r5 = r >> 3;
        let g5 = g >> 3;
        let b5 = b >> 3;
        let pixel: u16 = if swap_rgb {
            ((b5 << 10) | (g5 << 5) | r5) as u16
        } else {
            ((r5 << 10) | (g5 << 5) | b5) as u16
        };
        let bytes = if big_endian {
            pixel.to_be_bytes()
        } else {
            pixel.to_le_bytes()
        };
        let o = i * 2;
        out[o] = bytes[0];
        out[o + 1] = bytes[1];
    }
}

#[cfg(target_arch = "x86_64")]
fn encode_rgb555_avx2(bgra: &[u8], out: &mut [u8], swap_rgb: bool) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 8 <= n_pixels {
        let oct: &[u8; 32] = (&bgra[i * 4..][..32]).try_into().unwrap();
        let result = unsafe { encode_rgb555_8px_avx2(oct, swap_rgb) };
        out[i * 2..i * 2 + 16].copy_from_slice(&result);
        i += 8;
    }
    if i < n_pixels {
        encode_rgb555_ssse3(&bgra[i * 4..], &mut out[i * 2..], swap_rgb);
    }
}

// ===========================================================================
// 3. UYVY encode — BT.601 forward, chroma-average pairs
// ===========================================================================

/// Encode 4 BGRA pixels to 2 UYVY pairs (SSE4.1).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_uyvy_4px_sse41(bgra_quad: &[u8; 16]) -> [u8; 8] {
    use core::arch::x86_64::{_mm_loadu_si128, _mm_packus_epi16, _mm_setzero_si128, _mm_storeu_si128};

    let v = _mm_loadu_si128(bgra_quad.as_ptr().cast::<__m128i>());
    let (r16, g16, b16, _) = deint_bgra4_to_i16(v);
    let (y16, cb16, cr16) = forward_bt601_4px_sse41(r16, g16, b16);

    // Pack to bytes and extract individual values for chroma averaging
    let y_bytes = _mm_packus_epi16(y16, _mm_setzero_si128());
    let cb_bytes = _mm_packus_epi16(cb16, _mm_setzero_si128());
    let cr_bytes = _mm_packus_epi16(cr16, _mm_setzero_si128());

    let mut y_arr = [0u8; 16];
    _mm_storeu_si128(y_arr.as_mut_ptr().cast::<__m128i>(), y_bytes);
    let mut cb_arr = [0u8; 16];
    _mm_storeu_si128(cb_arr.as_mut_ptr().cast::<__m128i>(), cb_bytes);
    let mut cr_arr = [0u8; 16];
    _mm_storeu_si128(cr_arr.as_mut_ptr().cast::<__m128i>(), cr_bytes);

    let cb_avg0 = ((cb_arr[0] as u16 + cb_arr[1] as u16 + 1) >> 1) as u8;
    let cr_avg0 = ((cr_arr[0] as u16 + cr_arr[1] as u16 + 1) >> 1) as u8;
    let cb_avg1 = ((cb_arr[2] as u16 + cb_arr[3] as u16 + 1) >> 1) as u8;
    let cr_avg1 = ((cr_arr[2] as u16 + cr_arr[3] as u16 + 1) >> 1) as u8;

    [
        cb_avg0, y_arr[0], cr_avg0, y_arr[1], cb_avg1, y_arr[2], cr_avg1, y_arr[3],
    ]
}

/// Dispatch: encode BGRA to UYVY.
fn encode_uyvy_impl(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    // UYVY output: ceil(w/2) pairs per row, 4 bytes each.
    // Without knowing w, the minimum is ceil(n_pixels/2) * 4.
    debug_assert!(out.len() >= n_pixels.div_ceil(2) * 4);

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("sse4.1") {
        return encode_uyvy_avx2(bgra, out);
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    if is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("ssse3") {
        return encode_uyvy_sse41(bgra, out);
    }

    let n = n_pixels;
    let mut px_i = 0;
    let mut o_i = 0;
    while px_i < n {
        let px = px_i * 4;
        let r0 = i32::from(bgra[px + 2]);
        let g0 = i32::from(bgra[px + 1]);
        let b0 = i32::from(bgra[px]);
        let y0 = clamp_u8(bt601_y(r0, g0, b0));
        let cb0 = bt601_cb(r0, g0, b0);
        let cr0 = bt601_cr(r0, g0, b0);

        if px_i + 1 < n {
            let px2 = (px_i + 1) * 4;
            let r1 = i32::from(bgra[px2 + 2]);
            let g1 = i32::from(bgra[px2 + 1]);
            let b1 = i32::from(bgra[px2]);
            let y1 = clamp_u8(bt601_y(r1, g1, b1));
            let cb1 = bt601_cb(r1, g1, b1);
            let cr1 = bt601_cr(r1, g1, b1);
            out[o_i] = clamp_u8((cb0 + cb1 + 1) >> 1);
            out[o_i + 1] = y0;
            out[o_i + 2] = clamp_u8((cr0 + cr1 + 1) >> 1);
            out[o_i + 3] = y1;
        } else {
            out[o_i] = clamp_u8(cb0);
            out[o_i + 1] = y0;
            out[o_i + 2] = clamp_u8(cr0);
            out[o_i + 3] = 0;
        }
        px_i += 2;
        o_i += 4;
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn encode_uyvy_sse41(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 4 <= n_pixels {
        let quad: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let result = unsafe { encode_uyvy_4px_sse41(quad) };
        let o = (i / 2) * 4;
        out[o..o + 8].copy_from_slice(&result);
        i += 4;
    }
    // Remainder pixels (process remaining pairs+single)
    while i + 1 < n_pixels {
        let rem = &bgra[i * 4..];
        let r0 = i32::from(rem[2]);
        let g0 = i32::from(rem[1]);
        let b0 = i32::from(rem[0]);
        let y0 = clamp_u8(bt601_y(r0, g0, b0));
        let cb0 = bt601_cb(r0, g0, b0);
        let cr0 = bt601_cr(r0, g0, b0);
        let r1 = i32::from(rem[6]);
        let g1 = i32::from(rem[5]);
        let b1 = i32::from(rem[4]);
        let y1 = clamp_u8(bt601_y(r1, g1, b1));
        let cb1 = bt601_cb(r1, g1, b1);
        let cr1 = bt601_cr(r1, g1, b1);
        let o = (i / 2) * 4;
        out[o] = clamp_u8((cb0 + cb1 + 1) >> 1);
        out[o + 1] = y0;
        out[o + 2] = clamp_u8((cr0 + cr1 + 1) >> 1);
        out[o + 3] = y1;
        i += 2;
    }
    if i < n_pixels {
        let rem = &bgra[i * 4..];
        let r0 = i32::from(rem[2]);
        let g0 = i32::from(rem[1]);
        let b0 = i32::from(rem[0]);
        let o = (i / 2) * 4;
        out[o] = clamp_u8(bt601_cb(r0, g0, b0));
        out[o + 1] = clamp_u8(bt601_y(r0, g0, b0));
        out[o + 2] = clamp_u8(bt601_cr(r0, g0, b0));
        out[o + 3] = 0;
    }
}

#[cfg(target_arch = "x86_64")]
fn encode_uyvy_avx2(bgra: &[u8], out: &mut [u8]) {
    // AVX2 processes 8 pixels per iteration (calls SSE41 helper twice)
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 8 <= n_pixels {
        let lo: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let hi: &[u8; 16] = (&bgra[i * 4 + 16..][..16]).try_into().unwrap();
        let r_lo = unsafe { encode_uyvy_4px_sse41(lo) };
        let r_hi = unsafe { encode_uyvy_4px_sse41(hi) };
        let o = (i / 2) * 4;
        out[o..o + 8].copy_from_slice(&r_lo);
        out[o + 8..o + 16].copy_from_slice(&r_hi);
        i += 8;
    }
    if i < n_pixels {
        encode_uyvy_sse41(&bgra[i * 4..], &mut out[(i / 2) * 4..]);
    }
}

// ===========================================================================
// 4. Y plane for YCbCr 4:2:0 / CLCL — BT.601 Y for each pixel
// ===========================================================================

/// Dispatch: compute Y plane.
fn encode_y_plane_impl(bgra: &[u8], y_plane: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    debug_assert_eq!(y_plane.len(), n_pixels);

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("sse4.1") {
        return encode_y_plane_avx2(bgra, y_plane);
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    if is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("ssse3") {
        return encode_y_plane_sse41(bgra, y_plane);
    }

    for (i, px) in bgra.chunks_exact(4).enumerate().take(n_pixels) {
        y_plane[i] = clamp_u8(bt601_y(i32::from(px[2]), i32::from(px[1]), i32::from(px[0])));
    }
}

/// SSE4.1: compute Y plane, 4 pixels at a time.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn encode_y_plane_sse41(bgra: &[u8], y_plane: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 4 <= n_pixels {
        let quad: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let v = unsafe { core::arch::x86_64::_mm_loadu_si128(quad.as_ptr().cast::<__m128i>()) };
        let (r16, g16, b16, _) = unsafe { deint_bgra4_to_i16(v) };
        let (y16, _, _) = unsafe { forward_bt601_4px_sse41(r16, g16, b16) };
        let yb = unsafe { core::arch::x86_64::_mm_packus_epi16(y16, core::arch::x86_64::_mm_setzero_si128()) };
        let mut arr = [0u8; 16];
        unsafe {
            core::arch::x86_64::_mm_storeu_si128(arr.as_mut_ptr().cast::<__m128i>(), yb);
        }
        y_plane[i..i + 4].copy_from_slice(&arr[..4]);
        i += 4;
    }
    for (offset, px) in bgra[i * 4..].chunks_exact(4).enumerate().take(n_pixels - i) {
        y_plane[i + offset] = clamp_u8(bt601_y(i32::from(px[2]), i32::from(px[1]), i32::from(px[0])));
    }
}

/// AVX2: compute Y plane, 8 pixels at a time.
#[cfg(target_arch = "x86_64")]
fn encode_y_plane_avx2(bgra: &[u8], y_plane: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 8 <= n_pixels {
        let lo: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let hi: &[u8; 16] = (&bgra[i * 4 + 16..][..16]).try_into().unwrap();
        let v_lo = unsafe { core::arch::x86_64::_mm_loadu_si128(lo.as_ptr().cast::<__m128i>()) };
        let v_hi = unsafe { core::arch::x86_64::_mm_loadu_si128(hi.as_ptr().cast::<__m128i>()) };
        let (r_lo, g_lo, b_lo, _) = unsafe { deint_bgra4_to_i16(v_lo) };
        let (r_hi, g_hi, b_hi, _) = unsafe { deint_bgra4_to_i16(v_hi) };
        let (y_lo, _, _) = unsafe { forward_bt601_4px_sse41(r_lo, g_lo, b_lo) };
        let (y_hi, _, _) = unsafe { forward_bt601_4px_sse41(r_hi, g_hi, b_hi) };
        let yb_lo = unsafe { core::arch::x86_64::_mm_packus_epi16(y_lo, core::arch::x86_64::_mm_setzero_si128()) };
        let yb_hi = unsafe { core::arch::x86_64::_mm_packus_epi16(y_hi, core::arch::x86_64::_mm_setzero_si128()) };
        let mut arr = [0u8; 16];
        unsafe {
            core::arch::x86_64::_mm_storeu_si128(arr.as_mut_ptr().cast::<__m128i>(), yb_lo);
        }
        y_plane[i..i + 4].copy_from_slice(&arr[..4]);
        unsafe {
            core::arch::x86_64::_mm_storeu_si128(arr.as_mut_ptr().cast::<__m128i>(), yb_hi);
        }
        y_plane[i + 4..i + 8].copy_from_slice(&arr[..4]);
        i += 8;
    }
    if i < n_pixels {
        encode_y_plane_sse41(&bgra[i * 4..], &mut y_plane[i..]);
    }
}

// ===========================================================================
// 5. CL encode — Y + nibble CbCr (2 bytes per pixel)
// ===========================================================================

/// Encode 4 BGRA pixels to CL (SSE4.1).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn encode_cl_4px_sse41(bgra_quad: &[u8; 16]) -> [u8; 8] {
    use core::arch::x86_64::{
        _mm_loadu_si128, _mm_or_si128, _mm_packus_epi16, _mm_setzero_si128, _mm_slli_epi16, _mm_srli_epi16,
        _mm_storeu_si128,
    };

    let v = _mm_loadu_si128(bgra_quad.as_ptr().cast::<__m128i>());
    let (r16, g16, b16, _) = deint_bgra4_to_i16(v);
    let (y16, cb16, cr16) = forward_bt601_4px_sse41(r16, g16, b16);

    // Y bytes
    let y_bytes = _mm_packus_epi16(y16, _mm_setzero_si128());

    // CbCr nibble: (Cr>>4)<<4 | (Cb>>4)
    let cb_nib = _mm_srli_epi16(cb16, 4); // Cb>>4 in low nibble position
    let cr_nib = _mm_slli_epi16(_mm_srli_epi16(cr16, 4), 4); // Cr>>4 in high nibble
    let chroma = _mm_or_si128(cb_nib, cr_nib);
    let c_bytes = _mm_packus_epi16(chroma, _mm_setzero_si128());

    let mut y_arr = [0u8; 16];
    let mut c_arr = [0u8; 16];
    _mm_storeu_si128(y_arr.as_mut_ptr().cast::<__m128i>(), y_bytes);
    _mm_storeu_si128(c_arr.as_mut_ptr().cast::<__m128i>(), c_bytes);

    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&y_arr[..4]);
    out[4..].copy_from_slice(&c_arr[..4]);
    out
}

/// Dispatch: encode BGRA to CL format.
fn encode_cl_impl(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    debug_assert_eq!(out.len(), n_pixels * 2);

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("sse4.1") {
        return encode_cl_avx2(bgra, out);
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    if is_x86_feature_detected!("sse4.1") && is_x86_feature_detected!("ssse3") {
        return encode_cl_sse41(bgra, out);
    }

    for i in 0..n_pixels {
        let px = i * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);
        out[i] = clamp_u8(bt601_y(r, g, b));
        let cb_n = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
        let cr_n = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
        out[n_pixels + i] = (cr_n << 4) | cb_n;
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
fn encode_cl_sse41(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    // Chroma offset is always halfway through the output buffer
    let chroma_base = out.len() / 2;
    let mut i = 0usize;
    while i + 4 <= n_pixels {
        let quad: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let result = unsafe { encode_cl_4px_sse41(quad) };
        out[i..i + 4].copy_from_slice(&result[..4]);
        out[chroma_base + i..chroma_base + i + 4].copy_from_slice(&result[4..]);
        i += 4;
    }
    for j in i..n_pixels {
        let px = j * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);
        out[j] = clamp_u8(bt601_y(r, g, b));
        let cb_n = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
        let cr_n = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
        out[chroma_base + j] = (cr_n << 4) | cb_n;
    }
}

#[cfg(target_arch = "x86_64")]
fn encode_cl_avx2(bgra: &[u8], out: &mut [u8]) {
    let n_pixels = bgra.len() / 4;
    let mut i = 0usize;
    while i + 8 <= n_pixels {
        let lo: &[u8; 16] = (&bgra[i * 4..][..16]).try_into().unwrap();
        let hi: &[u8; 16] = (&bgra[i * 4 + 16..][..16]).try_into().unwrap();
        let r_lo = unsafe { encode_cl_4px_sse41(lo) };
        let r_hi = unsafe { encode_cl_4px_sse41(hi) };
        out[i..i + 4].copy_from_slice(&r_lo[..4]);
        out[i + 4..i + 8].copy_from_slice(&r_hi[..4]);
        out[n_pixels + i..n_pixels + i + 4].copy_from_slice(&r_lo[4..]);
        out[n_pixels + i + 4..n_pixels + i + 8].copy_from_slice(&r_hi[4..]);
        i += 8;
    }
    if i < n_pixels {
        let rem = &bgra[i * 4..];
        for j in 0..(n_pixels - i) {
            let px = j * 4;
            let r = i32::from(rem[px + 2]);
            let g = i32::from(rem[px + 1]);
            let b = i32::from(rem[px]);
            let out_idx = i + j;
            out[out_idx] = clamp_u8(bt601_y(r, g, b));
            let cb_n = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
            let cr_n = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
            out[n_pixels + out_idx] = (cr_n << 4) | cb_n;
        }
    }
}

// ===========================================================================
// 6. CLCL — same as CL but with separate Cb/Cr nibble planes
// ===========================================================================

#[allow(dead_code)]
/// Compute nibble chroma plane for CLCL (Cb or Cr).
fn encode_clcl_c_plane_impl(bgra: &[u8], c_plane: &mut [u8], is_cb: bool) {
    let n_pixels = bgra.len() / 4;
    let out_len = n_pixels.div_ceil(2);
    debug_assert_eq!(c_plane.len(), out_len);

    for i in 0..n_pixels {
        let px = i * 4;
        let r = i32::from(bgra[px + 2]);
        let g = i32::from(bgra[px + 1]);
        let b = i32::from(bgra[px]);
        let nibble = if is_cb {
            (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F
        } else {
            (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F
        };
        let ci = i / 2;
        if i & 1 == 0 {
            c_plane[ci] = nibble;
        } else {
            c_plane[ci] |= nibble << 4;
        }
    }
}

// ===========================================================================
// Helper functions used in scalar fallbacks
// ===========================================================================

#[inline]
fn bt601_y(r: i32, g: i32, b: i32) -> i32 {
    (77 * r + 150 * g + 29 * b) >> 8
}

#[inline]
fn bt601_cb(r: i32, g: i32, b: i32) -> i32 {
    ((-43 * r - 85 * g + 128 * b) >> 8) + 128
}

#[inline]
fn bt601_cr(r: i32, g: i32, b: i32) -> i32 {
    ((128 * r - 107 * g - 21 * b) >> 8) + 128
}

#[inline]
fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

// ===========================================================================
// Public dispatch functions
// ===========================================================================

pub(crate) fn bgra_to_rgb565(bgra: &[u8], out: &mut [u8], big_endian: bool) {
    encode_rgb565_impl(bgra, out, big_endian);
}

pub(crate) fn bgra_to_rgb555(bgra: &[u8], out: &mut [u8], big_endian: bool, swap_rgb: bool) {
    encode_rgb555_impl(bgra, out, big_endian, swap_rgb);
}

pub(crate) fn bgra_to_uyvy(bgra: &[u8], out: &mut [u8]) {
    encode_uyvy_impl(bgra, out);
}

pub(crate) fn bgra_to_y_plane(bgra: &[u8], y_plane: &mut [u8]) {
    encode_y_plane_impl(bgra, y_plane);
}

pub(crate) fn bgra_to_cl(bgra: &[u8], out: &mut [u8]) {
    encode_cl_impl(bgra, out);
}

#[allow(dead_code)]
pub(crate) fn bgra_to_clcl_y(bgra: &[u8], y_plane: &mut [u8]) {
    encode_y_plane_impl(bgra, y_plane);
}

#[allow(dead_code)]
pub(crate) fn bgra_to_clcl_c(bgra: &[u8], c_plane: &mut [u8], is_cb: bool) {
    encode_clcl_c_plane_impl(bgra, c_plane, is_cb);
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn gen_test_bgra(n_pixels: usize) -> Vec<u8> {
        let mut bgra = Vec::with_capacity(n_pixels * 4);
        let mut state: u32 = 0xABCD_0001;
        for _ in 0..n_pixels {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            bgra.push((state >> 16) as u8); // B
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            bgra.push((state >> 16) as u8); // G
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            bgra.push((state >> 16) as u8); // R
            bgra.push(255); // A
        }
        bgra
    }

    fn scalar_rgb565(bgra: &[u8]) -> Vec<u8> {
        let n = bgra.len() / 4;
        let mut out = vec![0u8; n * 2];
        for i in 0..n {
            let px = i * 4;
            let b = u32::from(bgra[px]);
            let g = u32::from(bgra[px + 1]);
            let r = u32::from(bgra[px + 2]);
            let pixel = (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16;
            let bytes = pixel.to_le_bytes();
            let o = i * 2;
            out[o] = bytes[0];
            out[o + 1] = bytes[1];
        }
        out
    }

    fn scalar_rgb555(bgra: &[u8], swap_rgb: bool) -> Vec<u8> {
        let n = bgra.len() / 4;
        let mut out = vec![0u8; n * 2];
        for i in 0..n {
            let px = i * 4;
            let b = u32::from(bgra[px]);
            let g = u32::from(bgra[px + 1]);
            let r = u32::from(bgra[px + 2]);
            let r5 = r >> 3;
            let g5 = g >> 3;
            let b5 = b >> 3;
            let pixel: u16 = if swap_rgb {
                ((b5 << 10) | (g5 << 5) | r5) as u16
            } else {
                ((r5 << 10) | (g5 << 5) | b5) as u16
            };
            let o = i * 2;
            let bytes = pixel.to_le_bytes();
            out[o] = bytes[0];
            out[o + 1] = bytes[1];
        }
        out
    }

    fn scalar_uyvy(bgra: &[u8]) -> Vec<u8> {
        let n_pixels = bgra.len() / 4;
        let pairs = n_pixels.div_ceil(2);
        let mut out = vec![0u8; pairs * 4];
        let mut px_i = 0;
        let mut o_i = 0;
        while px_i < n_pixels {
            let px = px_i * 4;
            let r0 = i32::from(bgra[px + 2]);
            let g0 = i32::from(bgra[px + 1]);
            let b0 = i32::from(bgra[px]);
            let y0 = clamp_u8(bt601_y(r0, g0, b0));
            let cb0 = bt601_cb(r0, g0, b0);
            let cr0 = bt601_cr(r0, g0, b0);
            if px_i + 1 < n_pixels {
                let px2 = (px_i + 1) * 4;
                let r1 = i32::from(bgra[px2 + 2]);
                let g1 = i32::from(bgra[px2 + 1]);
                let b1 = i32::from(bgra[px2]);
                let y1 = clamp_u8(bt601_y(r1, g1, b1));
                let cb1 = bt601_cb(r1, g1, b1);
                let cr1 = bt601_cr(r1, g1, b1);
                out[o_i] = clamp_u8((cb0 + cb1 + 1) >> 1);
                out[o_i + 1] = y0;
                out[o_i + 2] = clamp_u8((cr0 + cr1 + 1) >> 1);
                out[o_i + 3] = y1;
            } else {
                out[o_i] = clamp_u8(cb0);
                out[o_i + 1] = y0;
                out[o_i + 2] = clamp_u8(cr0);
                out[o_i + 3] = 0;
            }
            px_i += 2;
            o_i += 4;
        }
        out
    }

    fn scalar_cl(bgra: &[u8]) -> Vec<u8> {
        let n = bgra.len() / 4;
        let mut out = vec![0u8; n * 2];
        for i in 0..n {
            let px = i * 4;
            let r = i32::from(bgra[px + 2]);
            let g = i32::from(bgra[px + 1]);
            let b = i32::from(bgra[px]);
            out[i] = clamp_u8(bt601_y(r, g, b));
            let cb_n = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
            let cr_n = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
            out[n + i] = (cr_n << 4) | cb_n;
        }
        out
    }

    // ---- RGB565 tests ----

    #[test]
    fn test_rgb565_small() {
        let bgra = gen_test_bgra(4);
        let mut out = vec![0u8; 8];
        bgra_to_rgb565(&bgra, &mut out, false);
        assert_eq!(out, scalar_rgb565(&bgra), "RGB565 4px");
    }

    #[test]
    fn test_rgb565_various_sizes() {
        for &n in &[1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17] {
            let bgra = gen_test_bgra(n);
            let mut out = vec![0u8; n * 2];
            bgra_to_rgb565(&bgra, &mut out, false);
            assert_eq!(out, scalar_rgb565(&bgra), "RGB565 n={n}");
        }
    }

    #[test]
    fn test_rgb565_1000_random() {
        let bgra = gen_test_bgra(1000);
        let mut out = vec![0u8; 2000];
        bgra_to_rgb565(&bgra, &mut out, false);
        assert_eq!(out, scalar_rgb565(&bgra), "RGB565 1000px");
    }

    // ---- RGB555 tests ----

    #[test]
    fn test_rgb555_small() {
        let bgra = gen_test_bgra(4);
        let mut out = vec![0u8; 8];
        bgra_to_rgb555(&bgra, &mut out, false, false);
        assert_eq!(out, scalar_rgb555(&bgra, false), "RGB555 4px");
    }

    #[test]
    fn test_rgb555_swap() {
        let bgra = gen_test_bgra(4);
        let mut out = vec![0u8; 8];
        bgra_to_rgb555(&bgra, &mut out, false, true);
        assert_eq!(out, scalar_rgb555(&bgra, true), "RGB555 swap");
    }

    #[test]
    fn test_rgb555_various_sizes() {
        for &n in &[1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17] {
            for &swap in &[false, true] {
                let bgra = gen_test_bgra(n);
                let mut out = vec![0u8; n * 2];
                bgra_to_rgb555(&bgra, &mut out, false, swap);
                assert_eq!(out, scalar_rgb555(&bgra, swap), "RGB555 n={n} swap={swap}");
            }
        }
    }

    #[test]
    fn test_rgb555_1000_random() {
        for &swap in &[false, true] {
            let bgra = gen_test_bgra(1000);
            let mut out = vec![0u8; 2000];
            bgra_to_rgb555(&bgra, &mut out, false, swap);
            assert_eq!(out, scalar_rgb555(&bgra, swap), "RGB555 1000px swap={swap}");
        }
    }

    // ---- UYVY tests ----

    #[test]
    fn test_uyvy_small() {
        let bgra = gen_test_bgra(4);
        let pairs = 4usize.div_ceil(2);
        let mut out = vec![0u8; pairs * 4];
        bgra_to_uyvy(&bgra, &mut out);
        assert_eq!(out, scalar_uyvy(&bgra), "UYVY 4px");
    }

    #[test]
    fn test_uyvy_odd() {
        let bgra = gen_test_bgra(5);
        let pairs = 5usize.div_ceil(2);
        let mut out = vec![0u8; pairs * 4];
        bgra_to_uyvy(&bgra, &mut out);
        assert_eq!(out, scalar_uyvy(&bgra), "UYVY 5px");
    }

    #[test]
    fn test_uyvy_various_sizes() {
        for &n in &[1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17] {
            let bgra = gen_test_bgra(n);
            let pairs = n.div_ceil(2);
            let mut out = vec![0u8; pairs * 4];
            bgra_to_uyvy(&bgra, &mut out);
            assert_eq!(out, scalar_uyvy(&bgra), "UYVY n={n}");
        }
    }

    #[test]
    fn test_uyvy_1000_random() {
        let bgra = gen_test_bgra(1000);
        let pairs = 1000usize.div_ceil(2);
        let mut out = vec![0u8; pairs * 4];
        bgra_to_uyvy(&bgra, &mut out);
        assert_eq!(out, scalar_uyvy(&bgra), "UYVY 1000px");
    }

    // ---- CL tests ----

    #[test]
    fn test_cl_small() {
        let bgra = gen_test_bgra(4);
        let mut out = vec![0u8; 8];
        bgra_to_cl(&bgra, &mut out);
        assert_eq!(out, scalar_cl(&bgra), "CL 4px");
    }

    #[test]
    fn test_cl_various_sizes() {
        for &n in &[1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17] {
            let bgra = gen_test_bgra(n);
            let mut out = vec![0u8; n * 2];
            bgra_to_cl(&bgra, &mut out);
            assert_eq!(out, scalar_cl(&bgra), "CL n={n}");
        }
    }

    #[test]
    fn test_cl_1000_random() {
        let bgra = gen_test_bgra(1000);
        let mut out = vec![0u8; 2000];
        bgra_to_cl(&bgra, &mut out);
        assert_eq!(out, scalar_cl(&bgra), "CL 1000px");
    }

    // ---- Y plane tests ----

    #[test]
    fn test_y_plane_various_sizes() {
        for &n in &[1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33] {
            let bgra = gen_test_bgra(n);
            let mut y = vec![0u8; n];
            bgra_to_y_plane(&bgra, &mut y);
            for (i, y_val) in y.iter().enumerate() {
                let px = i * 4;
                let expected = clamp_u8(bt601_y(
                    i32::from(bgra[px + 2]),
                    i32::from(bgra[px + 1]),
                    i32::from(bgra[px]),
                ));
                assert_eq!(*y_val, expected, "Y plane n={n} i={i}");
            }
        }
    }

    #[test]
    fn test_y_plane_1000_random() {
        let bgra = gen_test_bgra(1000);
        let mut y = vec![0u8; 1000];
        bgra_to_y_plane(&bgra, &mut y);
        for (i, y_val) in y.iter().enumerate() {
            let px = i * 4;
            let expected = clamp_u8(bt601_y(
                i32::from(bgra[px + 2]),
                i32::from(bgra[px + 1]),
                i32::from(bgra[px]),
            ));
            assert_eq!(*y_val, expected, "Y plane i={i}");
        }
    }

    // ---- CLCL chroma plane tests ----

    #[test]
    fn test_clcl_c_plane() {
        for &n in &[1, 2, 3, 4, 5, 7, 8] {
            let bgra = gen_test_bgra(n);
            let c_len = n.div_ceil(2);
            let mut cb_out = vec![0u8; c_len];
            let mut cr_out = vec![0u8; c_len];
            bgra_to_clcl_c(&bgra, &mut cb_out, true);
            bgra_to_clcl_c(&bgra, &mut cr_out, false);
            for i in 0..n {
                let px = i * 4;
                let r = i32::from(bgra[px + 2]);
                let g = i32::from(bgra[px + 1]);
                let b = i32::from(bgra[px]);
                let cb_e = (clamp_u8(bt601_cb(r, g, b)) >> 4) & 0x0F;
                let cr_e = (clamp_u8(bt601_cr(r, g, b)) >> 4) & 0x0F;
                let ci = i / 2;
                let cb_a = if i & 1 == 0 { cb_out[ci] & 0x0F } else { cb_out[ci] >> 4 };
                let cr_a = if i & 1 == 0 { cr_out[ci] & 0x0F } else { cr_out[ci] >> 4 };
                assert_eq!(cb_a, cb_e, "Cb n={n} i={i}");
                assert_eq!(cr_a, cr_e, "Cr n={n} i={i}");
            }
        }
    }
}
