//! UYVY 4:2:2 -> BGRA - SIMD-accelerated (SSE2, SSE4.1, AVX2 on `x86_64`).
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
        sse::uyvy_quad_to_bgra_sse2(quad)
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
        sse::uyvy_double_quad_to_bgra_sse2(quads).expect("UYVY double quad SSE2 conversion infallible")
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
/// Returns [`crate::error::DecodeError::BufferTooShort`] when `src` does not contain a whole number of quads.
#[inline]
#[allow(clippy::too_many_lines)]
pub fn uyvy_row_to_bgra(src: &[u8], dst: &mut [u8]) -> Result<(), crate::error::DecodeError> {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("avx2") {
        return {
            unsafe { avx2::uyvy_row_to_bgra_avx2(src, dst) };
            Ok(())
        };
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("sse4.1") {
        return {
            unsafe { sse41::uyvy_row_to_bgra_sse41(src, dst) };
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
            let q0 = sse::uyvy_double_quad_to_bgra_sse2(&src[i..i + 8].try_into().map_err(|_| {
                crate::error::DecodeError::BufferTooShort {
                    expected: 8,
                    actual: src[i..i + 8].len(),
                }
            })?)?;
            let q1 = sse::uyvy_double_quad_to_bgra_sse2(&src[i + 8..i + 16].try_into().map_err(|_| {
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
            let px = sse::uyvy_quad_to_bgra_sse2(&arr);
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
