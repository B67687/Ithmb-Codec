//! Shared pixel-manipulation helpers for encoding and decoding.
//!
//! These functions are used across multiple decoders and encoders to avoid
//! code duplication. They cover MSB replication, value clamping, and
//! cancellation-check boilerplate.

use crate::error::DecodeError;
use std::sync::atomic::{AtomicBool, Ordering};

/// Replicates a 5-bit value to 8 bits: `(v << 3) | (v >> 2)`.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn msb_replicate_5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

/// Replicates a 6-bit value to 8 bits: `(v << 2) | (v >> 4)`.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn msb_replicate_6(v: u32) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}

/// Unpack an RGB565 pixel to BGRA (4 bytes).
///
/// The caller must provide the correctly-parsed pixel value (endianness
/// already handled). This function only applies the `swap_rgb_channels`
/// (BGR15) layout used by iPhone 2G.
#[inline]
#[must_use]
pub(crate) fn unpack_rgb565(pixel: u16, swap: bool) -> [u8; 4] {
    let r5 = u32::from((pixel >> 11) & 0x1F);
    let g6 = u32::from((pixel >> 5) & 0x3F);
    let b5 = u32::from(pixel & 0x1F);
    if swap {
        [msb_replicate_5(r5), msb_replicate_6(g6), msb_replicate_5(b5), 255]
    } else {
        [msb_replicate_5(b5), msb_replicate_6(g6), msb_replicate_5(r5), 255]
    }
}

/// Unpack an RGB555 pixel to BGRA (4 bytes).
///
/// Handles the `swap_rgb_channels` (BGR15) layout used by iPhone 2G.
#[inline]
#[must_use]
pub(crate) fn unpack_rgb555(pixel: u16, swap: bool) -> [u8; 4] {
    let (r5, g5, b5) = if swap {
        (
            u32::from(pixel & 0x1F),
            u32::from((pixel >> 5) & 0x1F),
            u32::from((pixel >> 10) & 0x1F),
        )
    } else {
        (
            u32::from((pixel >> 10) & 0x1F),
            u32::from((pixel >> 5) & 0x1F),
            u32::from(pixel & 0x1F),
        )
    };
    [msb_replicate_5(b5), msb_replicate_5(g5), msb_replicate_5(r5), 255]
}

/// Clamp an `i32` to the 0..255 u8 range.
#[inline]
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub(crate) fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Check whether the operation has been canceled.
///
/// Returns `Err(DecodeError::Canceled)` if `canceled` is `true`, allowing the
/// caller to short-circuit the decode loop.
#[inline]
pub(crate) fn check_canceled(canceled: &AtomicBool, name: &str) -> Result<(), DecodeError> {
    if canceled.load(Ordering::Acquire) {
        return Err(DecodeError::Canceled(name.into()));
    }
    Ok(())
}

/// Rotate BGRA8 pixel data clockwise by `rotation` degrees (0/90/180/270;
/// any other value returns an unrotated copy).
///
/// Returns `(rotated_data, new_width, new_height)` — for 90°/270° the
/// dimensions swap. This is the single rotation implementation shared by
/// the pipeline post-processor, the encoders, and the JPEG EXIF path.
///
/// The mapping used is: 90° CW `old (x, y) → new (h-1-y, x)`,
/// 180° `old (x, y) → new (w-1-x, h-1-y)`,
/// 270° CW `old (x, y) → new (y, w-1-x)` — identical to the historic
/// `rotate_bgra` semantics of the encoder and the JPEG EXIF path.
#[must_use]
pub(crate) fn rotate_pixels(src: &[u8], width: u32, height: u32, rotation: i32) -> (Vec<u8>, u32, u32) {
    let wu = width as usize;
    let hu = height as usize;
    let total = wu * hu * 4;
    match rotation % 360 {
        90 => {
            let mut dst = vec![0u8; total];
            for sy in 0..hu {
                for sx in 0..wu {
                    let s_idx = (sy * wu + sx) * 4;
                    let dx = hu - 1 - sy;
                    let dy = sx;
                    let d_idx = (dy * hu + dx) * 4;
                    dst[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
                }
            }
            (dst, height, width)
        }
        180 => {
            let mut dst = vec![0u8; total];
            for sy in 0..hu {
                for sx in 0..wu {
                    let s_idx = (sy * wu + sx) * 4;
                    let dx = wu - 1 - sx;
                    let dy = hu - 1 - sy;
                    let d_idx = (dy * wu + dx) * 4;
                    dst[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
                }
            }
            (dst, width, height)
        }
        270 => {
            // 270° CW = 90° CCW
            let mut dst = vec![0u8; total];
            for sy in 0..hu {
                for sx in 0..wu {
                    let s_idx = (sy * wu + sx) * 4;
                    let dx = sy;
                    let dy = wu - 1 - sx;
                    let d_idx = (dy * hu + dx) * 4;
                    dst[d_idx..d_idx + 4].copy_from_slice(&src[s_idx..s_idx + 4]);
                }
            }
            (dst, height, width)
        }
        _ => (src.to_vec(), width, height),
    }
}

/// Convert every byte in a gray buffer to BGRA: `gray[n] -> [gray[n], gray[n], gray[n], 255]`.
#[allow(unsafe_code, dead_code)]
#[must_use]
pub fn fill_gray_row(gray: &[u8]) -> Vec<u8> {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        crate::simd::fill_gray_row_sse2(gray)
    }

    #[cfg(target_arch = "aarch64")]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return crate::simd::neon::fill_gray_row_neon(gray);
    }

    #[cfg(not(any(any(target_arch = "x86_64", target_arch = "x86"), target_arch = "aarch64",)))]
    crate::simd::scalar::fill_gray_row(gray)
}

/// Convert luma bytes to BGRA using a single shared Cb/Cr pair.
///
/// Processes batches of 4 via `yuv420_quad_to_bgra` (SIMD when possible).
#[allow(dead_code)]
#[must_use]
pub(crate) fn fill_yuv_row(luma: &[u8], cb: u8, cr: u8) -> Vec<u8> {
    let n = luma.len();
    let mut dst = vec![0u8; n * 4];
    let mut i = 0;

    while i + 4 <= n {
        let quad = crate::simd::yuv420_quad_to_bgra(&[luma[i], luma[i + 1], luma[i + 2], luma[i + 3], cb, cr]);
        let o = i * 4;
        dst[o..o + 16].copy_from_slice(&quad);
        i += 4;
    }

    for (j, &y) in luma.iter().enumerate().skip(i) {
        let px = crate::yuv::yuv_to_bgra(y, cb, cr);
        let o = j * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
    dst
}
