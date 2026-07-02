//! CLCL decoder — Cb/Cr per-pixel nibble chroma in separate planes.
//!
//! # Payload layout (planar)
//!
//! ```text
//! [Y0, Y1, ..., Y(N-1), Cb0_Cb1, ..., Cb_{N-2}_Cb_{N-1}, Cr0_Cr1, ..., Cr_{N-2}_Cr_{N-1}]
//! ```
//!
//! * **Y** — 1 byte per pixel (full 8-bit luma).
//! * **Cb** — 1 nibble per pixel, packed 2 nibbles per byte (N/2 bytes total).
//!   Byte packing: `byte[k] = (Cb_{2k+1} << 4) | Cb_{2k}` (odd pixel in high nibble).
//! * **Cr** — Same packing scheme as Cb (N/2 bytes).
//! * Each nibble is upscaled to 8-bit by `<< 4`.
//!
//! In total: N (Y) + N/2 (Cb) + N/2 (Cr) = 2N bytes.
//!
//! Example for 4 pixels:
//!
//! ```text
//! Byte 0: Y0
//! Byte 1: Y1
//! Byte 2: Y2
//! Byte 3: Y3
//! Byte 4: (Cb1 << 4) | Cb0   // Cb nibbles for pixels 0,1
//! Byte 5: (Cb3 << 4) | Cb2   // Cb nibbles for pixels 2,3
//! Byte 6: (Cr1 << 4) | Cr0   // Cr nibbles for pixels 0,1
//! Byte 7: (Cr3 << 4) | Cr2   // Cr nibbles for pixels 2,3
//! ```
//!
//! Output is BGRA 8-bit per channel (via BT.601 YUV→RGB conversion).

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use crate::yuv;

