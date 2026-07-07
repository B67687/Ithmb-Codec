//! Error types and image container for ithmb-core.
//!
//! # `DecodeError`
//!
//! Every decoder path returns [`DecodeError`] on failure — never `Box<dyn Error>` or
//! raw I/O errors (this crate is pure, no I/O).
//!
//! # `DecodedImage`
//!
//! The canonical output type: a decoded bitmap with its dimensions.

use std::fmt;

// ---------------------------------------------------------------------------
// DecodedImage
// ---------------------------------------------------------------------------

/// A fully decoded bitmap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    /// Raw pixel data in BGRA 8-bit order (blue, green, red, alpha).
    pub data: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

// ---------------------------------------------------------------------------
// DecodeError
// ---------------------------------------------------------------------------

/// Errors that can occur while decoding an `.ithmb` thumbnail.
///
/// Every variant carries a human-readable detail string or structured fields.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    /// An I/O-level failure (e.g. end of stream, read error).
    #[error("I/O error: {0}")]
    Io(String),

    /// A JPEG decode failure (corrupt or unsupported JPEG data).
    #[error("JPEG error: {0}")]
    Jpeg(String),

    /// The file format is invalid or unrecognized.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// The format is recognized but not supported by this decoder.
    #[error("Unsupported format: {0}")]
    Unsupported(String),

    /// The input buffer ended before the expected amount of data was consumed.
    #[error("Buffer too short: expected {expected} bytes, got {actual}")]
    BufferTooShort {
        /// Number of bytes the decoder expected.
        expected: usize,
        /// Actual number of bytes available.
        actual: usize,
    },

    /// A decoder profile mismatch or configuration error.
    #[error("Profile error: {0}")]
    Profile(String),

    /// The operation was canceled by the caller.
    #[error("Canceled: {0}")]
    Canceled(String),
}

// Manual impls for traits that derive cannot produce for enum variants with
// named fields only (same as derived Debug + Display above — placeholder for
// future manual formatting).
impl fmt::Display for DecodedImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DecodedImage {{ data: {} bytes, width: {}, height: {} }}",
            self.data.len(),
            self.width,
            self.height,
        )
    }
}
