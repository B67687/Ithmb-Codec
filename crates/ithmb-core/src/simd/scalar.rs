//! Scalar (portable) fallback implementations for pixel conversions.
//! Always available — used when SIMD features are disabled or during
//! remainder handling.

#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
use crate::yuv;

#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn uyvy_quad_to_bgra(quad: &[u8; 4]) -> [u8; 8] {
    let u = quad[0];
    let y0 = quad[1];
    let v = quad[2];
    let y1 = quad[3];
    let p0 = yuv::yuv_to_bgra(y0, u, v);
    let p1 = yuv::yuv_to_bgra(y1, u, v);
    [p0[0], p0[1], p0[2], p0[3], p1[0], p1[1], p1[2], p1[3]]
}

#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn uyvy_double_quad_to_bgra(quads: &[u8; 8]) -> [u8; 16] {
    let left = uyvy_quad_to_bgra(&[quads[0], quads[1], quads[2], quads[3]]);
    let right = uyvy_quad_to_bgra(&[quads[4], quads[5], quads[6], quads[7]]);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&left);
    out[8..].copy_from_slice(&right);
    out
}

#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn yuv420_quad_to_bgra(quad: &[u8; 6]) -> [u8; 16] {
    let [y0, y1, y2, y3, cb, cr] = *quad;
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&yuv::yuv_to_bgra(y0, cb, cr));
    out[4..8].copy_from_slice(&yuv::yuv_to_bgra(y1, cb, cr));
    out[8..12].copy_from_slice(&yuv::yuv_to_bgra(y2, cb, cr));
    out[12..].copy_from_slice(&yuv::yuv_to_bgra(y3, cb, cr));
    out
}

#[inline]
pub(crate) fn rgb565_row_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    debug_assert_eq!(dst.len(), n_pixels * 4);
    for i in 0..n_pixels {
        let px = super::unpack_rgb565(u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]));
        let o = i * 4;
        dst[o] = px[0];
        dst[o + 1] = px[1];
        dst[o + 2] = px[2];
        dst[o + 3] = px[3];
    }
}

#[inline]
pub(crate) fn rgb555_row_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    debug_assert_eq!(dst.len(), n_pixels * 4);
    for i in 0..n_pixels {
        let px = super::unpack_rgb555(u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]));
        let o = i * 4;
        dst[o] = px[0];
        dst[o + 1] = px[1];
        dst[o + 2] = px[2];
        dst[o + 3] = px[3];
    }
}

#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn rgb555_pack_to_bgra(pixels: [[u8; 2]; 4], swap: bool) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let raw = u16::from_le_bytes(pixels[i]);
        let (r5, g5, b5) = if swap {
            (
                u32::from(raw & 0x1F),
                u32::from((raw >> 5) & 0x1F),
                u32::from((raw >> 10) & 0x1F),
            )
        } else {
            (
                u32::from((raw >> 10) & 0x1F),
                u32::from((raw >> 5) & 0x1F),
                u32::from(raw & 0x1F),
            )
        };
        let b = ((b5 << 3) | (b5 >> 2)) as u8;
        let g = ((g5 << 3) | (g5 >> 2)) as u8;
        let r = ((r5 << 3) | (r5 >> 2)) as u8;
        out[i * 4] = b;
        out[i * 4 + 1] = g;
        out[i * 4 + 2] = r;
        out[i * 4 + 3] = 255;
    }
    out
}

#[inline]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn fill_gray_row(gray: &[u8]) -> Vec<u8> {
    gray.iter().flat_map(|&g| [g, g, g, 255]).collect()
}

#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn cl_quad_to_bgra(quad: [u8; 8]) -> [u8; 16] {
    let y0 = u32::from(quad[0]);
    let y1 = u32::from(quad[1]);
    let y2 = u32::from(quad[2]);
    let y3 = u32::from(quad[3]);
    let cr0 = u32::from(quad[4]) & 0xF0;
    let cb0 = (u32::from(quad[4]) & 0x0F) << 4;
    let cr1 = u32::from(quad[5]) & 0xF0;
    let cb1 = (u32::from(quad[5]) & 0x0F) << 4;
    let cr2 = u32::from(quad[6]) & 0xF0;
    let cb2 = (u32::from(quad[6]) & 0x0F) << 4;
    let cr3 = u32::from(quad[7]) & 0xF0;
    let cb3 = (u32::from(quad[7]) & 0x0F) << 4;
    let mut out = [0u8; 16];
    let p0 = crate::yuv::yuv_to_bgra(y0 as u8, cb0 as u8, cr0 as u8);
    let p1 = crate::yuv::yuv_to_bgra(y1 as u8, cb1 as u8, cr1 as u8);
    let p2 = crate::yuv::yuv_to_bgra(y2 as u8, cb2 as u8, cr2 as u8);
    let p3 = crate::yuv::yuv_to_bgra(y3 as u8, cb3 as u8, cr3 as u8);
    out[..4].copy_from_slice(&p0);
    out[4..8].copy_from_slice(&p1);
    out[8..12].copy_from_slice(&p2);
    out[12..].copy_from_slice(&p3);
    out
}

/// Convert a full row of CL planar data to BGRA (scalar fallback).
///
/// Input `src` layout:
///   `src[0..n_pixels]` = Y bytes (one per pixel)
///   `src[n_pixels..]` = `CbCr` bytes (Cr in high nibble, Cb in low nibble)
/// Output `dst`: `n_pixels * 4` bytes BGRA.
#[inline]
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))))]
pub(crate) fn cl_row_to_bgra_scalar(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    for i in 0..n_pixels {
        let cr = chroma[i] & 0xF0; // high nibble → Cr
        let cb = (chroma[i] & 0x0F) << 4; // low nibble → Cb
        let px = crate::yuv::yuv_to_bgra(y[i], cb, cr);
        let o = i * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
}
