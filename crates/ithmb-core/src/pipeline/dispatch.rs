//! Internal decode core — prefix parsing, profile lookup, and decoder dispatch.
//!
//! Extracted from `mod.rs` to keep that file as the public-API facade.

use crate::cl;
use crate::clcl;
use crate::config;
use crate::decoder_helpers;
use crate::error::{DecodeError, DecodedImage};
use crate::jpeg;
use crate::profile::{Encoding, Profile};
use crate::reordered_rgb555;
use crate::rgb555;
use crate::rgb565;
use crate::uyvy;
use crate::ycbcr420;
use std::sync::atomic::AtomicBool;

use super::jpeg_scan::scan_for_embedded_jpeg;
use super::post_process::{apply_post_process, apply_post_process_with_transform};
use super::profile_loader::{fallback_jpeg_profile, get_db};

/// Shared entry core: prefix parsing, profile lookup, and embedded-JPEG fallback
/// for the `decode_ithmb*` family. `transform == None` means "use the profile's
/// own post-processing fields" — identical to the non-transform entry points.
pub(super) fn decode_ithmb_inner(
    src: &[u8],
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
    transform: Option<&config::TransformConfig>,
) -> Result<DecodedImage, DecodeError> {
    if src.len() < 4 {
        return Err(DecodeError::BufferTooShort {
            expected: 4,
            actual: src.len(),
        });
    }

    if src.len() > config.max_raw_file_size() {
        return Err(DecodeError::FileTooLarge {
            size: src.len(),
            limit: config.max_raw_file_size(),
        });
    }

    let prefix = i32::from_be_bytes([src[0], src[1], src[2], src[3]]);
    #[cfg(feature = "logging")]
    log::debug!("decode: prefix={prefix:08X}, len={}", src.len());
    let is_jpeg_stream = src[0] == 0xFF && src[1] == 0xD8;

    let db = get_db();

    let profile = if is_jpeg_stream {
        db.get(prefix).cloned().unwrap_or_else(fallback_jpeg_profile)
    } else if let Some(p) = db.resolve(prefix, src.len() - 4) {
        p
    } else {
        // Tier 2: data-size heuristic.
        let data_len = src.len() - 4;
        let mut best: Option<Profile> = None;
        let mut best_delta: usize = usize::MAX;
        for p in db.all().values() {
            #[allow(clippy::cast_sign_loss)]
            let delta = data_len.abs_diff(p.frame_byte_length as usize);
            if delta <= 256 && delta < best_delta {
                best_delta = delta;
                best = Some(p.clone());
            }
        }
        if let Some(profile) = best {
            profile
        } else {
            // Fallback: scan for embedded JPEG within the buffer.
            #[cfg(feature = "logging")]
            log::info!("decode: unknown prefix {prefix:08X}, scanning for embedded JPEG");
            match scan_for_embedded_jpeg(
                src,
                canceled,
                config.jpeg_scan_limit(),
                config.cancel_check_interval(),
                config.jfif_exif_scan_window(),
            ) {
                Some(jpeg_data) => {
                    let jp = fallback_jpeg_profile();
                    return decode_inner(jpeg_data, &jp, canceled, config, transform);
                }
                None => {
                    return Err(DecodeError::Unsupported(format!("unknown format prefix {prefix}")));
                }
            }
        }
    };

    decode_inner(src, &profile, canceled, config, transform)
}

/// Shared decode core: strips the 4-byte prefix (raw formats), dispatches to the
/// format decoder, then applies post-processing. The trailing-padding tolerance
/// from `config` is applied exactly once around the whole decode.
pub(super) fn decode_inner(
    src: &[u8],
    profile: &Profile,
    canceled: &AtomicBool,
    config: &config::DecodeConfig,
    transform: Option<&config::TransformConfig>,
) -> Result<DecodedImage, DecodeError> {
    decoder_helpers::with_tolerance(config.trailing_padding_tolerance(), || {
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
        Ok(match transform {
            Some(t) => apply_post_process_with_transform(img, profile, t),
            None => apply_post_process(img, profile),
        })
    })
}

/// Dispatches to the correct decoder based on the profile's encoding.
///
/// If the primary decoder fails and the profile specifies `fallback_encodings`,
/// each fallback is tried in order. Returns the first successful result or the
/// original error if no fallback succeeds.
pub(super) fn dispatch_decode(
    data: &[u8],
    profile: &Profile,
    canceled: &AtomicBool,
) -> Result<DecodedImage, DecodeError> {
    /// Inner dispatch — routes to the correct decoder for a single encoding.
    fn try_decode(data: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
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

    #[cfg(feature = "logging")]
    log::debug!(
        "dispatch_decode: encoding={:?}, dimensions={}x{}",
        profile.encoding,
        profile.width,
        profile.height
    );

    try_decode(data, profile, canceled).or_else(|primary_err| {
        if let Some(fallbacks) = &profile.fallback_encodings {
            for &enc in fallbacks {
                #[cfg(feature = "logging")]
                log::warn!("dispatch_decode: primary failed, trying fallback encoding {enc:?}");
                let fallback_profile = Profile {
                    encoding: enc,
                    // Prevent infinite recursion — fallbacks do not themselves
                    // carry a fallback list.
                    fallback_encodings: None,
                    ..profile.clone()
                };
                if let Ok(img) = try_decode(data, &fallback_profile, canceled) {
                    return Ok(img);
                }
            }
        }
        Err(primary_err)
    })
}
