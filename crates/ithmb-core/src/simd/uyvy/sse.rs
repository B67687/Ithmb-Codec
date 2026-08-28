//! UYVY SSE2 quad-level conversions.

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
