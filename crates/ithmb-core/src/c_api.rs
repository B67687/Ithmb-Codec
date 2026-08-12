//! C FFI bindings for ithmb-core.
//! The API is caller-allocated: the caller provides an output buffer via
//! [`crate::c_api::IthmbImage::data`] and the functions write decoded pixels into it.
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
    // The caller sizes out->data from ithmb_prefix_to_profile. If the decoded
    // image is larger than the profile claims (e.g. a JPEG embedded under a
    // profile with smaller display dimensions), copying nbytes would overflow
    // the caller's buffer — reject instead of corrupting memory (CWE-787).
    // Area-based (w×h) so EXIF rotation, which swaps width/height, does not
    // false-positive.
    if (u64::from(img.width) * u64::from(img.height)) > (u64::from(out_ref.width) * u64::from(out_ref.height)) {
        return ITHMB_ERROR_INVALID;
    }
    let nbytes = (img.width as usize) * (img.height as usize) * 4;
    // SAFETY: out_ref.data is valid for nbytes — the check above guarantees
    // nbytes fits the profile-sized caller buffer.
    unsafe {
        std::ptr::copy_nonoverlapping(img.data.as_ptr(), out_ref.data, nbytes);
    }
    out_ref.width = img.width;
    out_ref.height = img.height;

    ITHMB_OK
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Return codes are asserted against *literal* values rather than the
// ITHMB_ERROR_* constants on purpose: a mutated constant (e.g. -1 becoming 1)
// would otherwise trivially satisfy `ret == ITHMB_ERROR_INVALID`.
#[cfg(test)]
mod tests {
    use super::*;

    /// A valid white 128×128 RGB565 frame: 4-byte prefix 1055 + 32768 bytes of
    /// 0xFFFF little-endian (white) pixels.
    fn white_rgb565_1055_payload() -> Vec<u8> {
        let mut payload = 1055_i32.to_be_bytes().to_vec();
        payload.resize(payload.len() + 128 * 128 * 2, 0xFF);
        payload
    }

    #[test]
    fn prefix_to_profile_null_out_returns_invalid() {
        let ret = unsafe { ithmb_prefix_to_profile(1055, std::ptr::null_mut()) };
        assert_eq!(ret, -1, "ITHMB_ERROR_INVALID");
    }

    #[test]
    fn prefix_to_profile_unknown_prefix_returns_unsupported() {
        let mut out = IthmbImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        let ret = unsafe { ithmb_prefix_to_profile(0xDEAD_BEEF, std::ptr::from_mut(&mut out)) };
        assert_eq!(ret, -2, "ITHMB_ERROR_UNSUPPORTED");
    }

    #[test]
    fn prefix_to_profile_known_prefix_returns_display_dims() {
        let mut out = IthmbImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        let ret = unsafe { ithmb_prefix_to_profile(1055, std::ptr::from_mut(&mut out)) };
        assert_eq!(ret, 0, "ITHMB_OK");
        assert_eq!(out.width, 128);
        assert_eq!(out.height, 128);
    }

    #[test]
    fn decode_null_src_returns_invalid() {
        let mut out = IthmbImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        let ret = unsafe { ithmb_decode(std::ptr::null(), 0, std::ptr::from_mut(&mut out), std::ptr::null()) };
        assert_eq!(ret, -1, "ITHMB_ERROR_INVALID");
    }

    #[test]
    fn decode_null_out_returns_invalid() {
        let src = [0_u8; 4];
        let ret = unsafe { ithmb_decode(src.as_ptr(), src.len(), std::ptr::null_mut(), std::ptr::null()) };
        assert_eq!(ret, -1, "ITHMB_ERROR_INVALID");
    }

    #[test]
    fn decode_truncated_known_format_returns_invalid() {
        // Prefix 1055 claims 128×128 (needs 32768 pixel bytes); 100 is far too
        // short, so the decoder must fail with a buffer error → INVALID.
        let mut payload = 1055_i32.to_be_bytes().to_vec();
        payload.extend_from_slice(&[0_u8; 100]);
        let mut out = IthmbImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        let ret = unsafe {
            ithmb_decode(
                payload.as_ptr(),
                payload.len(),
                std::ptr::from_mut(&mut out),
                std::ptr::null(),
            )
        };
        assert_eq!(ret, -1, "ITHMB_ERROR_INVALID");
    }

    #[test]
    fn decode_unknown_prefix_returns_unsupported() {
        // 0xDEADBEEF matches no built-in profile and is not a JPEG stream; the
        // size heuristic also fails for an 8-byte payload, so the decoder must
        // reject the format as unsupported.
        let payload = [0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut out = IthmbImage {
            data: std::ptr::null_mut(),
            width: 0,
            height: 0,
        };
        let ret = unsafe {
            ithmb_decode(
                payload.as_ptr(),
                payload.len(),
                std::ptr::from_mut(&mut out),
                std::ptr::null(),
            )
        };
        assert_eq!(ret, -2, "ITHMB_ERROR_UNSUPPORTED");
    }

    #[test]
    fn decode_respects_cancel_flag() {
        let payload = white_rgb565_1055_payload();
        let mut out_data = vec![0_u8; 128 * 128 * 4];
        let mut out = IthmbImage {
            data: out_data.as_mut_ptr(),
            width: 128,
            height: 128,
        };
        let canceled = AtomicBool::new(true);
        let ret = unsafe {
            ithmb_decode(
                payload.as_ptr(),
                payload.len(),
                std::ptr::from_mut(&mut out),
                std::ptr::from_ref(&canceled),
            )
        };
        assert_eq!(ret, -3, "ITHMB_ERROR_CANCELED");
    }

    #[test]
    fn decode_valid_rgb565_returns_image() {
        let payload = white_rgb565_1055_payload();
        let mut out_data = vec![0_u8; 128 * 128 * 4];
        let mut out = IthmbImage {
            data: out_data.as_mut_ptr(),
            width: 128,
            height: 128,
        };
        let canceled = AtomicBool::new(false);
        let ret = unsafe {
            ithmb_decode(
                payload.as_ptr(),
                payload.len(),
                std::ptr::from_mut(&mut out),
                std::ptr::from_ref(&canceled),
            )
        };
        assert_eq!(ret, 0, "ITHMB_OK");
        assert_eq!(out.width, 128);
        assert_eq!(out.height, 128);
        // SAFETY: out.data was allocated for exactly 128*128*4 bytes above and
        // the call reported success.
        let actual = unsafe { std::slice::from_raw_parts(out.data, 128 * 128 * 4) };
        let expected = vec![255_u8; 128 * 128 * 4];
        assert_eq!(actual, &expected[..], "white RGB565 decodes to white BGRA");
    }
}
