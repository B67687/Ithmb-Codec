//! SIMD-accelerated pixel conversions.
//!
//! Each public function has a platform-specific implementation (e.g. SSE2 on
//! `x86_64`) **and** a portable scalar fallback so the code compiles
//! on all architectures.
//!
//! SIMD targets are selected at compile time with appropriate runtime dispatch.
#![allow(unsafe_code, unreachable_code, dead_code)]
#![allow(clippy::cast_ptr_alignment, clippy::cast_possible_truncation, clippy::similar_names)]

// ---------------------------------------------------------------------------
// Sub-modules: per-format SIMD implementations + runtime dispatch
// ---------------------------------------------------------------------------
mod cl;
mod clcl;
mod reordered;
mod rgb555;
mod rgb565;
mod uyvy;
mod yuv;

// Scalar fallbacks -- always available (used when SIMD is off or for
// remainder handling in NEON routines).
#[cfg_attr(
    any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64"),
    allow(dead_code)
)]
pub(crate) mod scalar;

#[cfg(target_arch = "aarch64")]
pub(crate) mod neon;

// ---------------------------------------------------------------------------
// Re-exports -- dispatch functions live in their per-format sub-modules.
// ---------------------------------------------------------------------------
#[allow(unused_imports)]
pub use cl::{cl_quad_to_bgra, cl_row_to_bgra};
pub(crate) use clcl::clcl_row_to_bgra;
pub(crate) use reordered::rgb555_pack_to_bgra;
#[allow(unused_imports)]
pub use rgb555::rgb555_apply_row_to_bgra;
#[allow(unused_imports)]
pub use rgb565::rgb565_apply_row_to_bgra;
#[allow(unused_imports)]
pub use uyvy::{uyvy_double_quad_to_bgra, uyvy_quad_to_bgra, uyvy_row_to_bgra};
#[allow(unused_imports)]
pub use yuv::{yuv420_quad_to_bgra, yuv420_row_pair_to_bgra};

// Re-export shared helpers from pixel_utils for backward compatibility.
pub use crate::pixel_utils::fill_gray_row;

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------

/// Nibble-to-byte lookup table: maps each 4-bit value (0-15) to `nibble << 4 (= nibble * 16)`.
/// Used with `_mm_shuffle_epi8` (SSSE3) / `_mm256_shuffle_epi8` (AVX2) to
/// expand packed CL/CLCL nibble chroma to full 8-bit values in a single
/// instruction -- replaces per-pixel shift+mask+multiply.
pub(crate) const CL_NIBBLE_TABLE: [u8; 16] = [0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 176, 192, 208, 224, 240];

// ---------------------------------------------------------------------------
// Helper functions  (used by scalar fallbacks)
// ---------------------------------------------------------------------------

#[inline]
#[must_use]
pub(super) fn unpack_rgb565(pixel: u16) -> [u8; 4] {
    let r5 = u32::from((pixel >> 11) & 0x1F);
    let g6 = u32::from((pixel >> 5) & 0x3F);
    let b5 = u32::from(pixel & 0x1F);
    [msb_replicate_5(b5), msb_replicate_6(g6), msb_replicate_5(r5), 255]
}

use crate::pixel_utils::{msb_replicate_5, msb_replicate_6};

#[inline]
#[must_use]
pub(super) fn unpack_rgb555(pixel: u16) -> [u8; 4] {
    // Layout: xRRRRRGGGGGBBBBB (MSB bit 15 unused)
    let r5 = u32::from((pixel >> 10) & 0x1F);
    let g5 = u32::from((pixel >> 5) & 0x1F);
    let b5 = u32::from(pixel & 0x1F);
    [msb_replicate_5(b5), msb_replicate_5(g5), msb_replicate_5(r5), 255]
}

