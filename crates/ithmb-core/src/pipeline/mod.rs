//! Pipeline module — central dispatch for .ithmb decoding.
//!
//! This is the most important module in the library. It orchestrates all decoders:
//! reads the format prefix, looks up the decoding profile, dispatches to the
//! correct decoder function, and applies post-processing (crop, rotation).
//!
//! The module is split into sub-modules:
//!
//! * `dispatch` - internal decode core (prefix parsing, profile lookup, decoder dispatch)
//! * `profile_loader` - one-time initialization of the built-in profile DB
//! * `open` - `PhotoDB` / `ArtworkDB` multi-frame container opening
//! * `post_process` - dimension swap, crop, and rotation
//! * `jpeg_scan` - embedded JPEG stream scanning and extraction

mod dispatch;
mod jpeg_scan;
mod open;
mod post_process;
mod profile_loader;

#[allow(unused_imports)]
pub(super) use jpeg_scan::{has_jpeg_marker, scan_for_embedded_jpeg};
#[allow(unused_imports)]
pub(super) use post_process::{
    apply_crop, apply_crop_with, apply_post_process, apply_post_process_with_transform, apply_rotation,
    apply_rotation_with,
};

pub use self::open::{open_ithmb, open_ithmb_with_config};

pub(crate) use self::profile_loader::get_db;
use crate::config;
use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::sync::atomic::AtomicBool;

/// Look up the human-readable encoding name for a given format prefix.
/// Returns `"Unknown format"` if the prefix is not found in the built-in profiles.
#[must_use]
pub fn encoding_name_for_prefix(prefix: i32) -> &'static str {
    let db = get_db();
    match db.get(prefix) {
        Some(profile) => profile.encoding.to_display_string(),
        None => "Unknown format",
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
    decode_ithmb_with_config(src, canceled, config::default_config())
}

/// Decode a complete `.ithmb` file with a custom [`DecodeConfig`](crate::config::DecodeConfig).
///
/// Like [`decode_ithmb`] but allows overriding decode parameters (file size limit,
/// JPEG scan limit, cancellation check interval, etc.) at runtime.
///
/// # Errors
///
/// Same as [`decode_ithmb`].
pub fn decode_ithmb_with_config(
    src: &[u8],
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
) -> Result<DecodedImage, DecodeError> {
    dispatch::decode_ithmb_inner(src, canceled, config, None)
}

/// Decode a complete `.ithmb` file with custom limits AND runtime decode-parameter
/// overrides ([`TransformConfig`](crate::config::TransformConfig)).
///
/// Like [`decode_ithmb_with_config`] but additionally applies the caller-supplied
/// transform overrides (rotation, crop) after profile selection.
///
/// # Errors
///
/// Same as [`decode_ithmb_with_config`].
pub fn decode_ithmb_with_transform(
    src: &[u8],
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
    transform: &config::TransformConfig,
) -> Result<DecodedImage, DecodeError> {
    dispatch::decode_ithmb_inner(src, canceled, config, Some(transform))
}

/// Decode an `.ithmb` file using an explicit profile, bypassing prefix-lookup.
///
/// This is useful when the caller already knows the profile (e.g. from `PhotoDB`
/// metadata, or for testing with synthetic data).
///
/// # Errors
///
/// Returns ``DecodeError::BufferTooShort`` if the input is too short for the
/// expected prefix (4 bytes for raw formats). Propagates decoder errors.
pub fn decode_with_profile(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    decode_with_profile_with_config(src, profile, canceled, config::default_config())
}

/// Decode an `.ithmb` file using an explicit profile and custom [`DecodeConfig`](crate::config::DecodeConfig).
///
/// Like [`decode_with_profile`] but accepts a [`DecodeConfig`](crate::config::DecodeConfig) for runtime
/// configuration of parameters such as trailing padding tolerance and file-size limits.
///
/// # Errors
///
/// Same as ``decode_with_profile``.
pub fn decode_with_profile_with_config(
    src: &[u8],
    profile: &Profile,
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
) -> Result<DecodedImage, DecodeError> {
    dispatch::decode_inner(src, profile, canceled, config, None)
}

