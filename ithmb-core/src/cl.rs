//! CL (per-pixel nibble chroma) decoder — also called "chunky" or "per-pixel" chroma.
//!
//! # Payload layout (planar)
//!
//! ```text
//! [Y0, Y1, ..., Y(N-1), CbCr0, CbCr1, ..., CbCr(N-1)]
//! ```
//!
//! * **Y** — 1 byte per pixel (full 8-bit luma).
//! * **`CbCr`** — 1 byte per pixel, packed nibbles: high nibble = Cr, low nibble = Cb.
//!   Each chroma nibble is 4-bit (0–15) and upscaled to 8-bit by `<< 4`.
//!
//! In total: 2 bytes per pixel × N pixels.
//!
//! Example for 2 pixels:
//!
//! ```text
//! Byte 0: Y0
//! Byte 1: Y1
//! Byte 2: (Cr0 << 4) | Cb0
//! Byte 3: (Cr1 << 4) | Cb1
//! ```
//!
//! Output is BGRA 8-bit per channel (via BT.601 YUV→RGB conversion).

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use crate::yuv;

/// Decode a CL (per-pixel nibble chroma) frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data: `w × h` Y bytes followed by `w × h` `CbCr` bytes.
/// * `profile` — The profile describing this frame's dimensions.
///
/// # Returns
///
/// `Ok(DecodedImage)` on success, or a [`DecodeError`] on failure.
///
/// # Errors
///
/// * [`DecodeError::InvalidFormat`] — width or height ≤ 0.
/// * [`DecodeError::BufferTooShort`] — input too small for the declared dimensions.
///
/// # Panics
///
/// Never panics.
pub fn decode(src: &[u8], profile: &Profile) -> Result<DecodedImage, DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat("CL dimensions must be positive".into()));
    }

    #[allow(clippy::cast_sign_loss)]
    let w = w_i32 as usize;
    #[allow(clippy::cast_sign_loss)]
    let h = h_i32 as usize;

    let pixel_count = w * h;
    let expected = pixel_count * 2; // Y bytes + CbCr bytes

    if src.len() < expected {
        return Err(DecodeError::BufferTooShort {
            expected,
            actual: src.len(),
        });
    }

    let y_plane_end = pixel_count;
    let mut dst = vec![0u8; pixel_count * 4];

    for i in 0..pixel_count {
        let y = src[i]; // full 8-bit Y
        let chroma = src[y_plane_end + i];
        let cr = chroma >> 4; // high nibble → Cr (4-bit)
        let cb = chroma & 0x0F; // low nibble → Cb (4-bit)

        let pixel = yuv::yuv_to_bgra(y, cb << 4, cr << 4);
        let dst_idx = i * 4;
        dst[dst_idx..dst_idx + 4].copy_from_slice(&pixel);
    }

    #[allow(clippy::cast_possible_truncation)]
    let out_w = w as u32;
    #[allow(clippy::cast_possible_truncation)]
    let out_h = h as u32;

    Ok(DecodedImage {
        data: dst,
        width: out_w,
        height: out_h,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Encoding;

    fn make_profile(w: i32, h: i32) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::Rgb565,
            frame_byte_length: w * h * 2,
            cl_chroma: true,
            ..Default::default()
        }
    }

    // ---- Error paths ----

    #[test]
    fn zero_width_returns_invalid_format() {
        let profile = make_profile(0, 100);
        let result = decode(b"", &profile);
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn zero_height_returns_invalid_format() {
        let profile = make_profile(100, 0);
        let result = decode(b"", &profile);
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn negative_width_returns_invalid_format() {
        let profile = make_profile(-1, 100);
        let result = decode(b"", &profile);
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn buffer_too_short_returns_error() {
        let profile = make_profile(10, 10);
        // 10*10*2 = 200 bytes needed, only 10 provided
        let result = decode(&[0u8; 10], &profile);
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort {
                expected: 200,
                actual: 10
            })
        ));
    }

    // ---- Neutral chroma (gray output) ----

    #[test]
    fn gray_pixel_neutral_chroma() {
        // Y=128, Cb=8 (neutral after <<4 → 128), Cr=8 (neutral after <<4 → 128)
        // chroma_byte = (Cr << 4) | Cb = (8 << 4) | 8 = 0x88
        let profile = make_profile(1, 1);
        let img = decode(&[128, 0x88], &profile).unwrap();
        assert_eq!(img.data, [128, 128, 128, 255]);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn black_with_neutral_chroma() {
        // Y=0, Cb=8, Cr=8 → BGRA [0, 0, 0, 255]
        let profile = make_profile(1, 1);
        let img = decode(&[0, 0x88], &profile).unwrap();
        assert_eq!(img.data, [0, 0, 0, 255]);
    }

    #[test]
    fn white_with_neutral_chroma() {
        // Y=255, Cb=8, Cr=8 → BGRA [255, 255, 255, 255]
        let profile = make_profile(1, 1);
        let img = decode(&[255, 0x88], &profile).unwrap();
        assert_eq!(img.data, [255, 255, 255, 255]);
    }

    // ---- Chroma nibble unpacking ----

    #[test]
    fn chroma_nibble_high_is_cr_low_is_cb() {
        // chroma_byte = 0xF0 → cr=15, cb=0
        // cr_8bit = 240, cb_8bit = 0
        // yuv_to_bgra(128, 0, 240) where cb=-128, cr=112
        //   r = clamp(128 + (112*359>>8)) = clamp(128 + 157) = clamp(285) = 255
        //   g = clamp(128 - (-128*88>>8) - (112*183>>8))
        //     = clamp(128 - (-44) - 80) = clamp(92) = 92
        //   b = clamp(128 + (-128*454>>8)) = clamp(128 + (-227)) = clamp(-99) = 0
        // → BGRA [0, 92, 255, 255]
        let profile = make_profile(1, 1);
        let img = decode(&[128, 0xF0], &profile).unwrap();
        assert_eq!(img.data, [0, 92, 255, 255], "high nibble CB=0, CR=15");
    }

    #[test]
    fn chroma_nibble_low_is_cb_high_is_cr() {
        // chroma_byte = 0x0F → cr=0, cb=15
        // cb_8bit = 240, cr_8bit = 0
        // yuv_to_bgra(128, 240, 0) where cb=112, cr=-128
        //   r = clamp(128 + (-128*359>>8)) = clamp(128 - 180) = 0
        //   g = clamp(128 - (112*88>>8) - (-128*183>>8))
        //     = clamp(128 - 38 - (-92)) = clamp(182) = 182
        //   b = clamp(128 + (112*454>>8)) = clamp(128 + 198) = clamp(326) = 255
        // → BGRA [255, 182, 0, 255]
        let profile = make_profile(1, 1);
        let img = decode(&[128, 0x0F], &profile).unwrap();
        assert_eq!(img.data, [255, 182, 0, 255], "low nibble CB=15, CR=0");
    }

    // ---- Multi-pixel decode ----

    #[test]
    fn two_pixels_planar_layout() {
        // Pixel 0: Y=128, Cb=8, Cr=8 → gray [128,128,128,255]
        // Pixel 1: Y=0,   Cb=8, Cr=8 → black [0,0,0,255]
        // Planar: [Y0=128, Y1=0, CbCr0=0x88, CbCr1=0x88]
        let profile = make_profile(2, 1);
        let img = decode(&[128, 0, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(
            img.data,
            [
                128, 128, 128, 255, // pixel 0
                0, 0, 0, 255, // pixel 1
            ]
        );
    }

    #[test]
    fn two_by_two_grid() {
        // 2×2 image, all Y=128, all Cb=8, Cr=8 → all gray
        // Planar: [128,128,128,128, 0x88,0x88,0x88,0x88]
        let profile = make_profile(2, 2);
        let img = decode(&[128, 128, 128, 128, 0x88, 0x88, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        let expected = vec![128u8, 128, 128, 255]; // one gray pixel
        for y in 0..2 {
            for x in 0..2 {
                let off = (y * 2 + x) * 4;
                assert_eq!(img.data[off..off + 4], expected, "pixel ({x},{y}) mismatch");
            }
        }
    }

    // ---- Nibble edge values ----

    #[test]
    fn chroma_nibbles_at_extremes() {
        // Full crayon-mode: Cb=15 (max), Cr=15 (max)
        // chroma_byte = (15 << 4) | 15 = 0xFF
        // cb_8bit = 240, cr_8bit = 240
        // yuv_to_bgra(255, 240, 240):
        //   g = 255 - (112*88>>8) - (112*183>>8) = 255 - 38 - 80 = 137
        let profile = make_profile(1, 1);
        let img = decode(&[255, 0xFF], &profile).unwrap();
        assert_eq!(img.data, [255, 137, 255, 255]);
    }

    // ---- Planar indexing: verify Y and chroma planes don't alias ----

    #[test]
    fn different_y_and_chroma_planes() {
        // Pixel 0: Y=100, CbCr=0x88 (Cb=8, Cr=8, neutral)
        // Pixel 1: Y=200, CbCr=0x88 (Cb=8, Cr=8, neutral)
        // The chroma plane starts at pixel_count = 2, so CbCr0 = src[2], CbCr1 = src[3]
        let profile = make_profile(2, 1);
        let img = decode(&[100, 200, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.data[0..4], [100, 100, 100, 255], "pixel 0 uses Y=100");
        assert_eq!(img.data[4..8], [200, 200, 200, 255], "pixel 1 uses Y=200");
    }
}