// ---- Fill gray row (SSE2) ----

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn fill_gray_row_sse2(gray: &[u8]) -> Vec<u8> {
    use core::arch::x86_64::{
        __m128i, _mm_loadl_epi64, _mm_or_si128, _mm_set1_epi32, _mm_setzero_si128, _mm_slli_epi32, _mm_storeu_si128,
        _mm_unpackhi_epi16, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };
    let n = gray.len();
    let mut dst = vec![0u8; n * 4];
    let mut i = 0;

    let alpha_mask = _mm_set1_epi32(0xFFi32 << 24);
    while i + 8 <= n {
        let v = _mm_loadl_epi64(gray.as_ptr().add(i).cast::<__m128i>());
        let w = _mm_unpacklo_epi8(v, _mm_setzero_si128());
        let lo = _mm_unpacklo_epi16(w, _mm_setzero_si128());
        let hi = _mm_unpackhi_epi16(w, _mm_setzero_si128());

        let lo_sh8 = _mm_slli_epi32(lo, 8);
        let lo_sh16 = _mm_slli_epi32(lo, 16);
        let hi_sh8 = _mm_slli_epi32(hi, 8);
        let hi_sh16 = _mm_slli_epi32(hi, 16);

        let lo_bgra = _mm_or_si128(_mm_or_si128(_mm_or_si128(lo, lo_sh8), lo_sh16), alpha_mask);
        let hi_bgra = _mm_or_si128(_mm_or_si128(_mm_or_si128(hi, hi_sh8), hi_sh16), alpha_mask);

        _mm_storeu_si128(dst.as_mut_ptr().add(i * 4).cast::<__m128i>(), lo_bgra);
        _mm_storeu_si128(dst.as_mut_ptr().add(i * 4 + 16).cast::<__m128i>(), hi_bgra);
        i += 8;
    }

    for (j, &g) in gray.iter().enumerate().skip(i) {
        let o = j * 4;
        dst[o] = g;
        dst[o + 1] = g;
        dst[o + 2] = g;
        dst[o + 3] = 255;
    }
    dst
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn uyvy_quad_neutral_chroma() {
        // U=128 (neutral Cb), V=128 (neutral Cr), Y0=100, Y1=200
        // Both pixels should be gray: [100,100,100,255] and [200,200,200,255]
        let quad = [128u8, 100, 128, 200];
        let result = uyvy_quad_to_bgra(&quad);
        assert_eq!(result[..4], [100, 100, 100, 255], "pixel 0 gray");
        assert_eq!(result[4..], [200, 200, 200, 255], "pixel 1 gray");
    }

    #[test]
    fn uyvy_quad_matches_scalar() {
        // Compare result to scalar yuv_to_bgra for random-ish values.
        for u in [0u8, 64, 128, 192, 255] {
            for v in [0u8, 64, 128, 192, 255] {
                for y0 in [0u8, 64, 128, 192, 255] {
                    for y1 in [0u8, 64, 128, 192, 255] {
                        let quad = [u, y0, v, y1];
                        let result = uyvy_quad_to_bgra(&quad);
                        let expected_p0 = crate::yuv::yuv_to_bgra(y0, u, v);
                        let expected_p1 = crate::yuv::yuv_to_bgra(y1, u, v);
                        assert_eq!(result[..4], expected_p0, "p0 mismatch: u={u}, y0={y0}, v={v}");
                        assert_eq!(result[4..], expected_p1, "p1 mismatch: u={u}, y1={y1}, v={v}");
                    }
                }
            }
        }
    }

    #[test]
    fn uyvy_double_quad_matches_scalar() {
        let quads = [128u8, 100, 128, 200, 64, 50, 192, 180];
        let result = uyvy_double_quad_to_bgra(&quads);
        let expected_left = uyvy_quad_to_bgra(&[128, 100, 128, 200]);
        let expected_right = uyvy_quad_to_bgra(&[64, 50, 192, 180]);
        assert_eq!(result[..8], expected_left);
        assert_eq!(result[8..], expected_right);
    }

    #[test]
    fn yuv420_quad_neutral_chroma_gray() {
        // Cb=128, Cr=128 (neutral), Y values 0,128,200,255
        // All pixels should be gray: Y,Y,Y,255
        let quad = [0u8, 128, 200, 255, 128, 128];
        let result = yuv420_quad_to_bgra(&quad);
        let expected: Vec<[u8; 4]> = (0..4)
            .map(|i| {
                let y = quad[i];
                crate::yuv::yuv_to_bgra(y, 128, 128)
            })
            .collect();
        assert_eq!(result[..4], expected[0], "pixel 0");
        assert_eq!(result[4..8], expected[1], "pixel 1");
        assert_eq!(result[8..12], expected[2], "pixel 2");
        assert_eq!(result[12..], expected[3], "pixel 3");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn yuv420_quad_matches_scalar_exhaustive() {
        // combinations of Y0,Y1,Y2,Y3,Cb,Cr from [0,64,128,192,255].
        for y0 in [0u8, 64, 128, 192, 255] {
            for y1 in [0u8, 64, 128, 192, 255] {
                for y2 in [0u8, 64, 128, 192, 255] {
                    for y3 in [0u8, 64, 128, 192, 255] {
                        for cb in [0u8, 64, 128, 192, 255] {
                            for cr in [0u8, 64, 128, 192, 255] {
                                let quad = [y0, y1, y2, y3, cb, cr];
                                let result = yuv420_quad_to_bgra(&quad);
                                let expected = [
                                    crate::yuv::yuv_to_bgra(y0, cb, cr),
                                    crate::yuv::yuv_to_bgra(y1, cb, cr),
                                    crate::yuv::yuv_to_bgra(y2, cb, cr),
                                    crate::yuv::yuv_to_bgra(y3, cb, cr),
                                ];
                                for (i, exp) in expected.iter().enumerate() {
                                    let slice = &result[i * 4..(i + 1) * 4];
                                    assert_eq!(slice, exp, "pixel {i}: yuv420({y0},{y1},{y2},{y3},cb={cb},cr={cr})");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn uyvy_quad_matches_scalar_10k_random() {
        // Generate 10,000 random UYVY quads and compare SSE4.1 result vs scalar.
        let mut state: u32 = 0xABCD_0001;
        for _ in 0..10_000 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let u = (state >> 16) as u8;
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let y0 = (state >> 16) as u8;
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let v = (state >> 16) as u8;
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let y1 = (state >> 16) as u8;
            let quad = [u, y0, v, y1];
            let result = uyvy_quad_to_bgra(&quad);
            let expected_p0 = crate::yuv::yuv_to_bgra(y0, u, v);
            let expected_p1 = crate::yuv::yuv_to_bgra(y1, u, v);
            assert_eq!(&result[..4], &expected_p0, "p0 mismatch");
            assert_eq!(&result[4..], &expected_p1, "p1 mismatch");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn yuv420_quad_matches_scalar_10k_random() {
        // Generate 10,000 random YCbCr 4:2:0 quads and compare SSE4.1 vs scalar.
        let mut state: u32 = 0xABCD_0001;
        for _ in 0..10_000 {
            let mut quad = [0u8; 6];
            for b in &mut quad {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                *b = (state >> 16) as u8;
            }
            let result = yuv420_quad_to_bgra(&quad);
            let expected = [
                crate::yuv::yuv_to_bgra(quad[0], quad[4], quad[5]),
                crate::yuv::yuv_to_bgra(quad[1], quad[4], quad[5]),
                crate::yuv::yuv_to_bgra(quad[2], quad[4], quad[5]),
                crate::yuv::yuv_to_bgra(quad[3], quad[4], quad[5]),
            ];
            for (i, exp) in expected.iter().enumerate() {
                let slice = &result[i * 4..(i + 1) * 4];
                assert_eq!(slice, exp, "pixel {i}: quad={quad:?}");
            }
        }
    }

    // ---- RGB565 cross-validation ----

    /// Generate a pseudo-random u16 value.
    fn random_u16(state: &mut u32) -> u16 {
        *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (*state >> 16) as u16
    }

    #[test]
    fn rgb565_row_matches_scalar_10k() {
        // Generate 10_000 random RGB565 pixel values (20_000 bytes)
        let mut state: u32 = 0xABCD_0001;
        let mut src = Vec::with_capacity(20_000);
        for _ in 0..10_000 {
            let p = random_u16(&mut state);
            src.push((p & 0xFF) as u8);
            src.push((p >> 8) as u8);
        }

        let mut result = vec![0u8; 10_000 * 4];
        super::rgb565::rgb565_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 10_000 * 4];
        super::scalar::rgb565_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB565 SIMD/scalar mismatch for 10K random pixels");
    }

    #[test]
    fn rgb565_quad_edge_cases() {
        let pixels: [u16; 8] = [0x0000, 0xFFFF, 0x001F, 0x07E0, 0xF800, 0xFFE0, 0x07FF, 0x1B6A];
        for &p0 in &pixels {
            for &p1 in &pixels {
                let quad = [
                    (p0 & 0xFF) as u8,
                    (p0 >> 8) as u8,
                    (p1 & 0xFF) as u8,
                    (p1 >> 8) as u8,
                    (p0 & 0xFF) as u8,
                    (p0 >> 8) as u8,
                    (p1 & 0xFF) as u8,
                    (p1 >> 8) as u8,
                ];

                let mut expected = vec![0u8; 16];
                super::scalar::rgb565_row_to_bgra_scalar(&quad, &mut expected);

                let mut result = vec![0u8; 16];
                super::rgb565::rgb565_apply_row_to_bgra(&quad, &mut result);
                assert_eq!(
                    &result[..16],
                    &expected[..],
                    "RGB565 dispatch mismatch for pixels {p0:#06x}, {p1:#06x}"
                );
            }
        }
    }

    #[test]
    fn rgb565_row_remainder_handling() {
        // Test with 5 pixels (1 remainder after 1 quad)
        let mut state: u32 = 0xDEAD_BEEF;
        let mut src = Vec::with_capacity(10);
        for _ in 0..5 {
            let p = random_u16(&mut state);
            src.push((p & 0xFF) as u8);
            src.push((p >> 8) as u8);
        }

        let mut result = vec![0u8; 20];
        super::rgb565::rgb565_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 20];
        super::scalar::rgb565_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB565 remainder handling mismatch");
    }

    #[test]
    fn rgb565_row_single_pixel() {
        let src = [0x00, 0xF8]; // red
        let mut result = vec![0u8; 4];
        super::rgb565::rgb565_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 4];
        super::scalar::rgb565_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB565 single pixel mismatch");
    }

    // ---- RGB555 cross-validation ----

    #[test]
    fn rgb555_row_matches_scalar_10k() {
        let mut state: u32 = 0xABCD_0001;
        let mut src = Vec::with_capacity(20_000);
        for _ in 0..10_000 {
            let p = random_u16(&mut state) & 0x7FFF; // mask off bit 15
            src.push((p & 0xFF) as u8);
            src.push((p >> 8) as u8);
        }

        let mut result = vec![0u8; 10_000 * 4];
        super::rgb555::rgb555_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 10_000 * 4];
        super::scalar::rgb555_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB555 SIMD/scalar mismatch for 10K random pixels");
    }

    #[test]
    fn rgb555_quad_edge_cases() {
        let pixels: [u16; 6] = [0x0000, 0x7FFF, 0x001F, 0x03E0, 0x7C00, 0x7C1F];
        for &p0 in &pixels {
            for &p1 in &pixels {
                let quad = [
                    (p0 & 0xFF) as u8,
                    (p0 >> 8) as u8,
                    (p1 & 0xFF) as u8,
                    (p1 >> 8) as u8,
                    (p0 & 0xFF) as u8,
                    (p0 >> 8) as u8,
                    (p1 & 0xFF) as u8,
                    (p1 >> 8) as u8,
                ];

                let mut expected = vec![0u8; 16];
                super::scalar::rgb555_row_to_bgra_scalar(&quad, &mut expected);

                let mut result = vec![0u8; 16];
                super::rgb555::rgb555_apply_row_to_bgra(&quad, &mut result);
                assert_eq!(
                    &result[..16],
                    &expected[..],
                    "RGB555 dispatch mismatch for pixels {p0:#06x}, {p1:#06x}"
                );
            }
        }
    }

    #[test]
    fn rgb555_row_remainder_handling() {
        let mut state: u32 = 0xCAFE_BABE;
        let mut src = Vec::with_capacity(10);
        for _ in 0..5 {
            let p = random_u16(&mut state) & 0x7FFF;
            src.push((p & 0xFF) as u8);
            src.push((p >> 8) as u8);
        }

        let mut result = vec![0u8; 20];
        super::rgb555::rgb555_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 20];
        super::scalar::rgb555_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB555 remainder handling mismatch");
    }

    #[test]
    fn rgb555_row_single_pixel() {
        let src = [0x00, 0x7C]; // red (RGB555)
        let mut result = vec![0u8; 4];
        super::rgb555::rgb555_apply_row_to_bgra(&src, &mut result);
        let mut expected = vec![0u8; 4];
        super::scalar::rgb555_row_to_bgra_scalar(&src, &mut expected);
        assert_eq!(result, expected, "RGB555 single pixel mismatch");
    }

    // ---- fill_gray_row cross-validation ----

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn fill_gray_row_matches_scalar_1000_random() {
        let mut state: u32 = 0x1234_5678;
        let lengths = [1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 256];
        for &len in &lengths {
            for _ in 0..60 {
                let mut gray = Vec::with_capacity(len);
                for _ in 0..len {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    gray.push((state >> 16) as u8);
                }
                let result = crate::pixel_utils::fill_gray_row(&gray);
                let expected = super::scalar::fill_gray_row(&gray);
                assert_eq!(result, expected, "fill_gray_row mismatch for len={len}");
            }
        }
    }

    // ---- fill_yuv_row cross-validation ----

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn fill_yuv_row_matches_scalar_1000_random() {
        let mut state: u32 = 0x9ABC_DEF0;
        let lengths = [1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 256];
        for &len in &lengths {
            for _ in 0..60 {
                let mut luma = Vec::with_capacity(len);
                for _ in 0..len {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    luma.push((state >> 16) as u8);
                }
                let cb = (state >> 8) as u8;
                let cr = (state >> 16) as u8;
                let result = crate::pixel_utils::fill_yuv_row(&luma, cb, cr);
                let mut expected = vec![0u8; len * 4];
                for (j, &y) in luma.iter().enumerate() {
                    let px = crate::yuv::yuv_to_bgra(y, cb, cr);
                    expected[j * 4..j * 4 + 4].copy_from_slice(&px);
                }
                assert_eq!(result, expected, "fill_yuv_row mismatch for len={len}");
            }
        }
    }

    // ---- cl_quad_to_bgra cross-validation ----

    #[test]
    fn cl_quad_matches_scalar_all_nibble_combos() {
        let y_vals = [0u8, 64, 128, 192, 255];
        let n_vals = [0u8, 1, 7, 8, 15];
        for &y0 in &y_vals {
            for &y1 in &y_vals {
                for &y2 in &y_vals {
                    for &y3 in &y_vals {
                        for &cb_n in &n_vals {
                            for &cr_n in &n_vals {
                                let chroma = (cr_n << 4) | cb_n;
                                let quad = [y0, y1, y2, y3, chroma, chroma, chroma, chroma];
                                let result = super::cl::cl_quad_to_bgra(&quad);
                                let expected = super::scalar::cl_quad_to_bgra(quad);
                                assert_eq!(&result[..], &expected[..], "cl_quad mismatch");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn cl_quad_ssse3_exhaustive_nibble_pair() {
        // Validate SSSE3 pshufb path against mathematical *17 expansion.
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        if is_x86_feature_detected!("ssse3") {
            for cb_n in 0..=15u8 {
                for cr_n in 0..=15u8 {
                    let chroma = (cr_n << 4) | cb_n;
                    let quad = [128u8, 128, 128, 128, chroma, chroma, chroma, chroma];
                    let result = unsafe { super::cl::cl_quad_to_bgra_ssse3(&quad) };
                    let cb = cb_n << 4;
                    let cr = cr_n << 4;
                    let expected = crate::yuv::yuv_to_bgra(128, cb, cr);
                    for p in 0..4 {
                        let off = p * 4;
                        assert_eq!(
                            result[off..off + 4],
                            expected,
                            "SSSE3 pixel {p} mismatch at cb_n={cb_n} cr_n={cr_n}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn cl_quad_avx2_exhaustive_nibble_pair() {
        // Validate AVX2 vpshufb path against mathematical *17 expansion.
        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            for cb_n in 0..=15u8 {
                for cr_n in 0..=15u8 {
                    let chroma = (cr_n << 4) | cb_n;
                    let quad = [128u8, 128, 128, 128, chroma, chroma, chroma, chroma];
                    let result = unsafe { super::cl::cl_quad_to_bgra_avx2(&quad) };
                    let cb = cb_n << 4;
                    let cr = cr_n << 4;
                    let expected = crate::yuv::yuv_to_bgra(128, cb, cr);
                    for p in 0..4 {
                        let off = p * 4;
                        assert_eq!(
                            result[off..off + 4],
                            expected,
                            "AVX2 pixel {p} mismatch at cb_n={cb_n} cr_n={cr_n}",
                        );
                    }
                }
            }
        }
    }
    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn cl_quad_matches_scalar_varying_chroma() {
        let mut state: u32 = 0xDEAD_BEEF;
        for _ in 0..1000 {
            let mut quad = [0u8; 8];
            for b in &mut quad {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                *b = (state >> 16) as u8;
            }
            let result = super::cl::cl_quad_to_bgra(&quad);
            let expected = super::scalar::cl_quad_to_bgra(quad);
            assert_eq!(&result[..], &expected[..], "cl_quad mismatch for random quad");
        }
    }

    // ---- cl_row_to_bgra cross-validation ----

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn cl_row_matches_scalar_at_various_widths() {
        // Test that cl_row_to_bgra matches per-pixel yuv_to_bgra at widths
        // that exercise the SIMD batch loop AND the odd-pixel remainder path.
        let widths = [1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 128, 256];
        let mut state: u32 = 0xDEAD_BEEF;
        for &w in &widths {
            for _ in 0..20 {
                let n = w;
                let mut src = vec![0u8; n * 2];
                for b in &mut src {
                    state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    *b = (state >> 16) as u8;
                }
                let mut dst = vec![0u8; n * 4];
                super::cl::cl_row_to_bgra(&src, &mut dst);
                let (y, chroma) = src.split_at(n);
                let mut expected = vec![0u8; n * 4];
                for i in 0..n {
                    let cr = chroma[i] & 0xF0;
                    let cb = (chroma[i] & 0x0F) << 4;
                    let px = crate::yuv::yuv_to_bgra(y[i], cb, cr);
                    let o = i * 4;
                    expected[o..o + 4].copy_from_slice(&px);
                }
                assert_eq!(dst, expected, "cl_row_to_bgra mismatch at width={w}");
            }
        }
    }

    // ---- rgb555_pack tests ----

    #[test]
    fn rgb555_pack_known_values() {
        // White: R=31, G=31, B=31 -> all channels 255
        // 0x7FFF LE: [0xFF, 0x7F]
        let white = [0xFFu8, 0x7Fu8];
        let result = super::rgb555_pack_to_bgra([white, white, white, white], false);
        assert_eq!(result, [0xFFu8; 16], "white");

        // Black: all zeros
        let black = [0x00u8, 0x00u8];
        let result = super::rgb555_pack_to_bgra([black, black, black, black], false);
        // Black: [B=0, G=0, R=0, A=255] for each of 4 pixels
        let mut black_expected = [0u8; 16];
        for i in 0..4 {
            black_expected[i * 4 + 3] = 255;
        }
        assert_eq!(result, black_expected, "black");

        // Red: R=31 -> 0x7C00 LE: [0x00, 0x7C]
        let red = [0x00u8, 0x7Cu8];
        let result = super::rgb555_pack_to_bgra([red, red, red, red], false);
        for i in 0..4 {
            assert_eq!(result[i * 4..][..4], [0, 0, 255, 255], "red pixel {i}");
        }

        // Green: G=31 -> 0x03E0 LE: [0xE0, 0x03]
        let green = [0xE0u8, 0x03u8];
        let result = super::rgb555_pack_to_bgra([green, green, green, green], false);
        for i in 0..4 {
            assert_eq!(result[i * 4..][..4], [0, 255, 0, 255], "green pixel {i}");
        }

        // Blue: B=31 -> 0x001F LE: [0x1F, 0x00]
        let blue = [0x1Fu8, 0x00u8];
        let result = super::rgb555_pack_to_bgra([blue, blue, blue, blue], false);
        for i in 0..4 {
            assert_eq!(result[i * 4..][..4], [255, 0, 0, 255], "blue pixel {i}");
        }
    }

    #[test]
    fn rgb555_pack_swap_mode() {
        // swap=true: layout xBBBBBGGGGGRRRRR
        // B=31 in high bits -> pixel 0x7C00 LE: [0x00, 0x7C] -> B_out=255
        let pixel = [0x00u8, 0x7Cu8];
        let result = super::rgb555_pack_to_bgra([pixel, pixel, pixel, pixel], true);
        for i in 0..4 {
            assert_eq!(result[i * 4..][..4], [255, 0, 0, 255], "swap B=31->B_out pixel {i}");
        }

        // R=31 in low bits -> pixel 0x001F LE: [0x1F, 0x00] -> R_out=255
        let pixel = [0x1Fu8, 0x00u8];
        let result = super::rgb555_pack_to_bgra([pixel, pixel, pixel, pixel], true);
        for i in 0..4 {
            assert_eq!(result[i * 4..][..4], [0, 0, 255, 255], "swap R=31->R_out pixel {i}");
        }
    }

    #[test]
    fn rgb555_pack_different_pixels() {
        // 4 different pixels in one call, LE byte order.
        let white = [0xFFu8, 0x7Fu8]; // 0x7FFF
        let red = [0x00u8, 0x7Cu8]; // 0x7C00
        let green = [0xE0u8, 0x03u8]; // 0x03E0
        let blue = [0x1Fu8, 0x00u8]; // 0x001F

        let pixels = [white, red, green, blue];
        let result = super::rgb555_pack_to_bgra(pixels, false);

        assert_eq!(result[0..4], [255, 255, 255, 255], "p0 white");
        assert_eq!(result[4..8], [0, 0, 255, 255], "p1 red");
        assert_eq!(result[8..12], [0, 255, 0, 255], "p2 green");
        assert_eq!(result[12..16], [255, 0, 0, 255], "p3 blue");
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn rgb555_pack_1000_random_cross_check() {
        // Deterministic PCG-style RNG.
        let mut state: u64 = 42;
        let mut rng = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };

        for _ in 0..1000 {
            let mut pixels = [[0u8; 2]; 4];
            for p in &mut pixels {
                let val = (rng() & 0x7FFF) as u16; // 15-bit RGB555
                *p = val.to_le_bytes();
            }
            let swap = (rng() & 1) != 0;

            let result = super::rgb555_pack_to_bgra(pixels, swap);

            // Manually compute expected.
            let mut expected = [0u8; 16];
            for i in 0..4 {
                let raw = u16::from_le_bytes(pixels[i]);
                let (r5, g5, b5) = if swap {
                    (raw & 0x1F, (raw >> 5) & 0x1F, (raw >> 10) & 0x1F)
                } else {
                    ((raw >> 10) & 0x1F, (raw >> 5) & 0x1F, raw & 0x1F)
                };
                expected[i * 4] = ((b5 << 3) | (b5 >> 2)) as u8;
                expected[i * 4 + 1] = ((g5 << 3) | (g5 >> 2)) as u8;
                expected[i * 4 + 2] = ((r5 << 3) | (r5 >> 2)) as u8;
                expected[i * 4 + 3] = 255;
            }

            assert_eq!(result, expected, "Mismatch for swap={swap}");
        }
    }

    // ---- SSSE3/AVX2 rgb555_pack random cross-checks ----

    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    #[cfg_attr(miri, ignore)]
    fn rgb555_pack_10k_random_ssse3() {
        use super::scalar;
        // Only run on SSSE3-capable hardware.
        if !is_x86_feature_detected!("ssse3") {
            return;
        }
        let mut state: u64 = 42;
        let mut rng = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };
        for _ in 0..10_000 {
            let mut pixels = [[0u8; 2]; 4];
            for p in &mut pixels {
                let val = (rng() & 0x7FFF) as u16;
                *p = val.to_le_bytes();
            }
            let swap = (rng() & 1) != 0;
            let expected = scalar::rgb555_pack_to_bgra(pixels, swap);
            let result;
            // SAFETY: checked is_x86_feature_detected!("ssse3") above.
            unsafe {
                result = super::reordered::rgb555_pack_to_bgra_ssse3(&pixels, swap);
            }
            assert_eq!(result, expected, "SSSE3 variant mismatch for swap={swap}");
        }
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    #[cfg_attr(miri, ignore)]
    fn rgb555_pack_10k_random_avx2() {
        use super::scalar;
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let mut state: u64 = 42;
        let mut rng = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };
        for _ in 0..10_000 {
            let mut pixels = [[0u8; 2]; 4];
            for p in &mut pixels {
                let val = (rng() & 0x7FFF) as u16;
                *p = val.to_le_bytes();
            }
            let swap = (rng() & 1) != 0;
            let expected = scalar::rgb555_pack_to_bgra(pixels, swap);
            let result;
            // SAFETY: checked is_x86_feature_detected!("avx2") above.
            unsafe {
                result = super::reordered::rgb555_pack_to_bgra_avx2(&pixels, swap);
            }
            assert_eq!(result, expected, "AVX2 variant mismatch for swap={swap}");
        }
    }
}
