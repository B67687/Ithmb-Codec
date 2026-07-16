//! Shared helper functions for decoders.
//!
//! This module provides common validation logic extracted from individual decoder
//! files to eliminate repetitive boilerplate (positive-dimension checks, sign-loss
//! casts, and buffer-too-short guards).
//!
//! The trailing-padding tolerance is configurable at runtime through a thread-local
//! override set by `set_tolerance` / `with_tolerance`. The `decode_ithmb_with_config`
//! entry point uses this mechanism to wire `DecodeConfig::trailing_padding_tolerance`
//! through to `validate_dimensions` without changing the public decode function
//! signatures in individual decoder modules.

use crate::error::DecodeError;
use crate::profile::Profile;
use std::borrow::Cow;
use std::cell::Cell;

/// Default trailing padding tolerance in bytes (C# reference: `TrailingPaddingTolerance = 256`).
const DEFAULT_TRAILING_PADDING_TOLERANCE: usize = 256;

// Thread-local override for the trailing padding tolerance.
// Set by `decode_ithmb_with_config` and `decode_with_profile_with_config`
// before calling into the decode pipeline, read by `validate_dimensions`.
thread_local! {
    static CURRENT_TOLERANCE: Cell<usize> = const { Cell::new(DEFAULT_TRAILING_PADDING_TOLERANCE) };
}

/// Get the current thread-local trailing padding tolerance.
#[must_use]
pub(crate) fn get_tolerance() -> usize {
    CURRENT_TOLERANCE.with(Cell::get)
}

/// Set the thread-local trailing padding tolerance and return the previous value.
pub(crate) fn set_tolerance(tolerance: usize) -> usize {
    CURRENT_TOLERANCE.with(|cell| cell.replace(tolerance))
}

/// Run a closure with a specific trailing padding tolerance, then restore.
///
/// # Example
///
/// ```ignore
/// let result = with_tolerance(512, || {
///     decode_with_profile(data, &profile, &canceled)
/// });
/// ```
pub(crate) fn with_tolerance<F, R>(tolerance: usize, f: F) -> R
where
    F: FnOnce() -> R,
{
    let old = set_tolerance(tolerance);
    let result = f();
    set_tolerance(old);
    result
}

/// Validates that profile dimensions are positive and converts them to `usize`.
///
/// When `bpp > 0` the function also checks that `src` is at least
/// `width × height × bpp` bytes long. If `src` is shorter but the deficit
/// is ≤ the current thread-local tolerance (default 256 bytes), the input
/// is zero-padded to the expected length instead of returning
/// [`DecodeError::BufferTooShort`].
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
/// `(padded_buf, width, height)` — the input buffer (owned if zero-padded,
/// borrowed otherwise) and the validated dimensions as `usize`.
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `InvalidFormat` | Width or height ≤ 0 |
/// | `BufferTooShort` | `bpp > 0` and deficit > tolerance |
#[allow(clippy::cast_sign_loss)]
pub(crate) fn validate_dimensions<'a>(
    src: &'a [u8],
    profile: &Profile,
    name: &str,
    bpp: usize,
) -> Result<(Cow<'a, [u8]>, usize, usize), DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat(name.into()));
    }

    let w = w_i32 as usize;
    let h = h_i32 as usize;

    if bpp > 0 {
        let expected = w
            .checked_mul(h)
            .and_then(|wh| wh.checked_mul(bpp))
            .ok_or(DecodeError::BufferTooShort { expected: 0, actual: 0 })?;
        if src.len() < expected {
            let deficit = expected - src.len();
            let tolerance = get_tolerance();
            if deficit > tolerance {
                return Err(DecodeError::BufferTooShort {
                    expected,
                    actual: src.len(),
                });
            }
            let mut padded = Vec::with_capacity(expected);
            padded.extend_from_slice(src);
            padded.resize(expected, 0);
            return Ok((Cow::Owned(padded), w, h));
        }
    }

    Ok((Cow::Borrowed(src), w, h))
}
