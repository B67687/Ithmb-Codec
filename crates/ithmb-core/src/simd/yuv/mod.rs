//! YCbCr 4:2:0 -> BGRA - SIMD-accelerated (SSE2, SSE4.1, AVX2 on `x86_64`).
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
// Runtime dispatch
// ---------------------------------------------------------------------------

/// Convert 4 YCbCr 4:2:0 pixels sharing Cb/Cr to 4 BGRA pixels (16 bytes).
///
/// Input layout: `[Y0, Y1, Y2, Y3, Cb, Cr]` -- 6 bytes
/// Output layout: `[B0, G0, R0, A0, B1, G1, R1, A1, B2, G2, R2, A2, B3, G3, R3, A3]`
///
/// This is the core inner-loop primitive called by the YCbCr 4:2:0 decoder
/// for each macroblock. On `x86_64` with SSE2 it processes all 4 pixels with
/// packed `i32` arithmetic.
#[inline]
#[must_use]
#[allow(clippy::trivially_copy_pass_by_ref)]
pub fn yuv420_quad_to_bgra(quad: &[u8; 6]) -> [u8; 16] {
    // SSE2 path (compile-time guaranteed on x86_64/x86)
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        sse::yuv420_quad_to_bgra_sse2(quad)
    }

    #[cfg(not(any(any(target_arch = "x86_64", target_arch = "x86"),)))]
    // Scalar fallback (used on all non-x86_64 platforms, including aarch64+simd)
    super::scalar::yuv420_quad_to_bgra(quad)
}

/// Convert an entire row-pair of YCbCr 4:2:0 data (2 rows of Y, 1 row each of Cb/Cr)
///
/// Each 4-pixel macroblock (2x2) is decoded via the platform-specific SIMD primitive,
/// bypassing the per-macroblock dispatch overhead.
///
/// # Arguments
///
/// * `y_row` - Two rows of Y data (`2 * w` bytes)
/// * `cb_row` - One row of Cb data (`cb_w` bytes)
/// * `cr_row` - One row of Cr data (`cb_w` bytes)
/// * `dst` - Output buffer (`2 * w * 4` bytes)
/// * `w` - Width in pixels
/// * `cb_w` - Chroma width (`w / 2`)
#[inline]
pub fn yuv420_row_pair_to_bgra(y_row: &[u8], cb_row: &[u8], cr_row: &[u8], dst: &mut [u8], w: usize, cb_w: usize) {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("avx2") {
        return unsafe { avx2::yuv420_row_pair_to_bgra_avx2(y_row, cb_row, cr_row, dst, w, cb_w) };
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("sse4.1") {
        return unsafe { sse41::yuv420_row_pair_to_bgra_sse41(y_row, cb_row, cr_row, dst, w, cb_w) };
    }

    #[cfg(target_arch = "aarch64")]
    #[allow(unreachable_code)]
    // SAFETY: aarch64 guarantees NEON.
    unsafe {
        return super::neon::yuv420_row_pair_to_bgra_neon(y_row, cb_row, cr_row, dst, w, cb_w);
    }
    for cx in 0..cb_w {
        let quad = [
            y_row[cx * 2],
            y_row[cx * 2 + 1],
            y_row[w + cx * 2],
            y_row[w + cx * 2 + 1],
            cb_row[cx],
            cr_row[cx],
        ];
        let out = yuv420_quad_to_bgra(&quad);

        let off = cx * 8;
        dst[off..off + 4].copy_from_slice(&out[0..4]);
        dst[off + 4..off + 8].copy_from_slice(&out[4..8]);
        let off2 = off + w * 4;
        dst[off2..off2 + 4].copy_from_slice(&out[8..12]);
        dst[off2 + 4..off2 + 8].copy_from_slice(&out[12..16]);
    }
}
