//! BT.601 YUV-to-BGRA conversion.
//!
//! Fixed-point integer math matching the C# `IthmbCodecPlugin.YuvUtils`
//! implementation bit-exactly.
//!
//! ## Math (BT.601-7)
//!
//! ```text
//! R = Y + (Cr - 128) ×  359 ÷ 256
//! G = Y - (Cb - 128) ×   88 ÷ 256 - (Cr - 128) × 183 ÷ 256
//! B = Y + (Cb - 128) ×  454 ÷ 256
//! ```
//!
//! Division uses arithmetic right-shift (`>> 8`) to match C# semantics exactly:
//! negative intermediates round toward negative infinity, *not* toward zero.

// ---- BT.601 fixed-point coefficients (ITU-R BT.601-7) ----

/// Cr → R coefficient: 1.402 × 256 = 359 (truncated).
pub const R_COEF: i32 = 359;

/// Cb → G coefficient: −0.344 × 256 = −88 (truncated magnitude, sign handled
/// in the expression as `- ((cb_s * G_COEF_CB) >> 8)` per the C# source).
pub const G_COEF_CB: i32 = 88;

/// Cr → G coefficient: −0.714 × 256 = −183 (truncated magnitude).
pub const G_COEF_CR: i32 = 183;

/// Cb → B coefficient: 1.772 × 256 = 454 (truncated).
pub const B_COEF: i32 = 454;

/// Clamp an [`i32`] to the inclusive `[0, 255]` range.
///
/// # Panics
///
/// Never panics.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn clamp(v: i32) -> u8 {
    crate::pixel_utils::clamp_u8(v)
}

/// Convert a single BT.601 Y′CbCr triad to BGRA 8-bit.
///
/// # Arguments
///
/// * `y`  — Luma component (0–255).
/// * `cb` — Blue-difference chroma (0–255).
/// * `cr` — Red-difference chroma (0–255).
///
/// # Returns
///
/// `[b, g, r, 255]` — BGRA pixel data.
///
/// # Panics
///
/// Never panics.
///
/// # Bit-exactness
///
/// Every arithmetic step matches the C# reference:
///
/// ```csharp
/// int r = Clamp(luma + ((YuvRCoef * cr) >> 8));
/// // … etc.
/// ```
#[inline]
#[must_use]
pub fn yuv_to_bgra(y: u8, cb: u8, cr: u8) -> [u8; 4] {
    let y = i32::from(y);
    let cb = i32::from(cb) - 128;
    let cr = i32::from(cr) - 128;

    let r = clamp(y + ((cr * R_COEF) >> 8));
    let g = clamp(y - ((cb * G_COEF_CB) >> 8) - ((cr * G_COEF_CR) >> 8));
    let b = clamp(y + ((cb * B_COEF) >> 8));

    [b, g, r, 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Gray / neutral-chroma cases ----

    #[test]
    fn gray_mid() {
        // yuv_to_bgra(128, 128, 128) → [128, 128, 128, 255]
        assert_eq!(yuv_to_bgra(128, 128, 128), [128, 128, 128, 255]);
    }

    #[test]
    fn white_full() {
        // yuv_to_bgra(255, 128, 128) → [255, 255, 255, 255]
        assert_eq!(yuv_to_bgra(255, 128, 128), [255, 255, 255, 255]);
    }

    #[test]
    fn black_full() {
        // yuv_to_bgra(0, 128, 128) → [0, 0, 0, 255]
        assert_eq!(yuv_to_bgra(0, 128, 128), [0, 0, 0, 255]);
    }

    // ---- Chroma-driven cases ----

    #[test]
    fn saturated_blue_positive() {
        // Cb = 255 (max blue excursion), Cr = 128 (neutral).
        //
        //   r = 128 + 0          = 128
        //   g = 128 - 127·88/256 = 128 - 43 = 85
        //   b = 128 + 127·454/256 = 128 + 225 = 353 → clamp 255
        assert_eq!(yuv_to_bgra(128, 255, 128), [255, 85, 128, 255]);
    }

    #[test]
    fn saturated_red_positive() {
        // Cr = 255 (max red excursion), Cb = 128 (neutral).
        //
        //   r = 128 + 127·359/256 = 128 + 178 = 306 → clamp 255
        //   g = 128 - 0 - 127·183/256 = 128 - 90 = 38
        //   b = 128 + 0 = 128
        assert_eq!(yuv_to_bgra(128, 128, 255), [128, 38, 255, 255]);
    }

    // ---- Clamping edge cases ----

    #[test]
    fn clamp_negative_yields_zero() {
        // Y = 0, Cb = 0 (Cr-128 = -128 → pushes R/G negative).
        // r = 0 + (-128)·359/256 = 0 + (-180) = -180 → clamp 0
        // g = 0 - (-128)·88/256 - (-128)·183/256 = 0 - (-44) - (-92) = 136
        // b = 0 + (-128)·454/256 = -227 → clamp 0
        let pixel = yuv_to_bgra(0, 0, 0);
        assert_eq!(pixel[0], 0, "b channel must clamp to 0"); // b
        assert_eq!(pixel[2], 0, "r channel must clamp to 0"); // r
    }

    #[test]
    fn max_chroma_does_not_overflow_green() {
        // Y = 255, Cb = 255, Cr = 255.
        // G channel: 255 - 127·88/256 - 127·183/256 = 255 - 43 - 90 = 122.
        // R and B saturate at 255.
        assert_eq!(yuv_to_bgra(255, 255, 255), [255, 122, 255, 255]);
    }

    // ---- Boundary / corner cases ----

    #[test]
    fn neutral_chroma_various_luma() {
        for y in [0u8, 16, 128, 235, 255] {
            let pixel = yuv_to_bgra(y, 128, 128);
            assert_eq!(pixel, [y, y, y, 255], "neutral chroma must yield gray");
        }
    }

    #[test]
    fn clamp_single_values() {
        assert_eq!(clamp(-1), 0);
        assert_eq!(clamp(0), 0);
        assert_eq!(clamp(128), 128);
        assert_eq!(clamp(255), 255);
        assert_eq!(clamp(256), 255);
        assert_eq!(clamp(i32::MIN), 0);
        assert_eq!(clamp(i32::MAX), 255);
    }

    #[test]
    fn bgra_output_alpha_is_always_255() {
        for y in [0u8, 128, 255] {
            for cb in [0u8, 128, 255] {
                for cr in [0u8, 128, 255] {
                    let pixel = yuv_to_bgra(y, cb, cr);
                    assert_eq!(
                        pixel[3], 255,
                        "alpha channel must always be 255 (y={y}, cb={cb}, cr={cr})"
                    );
                }
            }
        }
    }

    // ---- BT.601 white-point sanity ----

    #[test]
    fn broadcast_white() {
        // Y = 235 (broadcast white), Cb = Cr = 128 (neutral).
        // R = G = B = 235.
        assert_eq!(yuv_to_bgra(235, 128, 128), [235, 235, 235, 255]);
    }

    // ---- Known mid-scale values ----

    /// Returns `true` when values compare equal.
    fn yuv_roundtrip_gray(y: u8) -> bool {
        yuv_to_bgra(y, 128, 128)[..3] == [y, y, y]
    }

    #[test]
    fn every_gray_value_roundtrips() {
        // For every luma value with neutral chroma, the RGB output must be
        // the neutral gray [y, y, y].
        for y in 0..=255u8 {
            assert!(yuv_roundtrip_gray(y), "gray roundtrip failed at y={y}");
        }
    }
}
