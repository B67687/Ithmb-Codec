//! Shared helper functions for decoders.
//!
//! This module provides common validation logic extracted from individual decoder
//! files to eliminate repetitive boilerplate (positive-dimension checks, sign-loss
//! casts, and buffer-too-short guards).

use crate::error::DecodeError;
use crate::profile::Profile;

/// Validates that profile dimensions are positive and converts them to `usize`.
///
/// When `bpp > 0` the function also checks that `src` is at least
/// `width × height × bpp` bytes long.
///
/// # Arguments
///
/// * `src` — Raw input byte slice.
/// * `profile` — The profile whose `width` and `height` are validated.
/// * `name` — Label used in the [`DecodeError::InvalidFormat`] message.
/// * `bpp` — Bytes per pixel. Pass `0` to skip the buffer-length check for
///   formats that need a more complex expected-size calculation (e.g., `YCbCr420`).
///
/// # Returns
///
/// `(width, height)` as `usize`.
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `InvalidFormat` | Width or height ≤ 0 |
/// | `BufferTooShort` | `bpp > 0` and `src.len() < w × h × bpp` |
#[allow(clippy::cast_sign_loss)]
pub(crate) fn validate_dimensions(
    src: &[u8],
    profile: &Profile,
    name: &str,
    bpp: usize,
) -> Result<(usize, usize), DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat(name.into()));
    }

    let w = w_i32 as usize;
    let h = h_i32 as usize;

    if bpp > 0 {
        let expected = w * h * bpp;
        if src.len() < expected {
            return Err(DecodeError::BufferTooShort {
                expected,
                actual: src.len(),
            });
        }
    }

    Ok((w, h))
}