/// Decode a CLCL (separate Cb/Cr nibble-plane) frame to BGRA8 output.
///
/// # Arguments
///
/// * `src` — Raw pixel data: `w × h` Y bytes, then `w × h / 2` packed-Cb bytes,
///   then `w × h / 2` packed-Cr bytes.
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
#[allow(clippy::similar_names)]
pub fn decode(src: &[u8], profile: &Profile) -> Result<DecodedImage, DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat("CLCL dimensions must be positive".into()));
    }

    #[allow(clippy::cast_sign_loss)]
    let w = w_i32 as usize;
    #[allow(clippy::cast_sign_loss)]
    let h = h_i32 as usize;

    let pixel_count = w * h;
    let y_len = pixel_count;
    let chroma_len = pixel_count / 2; // N/2 bytes per chroma plane (2 nibbles/byte)
    let expected = y_len + chroma_len + chroma_len;

    if src.len() < expected {
        return Err(DecodeError::BufferTooShort {
            expected,
            actual: src.len(),
        });
    }

    let mut dst = vec![0u8; pixel_count * 4];

    let cb_off = y_len;
    let cr_off = y_len + chroma_len;

    for i in 0..pixel_count {
        let y = src[i];
        let cbi = src[cb_off + i / 2];
        let cri = src[cr_off + i / 2];

        // Even pixel → low nibble, odd pixel → high nibble
        let n_cb = if i & 1 == 0 { cbi & 0x0F } else { cbi >> 4 };
        let n_cr = if i & 1 == 0 { cri & 0x0F } else { cri >> 4 };

        let pixel = yuv::yuv_to_bgra(y, n_cb << 4, n_cr << 4);
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
            frame_byte_length: (w * h * 2).max(0),
            clcl_chroma: true,
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

    #[test]
    fn buffer_too_short_odd_bytes() {
        // 2×2 = 4 pixels → expected = 4 + 2 + 2 = 8 bytes
        // Provide only 7 bytes
        let profile = make_profile(2, 2);
        let result = decode(&[0u8; 7], &profile);
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 8, actual: 7 })
        ));
    }

    // ---- Neutral chroma (gray output) ----

    #[test]
    fn gray_pixel_neutral_chroma() {
        // Y=128, Cb=8 (neutral after <<4 → 128), Cr=8 (neutral after <<4 → 128)
        // 2 pixels: 2 Y bytes + 1 Cb byte + 1 Cr byte = 4 bytes
        // Both pixels get Cb=(low nibble=8), Cr=(low nibble=8)
        let profile = make_profile(2, 1);
        let img = decode(&[128, 128, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.data[0..4], [128, 128, 128, 255]);
        assert_eq!(img.data[4..8], [128, 128, 128, 255]);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn black_with_neutral_chroma() {
        let profile = make_profile(2, 1);
        let img = decode(&[0, 0, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.data[0..4], [0, 0, 0, 255]);
        assert_eq!(img.data[4..8], [0, 0, 0, 255]);
    }

    #[test]
    fn white_with_neutral_chroma() {
        let profile = make_profile(2, 1);
        let img = decode(&[255, 255, 0x88, 0x88], &profile).unwrap();
        assert_eq!(img.data[0..4], [255, 255, 255, 255]);
        assert_eq!(img.data[4..8], [255, 255, 255, 255]);
    }

    // ---- Chroma nibble unpacking ----

    #[test]
    fn low_nibble_is_first_pixel_cb() {
        // 2 pixels: Cb byte = 0x0F (low=15, high=0), Cr byte = 0x00
        // Pixel 0 (even, low nibble): Cb=15, Cr=0
        //   cb_8bit = 240, cr_8bit = 0
        //   yuv_to_bgra(128, 240, 0):
        //     r = clamp(128 + (-128*359>>8)) = clamp(128 - 180) = 0
        //     g = clamp(128 - (112*88>>8) - (-128*183>>8))
        //       = clamp(128 - 38 + 92) = clamp(182) = 182
        //     b = clamp(128 + (112*454>>8)) = clamp(128 + 198) = clamp(326) = 255
        //   → BGRA [255, 182, 0, 255]
        // Pixel 1 (odd, high nibble): Cb=0, Cr=0
        //   cb_8bit = 0, cr_8bit = 0
        //   yuv_to_bgra(128, 0, 0):
        //     r = clamp(128 + (-128*359>>8)) = clamp(128 - 180) = 0
        //     g = clamp(128 - (-128*88>>8) - (-128*183>>8))
        //       = clamp(128 + 44 + 92) = clamp(264) = 255
        //     b = clamp(128 + (-128*454>>8)) = clamp(128 - 227) = 0
        //   → BGRA [0, 255, 0, 255]
        let profile = make_profile(2, 1);
        let img = decode(&[128, 128, 0x0F, 0x00], &profile).unwrap();
        // Pixel 0: Cb=15(hi), Cr=0(lo) → blue-ish
        assert_eq!(img.data[0..4], [255, 182, 0, 255], "pixel 0 low nibble Cb=15 Cr=0");
        // Pixel 1: Cb=0(hi), Cr=0(hi) → green-ish
        assert_eq!(img.data[4..8], [0, 255, 0, 255], "pixel 1 high nibble Cb=0 Cr=0");
    }

    #[test]
    fn high_nibble_is_second_pixel_chroma() {
        // Cb byte = 0xF0 (low=0, high=15), Cr byte = 0xF0 (low=0, high=15)
        // Pixel 0 (even, low nibble): Cb=0, Cr=0 → green cast
        // Pixel 1 (odd, high nibble): Cb=15, Cr=15 → blue+red cast
        let profile = make_profile(2, 1);
        let img = decode(&[128, 128, 0xF0, 0xF0], &profile).unwrap();
        // Pixel 0: Cb=0, Cr=0 → green
        assert_eq!(img.data[0..4], [0, 255, 0, 255], "pixel 0 low nibble Cb=0 Cr=0");
        // Pixel 1: Cb=15, Cr=15 → Cb8bit=240, Cr8bit=240
        // yuv_to_bgra(128, 240, 240):
        //   r = clamp(128 + (112*359>>8)) = 128 + 157 = 255
        //   g = clamp(128 - (112*88>>8) - (112*183>>8)) = 128 - 38 - 80 = 10
        //   b = clamp(128 + (112*454>>8)) = 128 + 198 = 255
        // → [255, 10, 255, 255]
        assert_eq!(img.data[4..8], [255, 10, 255, 255], "pixel 1 high nibble Cb=15 Cr=15");
    }

    // ---- Multi-pixel decode ----

    #[test]
    fn two_by_two_grid_all_gray() {
        // 2×2 image, all Y=128, all Cb=8, Cr=8 → all gray [128,128,128,255]
        // 4 pixels: 4 Y bytes + 2 Cb bytes + 2 Cr bytes = 8 bytes
        let profile = make_profile(2, 2);
        let img = decode(
            &[
                128, 128, 128, 128, // Y plane (4 bytes)
                0x88, 0x88, // Cb plane (2 bytes): each (8<<4)|8 = neutral
                0x88, 0x88, // Cr plane (2 bytes): neutral
            ],
            &profile,
        )
        .unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        let expected = [128u8, 128, 128, 255];
        for y in 0..2 {
            for x in 0..2 {
                let off = (y * 2 + x) * 4;
                assert_eq!(img.data[off..off + 4], expected, "pixel ({x},{y}) mismatch");
            }
        }
    }

    #[test]
    fn two_by_two_with_varying_y() {
        // Pixel pattern:
        // [Y=255] [Y=128]
        // [Y=64]  [Y=0]
        // All Cb=8, Cr=8 (neutral)
        let profile = make_profile(2, 2);
        let img = decode(
            &[
                255, 128, 64, 0, // Y plane (4 bytes)
                0x88, 0x88, // Cb plane
                0x88, 0x88, // Cr plane
            ],
            &profile,
        )
        .unwrap();
        // Pixel (0,0): Y=255, neutral → white
        assert_eq!(img.data[0..4], [255, 255, 255, 255]);
        // Pixel (1,0): Y=128, neutral → gray
        assert_eq!(img.data[4..8], [128, 128, 128, 255]);
        // Pixel (0,1): Y=64, neutral → dark gray
        assert_eq!(img.data[8..12], [64, 64, 64, 255]);
        // Pixel (1,1): Y=0, neutral → black
        assert_eq!(img.data[12..16], [0, 0, 0, 255]);
    }

    // ---- Chroma is shared across pixel pairs ----

    #[test]
    fn pixel_pair_shares_chroma() {
        // 4 pixels sharing chroma across pairs:
        // Pixels 0,1 share Cb=8, Cr=15 → same byte but diff nibbles
        // Pixels 2,3 share Cb=0, Cr=0
        //
        // Cb bytes: [(8<<4)|8, (0<<4)|0] = [0x88, 0x00]
        // Cr bytes: [(15<<4)|15, (0<<4)|0] = [0xFF, 0x00]
        //
        // Pixel 0: Y=255, Cb=128, Cr=240
        //   yuv_to_bgra(255, 128, 240):
        //     r = 255 + 112*359/256 = 255 + 157 = 255
        //     g = 255 - 0 - 80 = 175
        //     b = 255 + 0 = 255
        //   → [255, 175, 255, 255]
        //
        // Pixel 1: Same chroma as pixel 0, Y=200
        //   yuv_to_bgra(200, 128, 240):
        //     r = 200 + 157 = 255
        //     g = 200 - 80 = 120
        //     b = 200 + 0 = 200
        //   → [200, 120, 255, 255]
        //
        // Pixel 2: Y=100, Cb=0, Cr=0
        //   yuv_to_bgra(100, 0, 0):
        //     r = 100 - 180 = 0
        //     g = 100 + 44 + 92 = 236
        //     b = 100 - 227 = 0
        //   → [0, 236, 0, 255]
        //
        // Pixel 3: Y=50, Cb=0, Cr=0
        //   yuv_to_bgra(50, 0, 0):
        //     r = 50 - 180 = 0
        //     g = 50 + 44 + 92 = 186
        //     b = 50 - 227 = 0
        //   → [0, 186, 0, 255]
        let profile = make_profile(4, 1);
        let img = decode(
            &[
                255, 200, 100, 50, // Y plane (4 bytes)
                0x88, 0x00, // Cb plane (2 bytes)
                0xFF, 0x00, // Cr plane (2 bytes)
            ],
            &profile,
        )
        .unwrap();
        assert_eq!(img.data[0..4], [255, 175, 255, 255], "pixel 0");
        assert_eq!(img.data[4..8], [200, 120, 255, 255], "pixel 1");
        assert_eq!(img.data[8..12], [0, 236, 0, 255], "pixel 2");
        assert_eq!(img.data[12..16], [0, 186, 0, 255], "pixel 3");
    }

    // ---- Nibble edge values ----

    #[test]
    fn chroma_nibbles_at_extremes() {
        // Cb=15, Cr=15 in both nibbles → Cb byte=0xFF, Cr byte=0xFF
        // Both pixels get Cb_8bit=240, Cr_8bit=240
        // yuv_to_bgra(255, 240, 240):
        //   g = 255 - (112*88>>8) - (112*183>>8) = 255 - 38 - 80 = 137
        let profile = make_profile(2, 1);
        let img = decode(&[255, 255, 0xFF, 0xFF], &profile).unwrap();
        assert_eq!(img.data[0..4], [255, 137, 255, 255]);
        assert_eq!(img.data[4..8], [255, 137, 255, 255]);
    }

    #[test]
    fn chroma_nibbles_at_minimum() {
        // Cb=0, Cr=0 → Cb byte=0x00, Cr byte=0x00
        // yuv_to_bgra(128, 0, 0):
        //   r = clamp(128 - 180) = 0
        //   g = clamp(128 + 44 + 92) = clamp(264) = 255
        //   b = clamp(128 - 227) = 0
        // → [0, 255, 0, 255]
        let profile = make_profile(2, 1);
        let img = decode(&[128, 128, 0x00, 0x00], &profile).unwrap();
        assert_eq!(img.data[0..4], [0, 255, 0, 255]);
    }

    // ---- Planar indexing: verify planes don't alias ----

    #[test]
    fn cb_and_cr_planes_are_separate() {
        // Pixel 0: Y=128, Cb=15, Cr=0  → Cb byte low nibble=15, Cr byte low nibble=0
        // Pixel 1: Y=128, Cb=0,  Cr=15 → Cb byte high nibble=0, Cr byte high nibble=15
        // Cb byte = (0 << 4) | 15 = 0x0F
        // Cr byte = (15 << 4) | 0 = 0xF0
        let profile = make_profile(2, 1);
        let img = decode(&[128, 128, 0x0F, 0xF0], &profile).unwrap();
        // Pixel 0: Cb=15, Cr=0 → blue (already tested above)
        assert_eq!(img.data[0..4], [255, 182, 0, 255], "pixel 0 Cb=15 Cr=0");
        // Pixel 1: Cb=0, Cr=15 → red
        // yuv_to_bgra(128, 0, 240):
        //   r = 128 + (112*359>>8) = 128 + 157 = 255
        //   g = 128 - (-128*88>>8) - (112*183>>8) = 128 + 44 - 80 = 92
        //   b = 128 + (-128*454>>8) = 128 - 227 = 0
        // → [0, 92, 255, 255]
        assert_eq!(img.data[4..8], [0, 92, 255, 255], "pixel 1 Cb=0 Cr=15");
    }

    // ---- 2x2 image decode (required acceptance test) ----

    #[test]
    fn two_by_two_image_decode() {
        // 2×2 grid with varying Y and chroma
        // Pixel layout (row-major):
        //   (0,0): Y=128, Cb=8,  Cr=8  → gray
        //   (1,0): Y=200, Cb=15, Cr=0  → blue-ish
        //   (0,1): Y=64,  Cb=0,  Cr=15 → red-ish
        //   (1,1): Y=255, Cb=15, Cr=15 → magenta-ish
        //
        // Y bytes: [128, 200, 64, 255]
        // Cb bytes: pair0(0,1)=(Cb1<<4)|Cb0=(0<<4)|15=0x0F? No...
        //   Actually, let me do this more carefully:
        //   Pixels are indexed 0,1,2,3 (row-major for 2×2).
        //   Pixel pairs: (0,1) and (2,3)
        //   Cb_0 for pixels 0,1 = 8, 15 → low nibble = 8 (for pixel 0)
        //   Cb_1 for pixels 2,3 = 0, 15 → ... wait
        //
        // Actually, let me re-index:
        //   Pixel 0 = (0,0): Y=128, Cb=8, Cr=8
        //   Pixel 1 = (1,0): Y=200, Cb=15, Cr=0
        //   Pixel 2 = (0,1): Y=64, Cb=0, Cr=15
        //   Pixel 3 = (1,1): Y=255, Cb=15, Cr=15
        //
        // Pixel pairs: (0,1) and (2,3) — because pairs are (2k, 2k+1)
        //
        // Cb byte 0 (for pixel pair 0 = pixels 0,1):
        //   low nibble = Cb for pixel 0 = 8
        //   high nibble = Cb for pixel 1 = 15
        //   = (15 << 4) | 8 = 0xF8
        //
        // Cb byte 1 (for pixel pair 1 = pixels 2,3):
        //   low nibble = Cb for pixel 2 = 0
        //   high nibble = Cb for pixel 3 = 15
        //   = (15 << 4) | 0 = 0xF0
        //
        // Cr byte 0: low nibble = Cr for pixel 0 = 8, high nibble = Cr for pixel 1 = 0
        //   = (0 << 4) | 8 = 0x08
        //
        // Cr byte 1: low nibble = Cr for pixel 2 = 15, high nibble = Cr for pixel 3 = 15
        //   = (15 << 4) | 15 = 0xFF
        let profile = make_profile(2, 2);
        let img = decode(
            &[
                // Y plane
                128, 200, 64, 255, // Cb plane
                0xF8, 0xF0, // Cr plane
                0x08, 0xFF,
            ],
            &profile,
        )
        .unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);

        // Pixel 0: Y=128, Cb=8, Cr=8 → gray
        assert_eq!(img.data[0..4], [128, 128, 128, 255]);
        // Pixel 1: Y=200, Cb=15, Cr=0 → blue cast
        // yuv_to_bgra(200, 240, 0):
        //   r = 200 + (-128*359>>8) = 200 - 180 = 20
        //   g = 200 - (112*88>>8) - (-128*183>>8) = 200 - 38 + 92 = 254
        //   b = 200 + (112*454>>8) = 200 + 198 = 255
        // → [255, 254, 20, 255]
        assert_eq!(img.data[4..8], [255, 254, 20, 255]);
        // Pixel 2: Y=64, Cb=0, Cr=15 → red cast
        // yuv_to_bgra(64, 0, 240):
        //   r = 64 + (112*359>>8) = 64 + 157 = 221
        //   g = 64 - (-128*88>>8) - (112*183>>8) = 64 + 44 - 80 = 28
        //   b = 64 + (-128*454>>8) = 64 - 227 = 0
        // → [0, 28, 221, 255]
        assert_eq!(img.data[8..12], [0, 28, 221, 255]);
        // Pixel 3: Y=255, Cb=15, Cr=15 → magenta cast
        // yuv_to_bgra(255, 240, 240):
        //   g = 255 - (112*88>>8) - (112*183>>8) = 255 - 38 - 80 = 137
        // → [255, 137, 255, 255]
        assert_eq!(img.data[12..16], [255, 137, 255, 255]);
    }
}
