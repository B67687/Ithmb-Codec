//! Multi-frame container opening — `PhotoDB` / `ArtworkDB` dispatch.
//!
//! [`open_ithmb`] is the entry point for opening `.ithmb` files that may be
//! `PhotoDB` containers (MHFD magic) or bare single-frame files. For containers
//! it parses the chunk tree, resolves profiles, applies optional device-name
//! filtering, and decodes every entry. For bare files it delegates to
//! [`decode_ithmb`].

use crate::config;
use crate::device_profiles;
use crate::error::{DecodeError, DecodedImage};
use crate::photodb::parser::{can_open_photodb, try_parse_photodb};
use crate::pipeline::profile_loader::{fallback_jpeg_profile, get_db};
use crate::pipeline::{decode_ithmb_with_config, decode_with_profile_with_config};
use crate::profile::{Encoding, Profile};
#[cfg(feature = "logging")]
use log::{debug, info};
use std::sync::atomic::AtomicBool;

/// Open a `PhotoDB` container file and decode all contained thumbnails.
///
/// When the input is a bare .ithmb file (no MHFD magic), the entire input is
/// decoded as a single frame via `decode_ithmb` and returned as a one-element
/// vector.
///
/// `device_name` is an optional case-insensitive device name (e.g. 'iPod Classic 5G').
/// When provided, only entries whose `format_id` matches the device's known format
/// list are decoded. Entries with format IDs unknown to the device are silently
/// skipped.
///
/// When no device name is given (or the device is not found), all entries are
/// decoded.
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `BufferTooShort` | Input is smaller than 4 bytes. |
/// | `InvalidFormat` | PhotoDB parsing failed. |
/// | Decoder errors | Propagated from the underlying decoder. |
#[allow(clippy::cast_sign_loss)]
pub fn open_ithmb(
    src: &[u8],
    canceled: &AtomicBool,
    device_name: Option<&str>,
) -> Result<Vec<DecodedImage>, DecodeError> {
    open_ithmb_with_config(src, canceled, device_name, config::default_config())
}

/// Open a `PhotoDB` container file (or bare .ithmb) with a custom `DecodeConfig`.
///
/// Like `open_ithmb` but accepts a `DecodeConfig` for runtime parameter
/// customization (file size limit, etc.).
///
/// # Errors
///
/// Same as [`open_ithmb`].
#[allow(clippy::cast_sign_loss)]
pub fn open_ithmb_with_config(
    src: &[u8],
    canceled: &AtomicBool,
    device_name: Option<&str>,
    config: &config::DecodeConfig,
) -> Result<Vec<DecodedImage>, DecodeError> {
    if src.len() > config.max_raw_file_size() {
        return Err(DecodeError::FileTooLarge {
            size: src.len(),
            limit: config.max_raw_file_size(),
        });
    }
    #[cfg(feature = "logging")]
    debug!("open_ithmb: len={}", src.len());
    if can_open_photodb(src) {
        #[cfg(feature = "logging")]
        info!("open_ithmb: detected PhotoDB container");
        let mut entries = Vec::new();
        try_parse_photodb(src, &mut entries)?;

        // If a device name is provided, resolve it to known format IDs.
        let allowed_formats: Option<Vec<i32>> = device_name.map(|name| {
            device_profiles::find_device(name)
                .map(|dp| dp.formats.iter().map(|f| f.format_id).collect())
                .unwrap_or_default()
        });

        let db = get_db();
        let mut results = Vec::with_capacity(entries.len());

        for entry in &entries {
            // Skip entries filtered out by device-name constraint.
            if let Some(ref allowed) = allowed_formats
                && !allowed.contains(&entry.format_id)
            {
                continue;
            }

            if entry.data.is_empty() {
                continue;
            }

            if let Some(profile) = db.get(entry.format_id) {
                #[cfg(feature = "logging")]
                debug!("open_ithmb: profile matched: prefix={:08X}", profile.prefix);
                // Override dimensions from MHNI chunk metadata when the profile
                // specifies use_mhni_dimensions.
                let adjusted;
                let profile = if profile.use_mhni_dimensions {
                    adjusted = Profile {
                        width: entry.width,
                        height: entry.height,
                        ..profile.clone()
                    };
                    &adjusted
                } else {
                    profile
                };

                // Known profile - construct the buffer with prefix if needed.
                let prefixed = if profile.encoding == Encoding::Jpeg {
                    entry.data.clone()
                } else {
                    let prefix_bytes = (profile.prefix as u32).to_be_bytes();
                    let mut with_prefix = Vec::with_capacity(4 + entry.data.len());
                    with_prefix.extend_from_slice(&prefix_bytes);
                    with_prefix.extend_from_slice(&entry.data);
                    with_prefix
                };

                let img = decode_with_profile_with_config(&prefixed, profile, canceled, config)?;
                results.push(img);
            } else if entry.data.len() >= 2 && entry.data[0] == 0xFF && entry.data[1] == 0xD8 {
                // No profile but data is a JPEG stream - use a fallback profile.
                let mut profile = fallback_jpeg_profile();
                profile.width = entry.width;
                profile.height = entry.height;

                let img = decode_with_profile_with_config(&entry.data, &profile, canceled, config)?;
                results.push(img);
            }
            // No profile and not JPEG: skip entry
        }

        Ok(results)
    } else {
        // Not a PhotoDB file - decode as bare .ithmb.
        Ok(decode_ithmb_with_config(src, canceled, config).map(|img| vec![img])?)
    }
}