/// Decode an `.ithmb` file using an explicit profile and custom [`DecodeConfig`](crate::config::DecodeConfig)
/// with runtime decode-parameter overrides ([`TransformConfig`](crate::config::TransformConfig)).
///
/// Like [`decode_with_profile_with_config`] but applies the caller-supplied
/// transform overrides after profile selection.
///
/// This is a public API extension point for advanced callers who already have
/// a resolved [`Profile`] (e.g. from `PhotoDB` metadata or a custom registry)
/// and want to override rotation/crop at decode time without mutating the
/// shared profile. It has no internal callers; prefer [`decode_ithmb_with_transform`]
/// when prefix-based profile lookup is acceptable.
///
/// # Errors
///
/// Same as [`decode_with_profile_with_config`].
pub fn decode_with_profile_with_transform(
    src: &[u8],
    profile: &Profile,
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
    transform: &config::TransformConfig,
) -> Result<DecodedImage, DecodeError> {
    dispatch::decode_inner(src, profile, canceled, config, Some(transform))
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
        let buf = [0x00, 0x00, 0x27, 0x0F]; // 9999 in big-endian
        let result = decode_ithmb(&buf, &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::Unsupported(ref msg)) if msg.contains("9999")
        ));
    }

    #[test]
    fn test_jpeg_fallback_profile_is_used() {
        let buf = [0xFF, 0xD8, 0x00, 0x00, 0x00];
        let result = decode_ithmb(&buf, &AtomicBool::new(false));
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
            frame_byte_length: 6,
            ..Default::default()
        };
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
        let profile = Profile {
            prefix: -1,
            width: 0,
            height: 0,
            encoding: Encoding::Jpeg,
            use_mhni_dimensions: true,
            ..Default::default()
        };
        let buf = [0xFF, 0xD8, 0x00, 0x00];
        let result = decode_with_profile(&buf, &profile, &AtomicBool::new(false));
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
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 6 * 4);
    }

    // ---- Crop ----

    #[test]
    fn test_crop_2x2_to_1x1() {
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
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
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
            crop_width: 0,
            crop_height: 0,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 6 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
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
        buf.extend_from_slice(&[
            0x00, 0xF8, 0xE0, 0x07, // row 0
            0x1F, 0x00, 0xE0, 0xFF, // row 1
            0xFF, 0x07, 0x1F, 0xF8, // row 2
        ]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 3 * 2 * 4);

        assert_eq!(&img.data[0..4], &[255, 255, 0, 255]);
        assert_eq!(&img.data[8..12], &[0, 0, 255, 255]);
        assert_eq!(&img.data[12..16], &[255, 0, 255, 255]);
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
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);

        assert_eq!(&img.data[0..4], &[255, 255, 255, 255]);
        assert_eq!(&img.data[4..8], &[255, 0, 0, 255]);
        assert_eq!(&img.data[8..12], &[0, 255, 0, 255]);
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
        buf.extend_from_slice(&[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xE0, 0xFF, 0xFF, 0x07, 0x1F, 0xF8]);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 3);
        assert_eq!(img.height, 2);

        assert_eq!(&img.data[0..4], &[0, 255, 0, 255]);
        assert_eq!(&img.data[8..12], &[255, 0, 255, 255]);
        assert_eq!(&img.data[12..16], &[0, 0, 255, 255]);
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
            rotation: 45,
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
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(&img.data[0..4], &[255, 0, 0, 255]);
        assert_eq!(&img.data[4..8], &[0, 0, 255, 255]);
    }

    // ---- decode_ithmb with known profile 1007 ----

    #[test]
    fn test_decode_ithmb_prefix_1007_dispatch() {
        let w = 480usize;
        let h = 864usize;
        let mut buf = vec![0u8; 4 + w * h * 2];
        buf[0..4].copy_from_slice(&1007i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_ithmb(&buf, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 480);
        assert_eq!(img.height, 864);
        assert_eq!(img.data.len(), w * h * 4);
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
            crop_x: 10,
            crop_y: 10,
            crop_width: 5,
            crop_height: 5,
            ..Default::default()
        };
        let mut buf = vec![0u8; 4 + 4 * 2];
        buf[0..4].copy_from_slice(&9999i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_with_profile(&buf, &profile, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 4 * 4);
    }

    #[test]
    fn test_apply_crop_noop_when_not_needed() {
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
        let original = DecodedImage {
            data: (0..16).collect(),
            width: 2,
            height: 2,
        };
        let rotated = apply_rotation_with(apply_rotation_with(original.clone(), 90), 270);
        assert_eq!(rotated.width, original.width);
        assert_eq!(rotated.height, original.height);
        assert_eq!(rotated.data, original.data);
    }

    #[test]
    fn test_rotation_180_twice_is_identity() {
        let original = DecodedImage {
            data: (0..24).collect(),
            width: 2,
            height: 3,
        };
        let rotated = apply_rotation_with(apply_rotation_with(original.clone(), 180), 180);
        assert_eq!(rotated.data, original.data);
        assert_eq!(rotated.width, original.width);
        assert_eq!(rotated.height, original.height);
    }

    // ---- Decoder dispatch via decode_ithmb with known prefix ----

    #[test]
    fn test_decode_ithmb_prefix_1019_interlaced_uyvy() {
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
        let w = 50usize;
        let h = 50usize;
        let mut buf = vec![0u8; 4 + w * h * 2];
        buf[0..4].copy_from_slice(&2002i32.to_be_bytes());
        buf[4..].fill(0xFF);

        let img = decode_ithmb(&buf, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.width, w as u32);
        assert_eq!(img.height, h as u32);
    }

    // ---- Post-processing order: swap → crop → rotation ----

    #[test]
    fn test_swaps_dimensions_with_crop() {
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
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(img.data.len(), 2 * 4);
    }

    #[test]
    fn transform_override_rotation_applies_over_profile_zero() {
        let img = DecodedImage {
            data: vec![0u8; 2 * 3 * 4],
            width: 2,
            height: 3,
        };
        let profile = Profile {
            rotation: 0,
            ..Profile::default()
        };
        let transform = config::TransformConfig::default().with_rotation(90);
        let out = apply_post_process_with_transform(img, &profile, &transform);
        assert_eq!(out.width, 3);
        assert_eq!(out.height, 2);
    }

    #[test]
    fn transform_default_falls_back_to_profile_rotation() {
        let img = DecodedImage {
            data: vec![0u8; 2 * 3 * 4],
            width: 2,
            height: 3,
        };
        let profile = Profile {
            rotation: 90,
            ..Profile::default()
        };
        let transform = config::TransformConfig::default();
        let out = apply_post_process_with_transform(img, &profile, &transform);
        assert_eq!(out.width, 3);
        assert_eq!(out.height, 2);
    }

    #[test]
    fn transform_identity_matches_apply_post_process() {
        let img = DecodedImage {
            data: vec![7u8; 2 * 3 * 4],
            width: 2,
            height: 3,
        };
        let profile = Profile {
            rotation: 90,
            ..Profile::default()
        };
        let a = apply_post_process(img.clone(), &profile);
        let b = apply_post_process_with_transform(img, &profile, &config::TransformConfig::default());
        assert_eq!(a, b);
    }
}
