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
#[non_exhaustive]
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

    /// The input file exceeds the maximum allowed size.
    #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]
    FileTooLarge {
        /// Actual file size in bytes.
        size: usize,
        /// Maximum allowed size in bytes.
        limit: usize,
    },
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn decoded_image_display_reports_len_and_dims() {
        let img = DecodedImage {
            data: vec![1, 2, 3, 4],
            width: 2,
            height: 2,
        };
        assert_eq!(format!("{img}"), "DecodedImage { data: 4 bytes, width: 2, height: 2 }");
    }

    #[test]
    fn decoded_image_display_empty() {
        let img = DecodedImage {
            data: vec![],
            width: 0,
            height: 0,
        };
        assert_eq!(format!("{img}"), "DecodedImage { data: 0 bytes, width: 0, height: 0 }");
    }

    #[test]
    fn decode_error_display_string_variants() {
        assert_eq!(DecodeError::Io("disk".into()).to_string(), "I/O error: disk");
        assert_eq!(DecodeError::Jpeg("corrupt".into()).to_string(), "JPEG error: corrupt");
        assert_eq!(
            DecodeError::InvalidFormat("bad".into()).to_string(),
            "Invalid format: bad"
        );
        assert_eq!(
            DecodeError::Unsupported("unknown".into()).to_string(),
            "Unsupported format: unknown"
        );
        assert_eq!(
            DecodeError::Profile("no match".into()).to_string(),
            "Profile error: no match"
        );
        assert_eq!(DecodeError::Canceled("user".into()).to_string(), "Canceled: user");
    }

    #[test]
    fn decode_error_display_structured_variants() {
        assert_eq!(
            DecodeError::BufferTooShort {
                expected: 280,
                actual: 10
            }
            .to_string(),
            "Buffer too short: expected 280 bytes, got 10"
        );
        assert_eq!(
            DecodeError::FileTooLarge { size: 100, limit: 50 }.to_string(),
            "File too large: 100 bytes exceeds limit of 50 bytes"
        );
    }

    #[test]
    fn decode_error_is_std_error_without_source_chain() {
        // thiserror derives `std::error::Error`; no variant wraps a source error.
        assert!(DecodeError::Io("x".into()).source().is_none());
        assert!(DecodeError::Jpeg("x".into()).source().is_none());
        assert!(DecodeError::InvalidFormat("x".into()).source().is_none());
        assert!(DecodeError::Unsupported("x".into()).source().is_none());
        assert!(
            DecodeError::BufferTooShort { expected: 1, actual: 2 }
                .source()
                .is_none()
        );
        assert!(DecodeError::Profile("x".into()).source().is_none());
        assert!(DecodeError::Canceled("x".into()).source().is_none());
        assert!(DecodeError::FileTooLarge { size: 1, limit: 2 }.source().is_none());
    }

    #[test]
    fn decoded_image_clone_and_eq() {
        let a = DecodedImage {
            data: vec![9],
            width: 1,
            height: 1,
        };
        assert_eq!(a.clone(), a);
    }
}
