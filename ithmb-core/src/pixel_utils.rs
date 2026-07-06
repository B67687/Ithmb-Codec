//! Shared pixel-manipulation helpers for encoding and decoding.
//!
//! These functions are used across multiple decoders and encoders to avoid
//! code duplication. They cover MSB replication, value clamping, and
//! cancellation-check boilerplate.

use crate::error::DecodeError;
use std::sync::atomic::{AtomicBool, Ordering};

/// Replicates a 5-bit value to 8 bits: `(v << 3) | (v >> 2)`.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn msb_replicate_5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

/// Replicates a 6-bit value to 8 bits: `(v << 2) | (v >> 4)`.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn msb_replicate_6(v: u32) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}

/// Clamp an `i32` to the 0..255 u8 range.
#[inline]
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub(crate) fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Check whether the operation has been canceled.
///
/// Returns `Err(DecodeError::Canceled)` if `canceled` is `true`, allowing the
/// caller to short-circuit the decode loop.
#[inline]
pub(crate) fn check_canceled(canceled: &AtomicBool, name: &str) -> Result<(), DecodeError> {
    if canceled.load(Ordering::Acquire) {
        return Err(DecodeError::Canceled(name.into()));
    }
    Ok(())
}
