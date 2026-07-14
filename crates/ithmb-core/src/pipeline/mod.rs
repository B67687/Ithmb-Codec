//! Pipeline module — central dispatch for .ithmb decoding.
//!
//! This is the most important module in the library. It orchestrates all decoders:
//! reads the format prefix, looks up the decoding profile, dispatches to the
//! correct decoder function, and applies post-processing (crop, rotation).
//!
//! The module is split into sub-modules:
//!
//! * `profile_loader` - one-time initialization of the built-in profile DB
//! * `open` - `PhotoDB` / `ArtworkDB` multi-frame container opening

mod open;
mod profile_loader;

pub use self::open::open_ithmb;

use self::profile_loader::fallback_jpeg_profile;
pub(crate) use self::profile_loader::get_db;
use crate::cl;
use crate::clcl;
use crate::error::{DecodeError, DecodedImage};
use crate::jpeg;
use crate::profile::{Encoding, Profile};
use crate::reordered_rgb555;
use crate::rgb555;
use crate::rgb565;
use crate::uyvy;
use crate::ycbcr420;
use std::sync::atomic::AtomicBool;

/// Look up the human-readable encoding name for a given format prefix.
/// Returns `"Unknown format"` if the prefix is not found in the built-in profiles.
#[must_use]
pub fn encoding_name_for_prefix(prefix: i32) -> String {
    let db = get_db();
    match db.get(prefix) {
        Some(profile) => profile.encoding.to_display_string().to_string(),
        None => "Unknown format".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode a complete `.ithmb` file from its raw content bytes.
///
/// This is the top-level entry point. It:
/// 1. Reads the 4-byte big-endian format prefix.
/// 2. Detects JPEG-embedded streams by checking for the SOI marker (`FF D8`).
/// 3. Looks up the decoding profile from the built-in database.
/// 4. Falls back to a JPEG-compatible profile when a JPEG stream is detected
///    but the prefix is unknown.
/// 5. Dispatches to the correct decoder.
/// 6. Applies post-processing (dimension swap, crop, rotation).
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `BufferTooShort` | Input is smaller than 4 bytes. |
/// | `Unsupported` | The format prefix does not match any known profile |
/// | | and the data is not a JPEG stream. |
/// | Decoder errors | Propagated from the underlying decoder. |
pub fn decode_ithmb(src: &[u8], canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    if src.len() < 4 {
        return Err(DecodeError::BufferTooShort {
            expected: 4,
            actual: src.len(),
        });
    }

    if src.len() > open::MAX_RAW_FILE_SIZE {
        return Err(DecodeError::FileTooLarge {
            size: src.len(),
            limit: open::MAX_RAW_FILE_SIZE,
        });
    }

    let prefix = i32::from_be_bytes([src[0], src[1], src[2], src[3]]);
    let is_jpeg_stream = src[0] == 0xFF && src[1] == 0xD8;

    let db = get_db();

    let profile = if is_jpeg_stream {
        db.get(prefix).cloned().unwrap_or_else(fallback_jpeg_profile)
    } else {
        db.get(prefix)
            .ok_or_else(|| DecodeError::Unsupported(format!("unknown format prefix {prefix}")))?
            .clone()
    };

    decode_with_profile(src, &profile, canceled)
}

/// Decode an `.ithmb` file using an explicit profile, bypassing prefix-lookup.
///
/// This is useful when the caller already knows the profile (e.g. from `PhotoDB`
/// metadata, or for testing with synthetic data).
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] if the input is too short for the
/// expected prefix (4 bytes for raw formats). Propagates decoder errors.
pub fn decode_with_profile(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    // stream). Raw formats have a 4-byte format prefix before pixel data.
    let frame_data = if profile.encoding == Encoding::Jpeg {
        src
    } else {
        if src.len() < 4 {
            return Err(DecodeError::BufferTooShort {
                expected: 4,
                actual: src.len(),
            });
        }
        &src[4..]
    };

    let img = dispatch_decode(frame_data, profile, canceled)?;
    Ok(apply_post_process(img, profile))
}

// ---------------------------------------------------------------------------
// Decoder dispatch
// ---------------------------------------------------------------------------

/// Dispatches to the correct decoder based on the profile's encoding.
fn dispatch_decode(data: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    match profile.encoding {
        Encoding::Rgb565 => rgb565::decode(data, profile, canceled),
        Encoding::Rgb555 => rgb555::decode(data, profile, canceled),
        Encoding::ReorderedRgb555 => reordered_rgb555::decode(data, profile, canceled),
        Encoding::Yuv422 => {
            if profile.clcl_chroma {
                clcl::decode(data, profile, canceled)
            } else if profile.cl_chroma {
                cl::decode(data, profile, canceled)
            } else {
                uyvy::decode(data, profile, canceled)
            }
        }
        Encoding::Ycbcr420 => ycbcr420::decode(data, profile, canceled),
        Encoding::Jpeg => jpeg::decode(data, profile, canceled),
    }
}

// ---------------------------------------------------------------------------
// Post-processing
// ---------------------------------------------------------------------------

/// Applies dimension swap, crop, and rotation in that order.
fn apply_post_process(mut img: DecodedImage, profile: &Profile) -> DecodedImage {
    // 1. Swap display dimensions if the profile requests it.
    if profile.swaps_dimensions {
        std::mem::swap(&mut img.width, &mut img.height);
    }

    // 2. Crop to the visible region.
    img = apply_crop(img, profile);

    // 3. Rotate according to the profile's rotation field.
    apply_rotation(img, profile)
}

/// Crops the image to the region specified by the profile.
///
/// When `crop_width` or `crop_height` is 0 the remaining span from the
/// corresponding offset is used. All values are clamped to the image bounds.
fn apply_crop(img: DecodedImage, profile: &Profile) -> DecodedImage {
    let needs_crop = profile.crop_x != 0 || profile.crop_y != 0 || profile.crop_width != 0 || profile.crop_height != 0;

    if !needs_crop {
        return img;
    }

    #[allow(clippy::cast_sign_loss)]
    let cx = profile.crop_x.max(0) as usize;
    #[allow(clippy::cast_sign_loss)]
    let cy = profile.crop_y.max(0) as usize;
    let iw = img.width as usize;
    let ih = img.height as usize;

    #[allow(clippy::cast_sign_loss)]
    let cw = if profile.crop_width > 0 {
        profile.crop_width as usize
    } else {
        iw.saturating_sub(cx)
    };

    #[allow(clippy::cast_sign_loss)]
    let ch = if profile.crop_height > 0 {
        profile.crop_height as usize
    } else {
        ih.saturating_sub(cy)
    };

    // Clamp to image bounds.
    let cw = cw.min(iw.saturating_sub(cx));
    let ch = ch.min(ih.saturating_sub(cy));

    if cw == 0 || ch == 0 {
        return img;
    }

    let mut cropped = Vec::with_capacity(cw * ch * 4);
    for y in cy..cy + ch {
        let row_start = (y * iw + cx) * 4;
        cropped.extend_from_slice(&img.data[row_start..row_start + cw * 4]);
    }

    #[allow(clippy::cast_possible_truncation)]
    DecodedImage {
        data: cropped,
        width: cw as u32,
        height: ch as u32,
    }
}

/// Applies the rotation specified by the profile.
///
/// Supports 0°, 90°, 180°, and 270° clockwise rotation. Other values are
/// silently ignored.
fn apply_rotation(img: DecodedImage, profile: &Profile) -> DecodedImage {
    match profile.rotation {
        90 => rotate_90_cw(img),
        180 => rotate_180(img),
        270 => rotate_270_cw(img),
        _ => img,
    }
}

#[allow(clippy::needless_pass_by_value, clippy::cast_possible_truncation)]
fn rotate_90_cw(img: DecodedImage) -> DecodedImage {
    let w = img.width as usize;
    let h = img.height as usize;
    let mut rotated = Vec::with_capacity(w * h * 4);

    for x in 0..w {
        for y in (0..h).rev() {
            let src_idx = (y * w + x) * 4;
            rotated.extend_from_slice(&img.data[src_idx..src_idx + 4]);
        }
    }

    DecodedImage {
        data: rotated,
        width: h as u32,
        height: w as u32,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn rotate_180(img: DecodedImage) -> DecodedImage {
    let total_pixels = img.data.len() / 4;
    let mut rotated = vec![0u8; total_pixels * 4];

    for i in 0..total_pixels {
        let src_idx = i * 4;
        let dst_idx = (total_pixels - 1 - i) * 4;
        rotated[dst_idx..dst_idx + 4].copy_from_slice(&img.data[src_idx..src_idx + 4]);
    }

    DecodedImage {
        data: rotated,
        width: img.width,
        height: img.height,
    }
}

#[allow(clippy::needless_pass_by_value, clippy::cast_possible_truncation)]
fn rotate_270_cw(img: DecodedImage) -> DecodedImage {
    let w = img.width as usize;
    let h = img.height as usize;
    let total = w * h * 4;
    let mut rotated = vec![0u8; total];

    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) * 4;
            // 270° CW: old (x, y) -> new (y, w - 1 - x)
            let ox = y;
            let oy = w - 1 - x;
            let dst_idx = (oy * h + ox) * 4;
            rotated[dst_idx..dst_idx + 4].copy_from_slice(&img.data[src_idx..src_idx + 4]);
        }
    }

    DecodedImage {
        data: rotated,
        width: h as u32,
        height: w as u32,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profile::Encoding;
    use std::sync::atomic::AtomicBool;

    // -----------------------------------------------------------------------
    // Helper: build a small test profile for any raw encoding
    // -----------------------------------------------------------------------

    fn small_profile(w: i32, h: i32, encoding: Encoding) -> Profile {
        let bpp = match encoding {
            Encoding::Rgb565 | Encoding::Rgb555 | Encoding::ReorderedRgb555 | Encoding::Yuv422 | Encoding::Ycbcr420 => {
                2
            }
            Encoding::Jpeg => 0,
        };
        Profile {
            prefix: 9999,
            width: w,
            height: h,
            encoding,
            frame_byte_length: w * h * bpp,
            ..Default::default()
        }
    }

    // ---- decode_ithmb errors ----

    #[test]
    fn test_empty_input_returns_buffer_too_short() {
        let result = decode_ithmb(&[], &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 4, actual: 0 })
        ));
    }

    #[test]
    fn test_short_input_returns_buffer_too_short() {
        let result = decode_ithmb(&[0x00, 0x00, 0x00], &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 4, actual: 3 })
        ));
    }

    #[test]
    fn test_unknown_prefix_returns_unsupported() {
        // Prefix 9999 does not exist in the built-in profile DB.
        let buf = [0x00, 0x00, 0x27, 0x0F]; // 9999 in big-endian
        let result = decode_ithmb(&buf, &AtomicBool::new(false));
        assert!(matches!(result, Err(DecodeError::Unsupported(ref msg)) if msg.contains("9999")));
    }

    #[test]
    fn test_jpeg_fallback_profile_is_used() {
        // Buffer starts with JPEG SOI (FF D8) but the prefix -1 does not
        // necessarily exist in the DB. The fallback JPEG profile should be
        // used, so we should NOT get Unsupported.
        let buf = [0xFF, 0xD8, 0x00, 0x00, 0x00];
        let result = decode_ithmb(&buf, &AtomicBool::new(false));
        // The JPEG decoder will be called and should return a Jpeg error
        // (the data is not a valid JPEG stream after the SOI).
        assert!(result.is_err());
        assert!(
            !matches!(result, Err(DecodeError::Unsupported(_))),
            "JPEG SOI input must not return Unsupported"
        );
    }

    // ---- decode_with_profile dispatch ----

    #[test]
    fn test_rgb565_dispatch() {
        let profile = small_profile(2, 1, Encoding::Rgb565);
        // 2 white RGB565 pixels: 0xFFFF LE
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn test_rgb555_dispatch() {
        let profile = small_profile(1, 1, Encoding::Rgb555);
        // White RGB555 pixel: 0x7FFF LE
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0xFF, 0x7F]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn test_reordered_rgb555_dispatch() {
        let profile = small_profile(1, 1, Encoding::ReorderedRgb555);
        // Reordered RGB555 uses little-endian (like all other RGB profiles).
        // White pixel: 0x7FFF in little-endian = [0xFF, 0x7F]
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0xFF, 0x7F]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);
    }

    #[test]
    fn test_uyvy_dispatch() {
        let profile = small_profile(2, 1, Encoding::Yuv422);
        // UYVY: [U=128, Y0=128, V=128, Y1=128] = neutral gray
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[128, 128, 128, 128]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![128, 128, 128, 255, 128, 128, 128, 255]);
    }

    #[test]
    fn test_clcl_dispatch() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 1,
            encoding: Encoding::Yuv422,
            frame_byte_length: 4,
            clcl_chroma: true,
            ..Default::default()
        };
        // CLCL: Y=[128,128], Cb=[0x88], Cr=[0x88]
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[128, 128, 0x88, 0x88]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![128, 128, 128, 255, 128, 128, 128, 255]);
    }

    #[test]
    fn test_cl_dispatch() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 1,
            encoding: Encoding::Yuv422,
            frame_byte_length: 4,
            cl_chroma: true,
            ..Default::default()
        };
        // CL: Y=[128,128], CbCr=[0x88, 0x88]
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[128, 128, 0x88, 0x88]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![128, 128, 128, 255, 128, 128, 128, 255]);
    }

    #[test]
    fn test_ycbcr420_dispatch() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Ycbcr420,
            frame_byte_length: 6, // 4 + 1 + 1
            ..Default::default()
        };
        // YCbCr 4:2:0: Y=[128,128,128,128], Cb=[128], Cr=[128]
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[128u8; 6]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        for chunk in img.data.chunks_exact(4) {
            assert_eq!(chunk, &[128, 128, 128, 255]);
        }
    }

    #[test]
    fn test_jpeg_dispatch_with_decode_with_profile() {
        // Using the built-in JPEG decoder with SOI marker.
        let profile = Profile {
            prefix: -1,
            width: 0,
            height: 0,
            encoding: Encoding::Jpeg,
            use_mhni_dimensions: true,
            ..Default::default()
        };
        // Buffer is the full JPEG stream (no 4-byte prefix needed for JPEG).
        let buf = [0xFF, 0xD8, 0x00, 0x00];
        let result = decode_with_profile(&buf, &profile, &AtomicBool::new(false));
        // The JPEG decoder should be reached; data is invalid so we get Jpeg error.
        assert!(result.is_err());
        assert!(matches!(result, Err(DecodeError::Jpeg(_))));
    }

    // ---- decode_with_profile - buffer too short ----

    #[test]
    fn test_decode_with_profile_too_short_for_prefix() {
        let profile = small_profile(1, 1, Encoding::Rgb565);
        let result = decode_with_profile(&[0x00, 0x00], &profile, &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 4, actual: 2 })
        ));
    }

    // ---- swaps_dimensions ----

    #[test]
    fn test_swaps_dimensions_metadata_only() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 3,
            encoding: Encoding::Rgb565,
            frame_byte_length: 12,
            swaps_dimensions: true,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 6 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // swaps_dimensions: width and height are swapped.
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 6 * 4);
    }

    // ---- Crop ----

    #[test]
    fn test_crop_2x2_to_1x1() {
        // 2×2 RGB565 image: red, green, blue, white
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            crop_x: 0,
            crop_y: 0,
            crop_width: 1,
            crop_height: 1,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        // RGB565 LE: red=0xF800, green=0x07E0, blue=0x001F, white=0xFFFF
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        // Pixel (0,0) = red in RGB565 → BGRA: B=0, G=0, R=255
        assert_eq!(img.data, vec![0, 0, 0xFF, 255]);
    }

    #[test]
    fn test_crop_with_offset() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            crop_x: 1,
            crop_y: 0,
            crop_width: 1,
            crop_height: 1,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        // Pixel (1,0) = green in RGB565 → BGRA: B=0, G=255, R=0
        assert_eq!(img.data, vec![0, 0xFF, 0, 255]);
    }

    #[test]
    fn test_crop_full_dimensions_when_zero() {
        let profile = Profile {
            prefix: 9999,
            width: 3,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 12,
            crop_x: 1,
            crop_y: 0,
            crop_width: 0,  // use remaining
            crop_height: 0, // use remaining
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 6 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // crop_x=1, so crop_width = 3-1 = 2, crop_height = 2-0 = 2
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 2 * 2 * 4);
    }

    #[test]
    fn test_no_crop_when_all_zero() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            crop_x: 0,
            crop_y: 0,
            crop_width: 0,
            crop_height: 0,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 4 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 2 * 2 * 4);
    }

    // ---- Rotation ----

    #[test]
    fn test_rotation_90_cw_on_2x3() {
        // 2×3 RGB565 image with distinct color per pixel.
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 3,
            encoding: Encoding::Rgb565,
            frame_byte_length: 12,
            rotation: 90,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        // Row 0: red (0xF800), green (0x07E0)
        // Row 1: blue (0x001F), yellow (0xFFE0)
        // Row 2: cyan (0x07FF), magenta (0xF81F)
        buf.extend_from_slice(&[
            0x00, 0xF8, 0xE0, 0x07, // row 0
            0x1F, 0x00, 0xE0, 0xFF, // row 1
            0xFF, 0x07, 0x1F, 0xF8, // row 2
        ]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // 90° CW: dimensions swap → 3×2
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);

        // Verify a few pixels. After 90° CW:
        // old(0,0)=red → new(2,0)
        // old(1,0)=green → new(2,1)
        // old(0,2)=cyan → new(0,0)
        assert_eq!(img.data.len(), 3 * 2 * 4);

        // BGRA values for each color:
        //   red:    [0, 0, 255, 255]
        //   green:  [0, 255, 0, 255]
        //   blue:   [255, 0, 0, 255]
        //   yellow: [0, 255, 255, 255]
        //   cyan:   [255, 255, 0, 255]
        //   magenta:[255, 0, 255, 255]

        // new(0,0) = old(0,2) = cyan
        assert_eq!(&img.data[0..4], &[255, 255, 0, 255]);
        // new(2,0) = old(0,0) = red
        assert_eq!(&img.data[8..12], &[0, 0, 255, 255]);
        // new(0,1) = old(1,2) = magenta
        assert_eq!(&img.data[12..16], &[255, 0, 255, 255]);
        // new(2,1) = old(1,0) = green
        assert_eq!(&img.data[20..24], &[0, 255, 0, 255]);
    }

    #[test]
    fn test_rotation_180_on_2x2() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            rotation: 180,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // 180°: pixel order reversed. Dimensions same (2×2).
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);

        // old(0,0)=red   → new(1,1)
        // old(1,0)=green → new(0,1)
        // old(0,1)=blue  → new(1,0)
        // old(1,1)=white → new(0,0)

        // new(0,0) = old(1,1) = white
        assert_eq!(&img.data[0..4], &[255, 255, 255, 255]);
        // new(1,0) = old(0,1) = blue
        assert_eq!(&img.data[4..8], &[255, 0, 0, 255]);
        // new(0,1) = old(1,0) = green
        assert_eq!(&img.data[8..12], &[0, 255, 0, 255]);
        // new(1,1) = old(0,0) = red
        assert_eq!(&img.data[12..16], &[0, 0, 255, 255]);
    }

    #[test]
    fn test_rotation_270_cw_on_2x3() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 3,
            encoding: Encoding::Rgb565,
            frame_byte_length: 12,
            rotation: 270,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[
            0x00, 0xF8, 0xE0, 0x07, // row 0: red, green
            0x1F, 0x00, 0xE0, 0xFF, // row 1: blue, yellow
            0xFF, 0x07, 0x1F, 0xF8, // row 2: cyan, magenta
        ]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // 270° CW = 90° CCW, dimensions swap: 3×2
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);

        // 270° CW: old(x,y) → new(y, w-1-x)
        // old(0,0)=red   → new(0,1)
        // old(1,0)=green → new(0,0)
        // old(0,2)=cyan  → new(2,1)
        // old(1,2)=magenta → new(2,0)

        // new(0,0) = old(1,0) = green
        assert_eq!(&img.data[0..4], &[0, 255, 0, 255]);
        // new(2,0) = old(1,2) = magenta
        assert_eq!(&img.data[8..12], &[255, 0, 255, 255]);
        // new(0,1) = old(0,0) = red
        assert_eq!(&img.data[12..16], &[0, 0, 255, 255]);
        // new(2,1) = old(0,2) = cyan
        assert_eq!(&img.data[20..24], &[255, 255, 0, 255]);
    }

    #[test]
    fn test_rotation_noop_for_unknown_angle() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 1,
            encoding: Encoding::Rgb565,
            frame_byte_length: 4,
            rotation: 45, // unsupported → no-op
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255, 0xFF, 0xFF, 0xFF, 255]);
    }

    // ---- Crop + rotation ordering ----

    #[test]
    fn test_crop_then_rotation() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            crop_x: 0,
            crop_y: 0,
            crop_width: 1,
            crop_height: 2,
            rotation: 90,
            ..Default::default()
        };
        let mut buf = Vec::new();
        buf.extend_from_slice(&9999i32.to_be_bytes());
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // Step 1: crop to 1×2 (left column: red, blue)
        // Step 2: rotate 90° CW → 2×1 (old column becomes new row)
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        // After crop: column 0 = [red at (0,0), blue at (0,1)]
        // After 90° CW: old(x=0, y=0)=red → new(h-1-0, 0) = new(1,0)
        //               old(x=0, y=1)=blue → new(h-1-1, 0) = new(0,0)
        // new(0,0) = old(0,1) = blue → BGRA [255, 0, 0, 255]
        // new(1,0) = old(0,0) = red  → BGRA [0, 0, 255, 255]
        assert_eq!(&img.data[0..4], &[255, 0, 0, 255]);
        assert_eq!(&img.data[4..8], &[0, 0, 255, 255]);
    }

    // ---- decode_ithmb with known profile 1007 ----

    #[test]
    fn test_decode_ithmb_prefix_1007_dispatch() {
        // Profile 1007 is 480×864 RGB565. Provide minimal valid pixel data
        // so the decoder succeeds.
        let w = 480usize;
        let h = 864usize;
        let mut buf = vec![0u8; 4 + w * h * 2];
        buf[0..4].copy_from_slice(&1007i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_ithmb(&buf, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 480);
        assert_eq!(img.height, 864);
        assert_eq!(img.data.len(), w * h * 4);
        // All pixels should be white (BGRA = [255, 255, 255, 255])
        for chunk in img.data.chunks_exact(4) {
            assert_eq!(chunk, &[255, 255, 255, 255]);
        }
    }

    // ---- Post-processing edge cases ----

    #[test]
    fn test_crop_outside_bounds_clamps() {
        let profile = Profile {
            prefix: 9999,
            width: 2,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 8,
            crop_x: 10, // far outside image
            crop_y: 10, // far outside image
            crop_width: 5,
            crop_height: 5,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 4 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // Clamped: no pixels visible → image unchanged
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 4 * 4);
    }

    #[test]
    fn test_apply_crop_noop_when_not_needed() {
        // Directly test apply_crop returns the same image when no crop is set.
        let profile = Profile::default();
        let img = DecodedImage {
            data: vec![128u8; 16],
            width: 2,
            height: 2,
        };
        let result = apply_crop(img.clone(), &profile);
        assert_eq!(result.data, img.data);
        assert_eq!(result.width, img.width);
        assert_eq!(result.height, img.height);
    }

    #[test]
    fn test_apply_rotation_noop_when_zero() {
        let profile = Profile::default();
        let img = DecodedImage {
            data: vec![128u8; 16],
            width: 2,
            height: 2,
        };
        let result = apply_rotation(img.clone(), &profile);
        assert_eq!(result.data, img.data);
        assert_eq!(result.width, img.width);
        assert_eq!(result.height, img.height);
    }

    #[test]
    fn test_rotation_90_cw_identity() {
        // rotate_90_cw then rotate_270_cw should return to original.
        let original = DecodedImage {
            data: (0..16).collect(), // 2×2 image, every byte is its index
            width: 2,
            height: 2,
        };
        let rotated = rotate_90_cw(rotate_270_cw(original.clone()));
        assert_eq!(rotated.width, original.width);
        assert_eq!(rotated.height, original.height);
        assert_eq!(rotated.data, original.data);
    }

    #[test]
    fn test_rotation_180_twice_is_identity() {
        let original = DecodedImage {
            data: (0..24).collect(), // 2×3 image
            width: 2,
            height: 3,
        };
        let rotated = rotate_180(rotate_180(original.clone()));
        assert_eq!(rotated.data, original.data);
        assert_eq!(rotated.width, original.width);
        assert_eq!(rotated.height, original.height);
    }

    // ---- Decoder dispatch via decode_ithmb with known prefix ----

    #[test]
    fn test_decode_ithmb_prefix_1019_interlaced_uyvy() {
        // Profile 1019: 720×480 interlaced UYVY. Just check dispatch.
        let w = 720usize;
        let h = 480usize;
        let mut buf = vec![0u8; 4 + w * h * 2];
        buf[0..4].copy_from_slice(&1019i32.to_be_bytes());
        buf[4..].fill(128);

        let img = decode_ithmb(&buf, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 720);
        assert_eq!(img.height, 480);
        assert_eq!(img.data.len(), w * h * 4);
    }

    #[allow(clippy::cast_possible_truncation)]
    #[test]
    fn test_decode_ithmb_prefix_2002_big_endian_rgb565() {
        // Profile 2002: 50×50 big-endian RGB565 from built-in profiles.json.
        let w = 50usize;
        let h = 50usize;
        let mut buf = vec![0u8; 4 + w * h * 2];
        buf[0..4].copy_from_slice(&2002i32.to_be_bytes());
        buf[4..].fill(0xFF);
        buf[0..4].copy_from_slice(&2002i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_ithmb(&buf, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, w as u32);
        assert_eq!(img.height, h as u32);
    }

    // ---- Post-processing order: swap → crop → rotation ----

    #[test]
    fn test_swaps_dimensions_with_crop() {
        // 3×2 image with swaps_dimensions=true (becomes 2×3), then crop.
        let profile = Profile {
            prefix: 9999,
            width: 3,
            height: 2,
            encoding: Encoding::Rgb565,
            frame_byte_length: 12,
            swaps_dimensions: true,
            crop_x: 0,
            crop_y: 1,
            crop_width: 3,
            crop_height: 1,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 6 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        // After swaps_dimensions: 2×3 → width=2, height=3
        // After crop: y=1, height=1 → one row extracted (row 1 of 2×3 image)
        // But wait, swaps_dimensions happens on the decoded image metadata.
        // The decoder produces w=3, h=2, then swap gives w=2, h=3.
        // Then crop: cx=0, cy=1, cw=3, ch=1. But iw=2, ih=3.
        // cw = min(3, 2-0) = 2, ch = min(1, 3-1) = 1
        // Result: 2×1
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data.len(), 2 * 4);
    }
}
