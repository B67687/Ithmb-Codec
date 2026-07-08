//! C FFI bindings for ithmb-core.
//! The API is caller-allocated: the caller provides an output buffer via
//! [`IthmbImage::data`] and the functions write decoded pixels into it.
//!
//! # Safety
//! All functions are `extern "C"` and inherently unsafe. The caller must
//! provide valid pointers and correctly sized buffers.
#![allow(
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::manual_let_else
)]

use crate::error::DecodeError;
use crate::pipeline::decode_ithmb;
use crate::pipeline::get_db;
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A decoded image — caller allocates `data`, function fills it.
#[repr(C)]
#[derive(Debug)]
pub struct IthmbImage {
    /// Pointer to BGRA pixel data (8-bit per channel, 4 bytes per pixel).
    pub data: *mut u8,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

// ---------------------------------------------------------------------------
// Error codes
// ---------------------------------------------------------------------------

/// Success.
pub const ITHMB_OK: i32 = 0;
/// The input data is invalid or corrupt.
pub const ITHMB_ERROR_INVALID: i32 = -1;
/// The format is recognized but not supported by this decoder.
pub const ITHMB_ERROR_UNSUPPORTED: i32 = -2;
/// The operation was canceled by the caller.
pub const ITHMB_ERROR_CANCELED: i32 = -3;

// ---------------------------------------------------------------------------
// Exported C functions
// ---------------------------------------------------------------------------

/// Look up the output dimensions for a given format prefix.
///
/// Sets `out->width` and `out->height` to the pixel dimensions of the profile
/// matching `prefix`. The caller can then allocate `out->data` as
/// `width * height * 4` bytes and pass the struct to [`ithmb_decode`].
///
/// # Safety
/// * `out` must be a valid, non-null pointer to an `IthmbImage`.
///
/// # Returns
/// * `ITHMB_OK` (0) on success.
/// * `ITHMB_ERROR_UNSUPPORTED` (-2) if `prefix` does not match any known profile.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ithmb_prefix_to_profile(prefix: u32, out: *mut IthmbImage) -> i32 {
    if out.is_null() {
        return ITHMB_ERROR_INVALID;
    }
    let db = get_db();
    let profile = match db.get(prefix as i32) {
        Some(p) => p,
        None => return ITHMB_ERROR_UNSUPPORTED,
    };
    (*out).width = profile.display_width() as u32;
    (*out).height = profile.display_height() as u32;
    ITHMB_OK
}

/// Decode an `.ithmb` file from a raw byte buffer.
///
/// The caller must provide a pre-allocated output buffer in `out->data`.
/// Before calling this function, use [`ithmb_prefix_to_profile`] to determine
/// the required buffer size (`width * height * 4` bytes).
///
/// # Safety
/// * `src` must be a valid pointer to `len` readable bytes.
/// * `out` must be a valid, non-null pointer to an `IthmbImage` with
///   `out->data` pointing to a buffer of at least
///   `out->width * out->height * 4` bytes.
/// * `cancel_flag` must be a valid pointer to an `AtomicBool`, or `NULL`.
///
/// # Returns
/// * `ITHMB_OK` (0) on success.
/// * `ITHMB_ERROR_INVALID` (-1) if the input is corrupt or invalid.
/// * `ITHMB_ERROR_UNSUPPORTED` (-2) if the format is unknown.
/// * `ITHMB_ERROR_CANCELED` (-3) if the operation was cancelled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ithmb_decode(
    src: *const u8,
    len: usize,
    out: *mut IthmbImage,
    cancel_flag: *const AtomicBool,
) -> i32 {
    if src.is_null() || out.is_null() {
        return ITHMB_ERROR_INVALID;
    }
    let src_slice: &[u8] = std::slice::from_raw_parts(src, len);

    // Resolve cancellation flag.
    let canceled_ref: &AtomicBool = if cancel_flag.is_null() {
        static FALSE: AtomicBool = AtomicBool::new(false);
        &FALSE
    } else {
        &*cancel_flag
    };

    let img = match decode_ithmb(src_slice, canceled_ref) {
        Ok(img) => img,
        Err(DecodeError::Canceled(_)) => return ITHMB_ERROR_CANCELED,
        Err(DecodeError::InvalidFormat(_) | DecodeError::Io(_)) => return ITHMB_ERROR_INVALID,
        Err(DecodeError::Unsupported(_) | DecodeError::Profile(_)) => return ITHMB_ERROR_UNSUPPORTED,
        Err(DecodeError::BufferTooShort { .. } | DecodeError::FileTooLarge { .. } | DecodeError::Jpeg(_)) => {
            return ITHMB_ERROR_INVALID;
        }
    };

    let out_ref = &mut *out;
    let nbytes = (img.width as usize) * (img.height as usize) * 4;
    // SAFETY: out_ref.data is valid for nbytes (caller must ensure).
    unsafe {
        std::ptr::copy_nonoverlapping(img.data.as_ptr(), out_ref.data, nbytes);
    }
    out_ref.width = img.width;
    out_ref.height = img.height;

    ITHMB_OK
}
