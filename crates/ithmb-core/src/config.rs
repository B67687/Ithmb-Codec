//! Runtime configuration for the decode pipeline.
//!
//! [`DecodeConfig`] allows callers to customize decode parameters such as
//! maximum file size, JPEG scan limits, and cancellation check frequency,
//! instead of using hardcoded defaults.
//!
//! # Example
//!
/// ```rust
/// use ithmb_core::config::DecodeConfig;
///
/// let config = DecodeConfig::default()
///     .with_max_raw_file_size(16 * 1024 * 1024)
///     .with_jpeg_scan_limit(8 * 1024 * 1024);
///
/// assert_eq!(config.max_raw_file_size(), 16 * 1024 * 1024);
/// ```
use std::sync::OnceLock;

/// Runtime configuration for `.ithmb` decode parameters.
///
/// All fields have sensible defaults matching the original hardcoded constants.
/// Use the builder-pattern methods to customize individual parameters.
///
/// # Default values
///
/// | Field | Default | Description |
/// |---|---|---|
/// | `max_raw_file_size` | 8 MiB | Maximum input file size before rejection |
/// | `jpeg_scan_limit` | 4 MiB | Max bytes to scan for embedded JPEG markers |
/// | `cancel_check_interval` | 64 KiB | Byte interval between cancellation polls |
/// | `trailing_padding_tolerance` | 256 | Padding deficit allowed before `BufferTooShort` |
/// | `jfif_exif_scan_window` | 512 | Bytes after SOI to search for JFIF/Exif |
#[derive(Clone, Debug)]
pub struct DecodeConfig {
    /// Maximum raw file size in bytes. Files larger than this are rejected
    /// before any decoding begins (prevents OOM from pathological input).
    max_raw_file_size: usize,
    /// Maximum number of bytes to scan for embedded JPEG markers.
    jpeg_scan_limit: usize,
    /// Interval (in bytes) between cancellation flag checks during scanning.
    cancel_check_interval: usize,
    /// Trailing padding tolerance in bytes. Decoders tolerate up to this
    /// many missing bytes at the end of a frame before raising
    /// [`DecodeError::BufferTooShort`](crate::DecodeError::BufferTooShort).
    trailing_padding_tolerance: usize,
    /// Maximum bytes after JPEG SOI to search for JFIF or Exif marker.
    jfif_exif_scan_window: usize,
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            max_raw_file_size: 8 * 1024 * 1024, // 8 MiB
            jpeg_scan_limit: 4 * 1024 * 1024,   // 4 MiB
            cancel_check_interval: 64 * 1024,   // 64 KiB
            trailing_padding_tolerance: 256,
            jfif_exif_scan_window: 512,
        }
    }
}

// ---------------------------------------------------------------------------
// Getters
// ---------------------------------------------------------------------------

impl DecodeConfig {
    /// Maximum input file size in bytes.
    #[must_use]
    pub fn max_raw_file_size(&self) -> usize {
        self.max_raw_file_size
    }

    /// Maximum number of bytes to scan for embedded JPEG markers.
    #[must_use]
    pub fn jpeg_scan_limit(&self) -> usize {
        self.jpeg_scan_limit
    }

    /// Byte interval between cancellation flag checks during scanning.
    #[must_use]
    pub fn cancel_check_interval(&self) -> usize {
        self.cancel_check_interval
    }

    /// Trailing padding tolerance in bytes (deficit allowed before error).
    #[must_use]
    pub fn trailing_padding_tolerance(&self) -> usize {
        self.trailing_padding_tolerance
    }

    /// Maximum bytes after JPEG SOI to search for JFIF or Exif marker.
    #[must_use]
    pub fn jfif_exif_scan_window(&self) -> usize {
        self.jfif_exif_scan_window
    }
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl DecodeConfig {
    /// Set the maximum raw file size in bytes.
    #[must_use]
    pub fn with_max_raw_file_size(mut self, val: usize) -> Self {
        self.max_raw_file_size = val;
        self
    }

    /// Set the maximum number of bytes to scan for embedded JPEG markers.
    #[must_use]
    pub fn with_jpeg_scan_limit(mut self, val: usize) -> Self {
        self.jpeg_scan_limit = val;
        self
    }

    /// Set the byte interval between cancellation flag checks during scanning.
    #[must_use]
    pub fn with_cancel_check_interval(mut self, val: usize) -> Self {
        self.cancel_check_interval = val;
        self
    }

    /// Set the trailing padding tolerance in bytes.
    #[must_use]
    pub fn with_trailing_padding_tolerance(mut self, val: usize) -> Self {
        self.trailing_padding_tolerance = val;
        self
    }

    /// Set the maximum bytes after JPEG SOI to search for JFIF or Exif marker.
    #[must_use]
    pub fn with_jfif_exif_scan_window(mut self, val: usize) -> Self {
        self.jfif_exif_scan_window = val;
        self
    }
}

// ---------------------------------------------------------------------------
// Global default config
// ---------------------------------------------------------------------------

/// Global default [`DecodeConfig`] that existing entry points use.
///
/// This is lazily initialized to [`DecodeConfig::default()`] on first access.
/// Users who want a fully custom config can ignore this and pass their own
/// `&DecodeConfig` to the `_with_config` function variants.
pub static DEFAULT_CONFIG: OnceLock<DecodeConfig> = OnceLock::new();

/// Return a reference to the global default [`DecodeConfig`], initializing it
/// on the first call.
#[must_use]
pub fn default_config() -> &'static DecodeConfig {
    DEFAULT_CONFIG.get_or_init(DecodeConfig::default)
}
