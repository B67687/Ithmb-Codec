//! Runtime configuration for the decode pipeline.
//!
//! `DecodeConfig` allows callers to customize decode parameters such as
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
use std::fmt;
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// ``DecodeError::BufferTooShort`` (`crate::DecodeError::BufferTooShort`).
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

impl fmt::Display for DecodeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodeConfig")
            .field("max_raw_file_size", &self.max_raw_file_size)
            .field("jpeg_scan_limit", &self.jpeg_scan_limit)
            .field("cancel_check_interval", &self.cancel_check_interval)
            .field("trailing_padding_tolerance", &self.trailing_padding_tolerance)
            .field("jfif_exif_scan_window", &self.jfif_exif_scan_window)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Global default config
// ---------------------------------------------------------------------------

/// Global default ``DecodeConfig`` that existing entry points use.
///
/// This is lazily initialized to ``DecodeConfig::default()`` on first access.
/// Users who want a fully custom config can ignore this and pass their own
/// `&DecodeConfig` to the `_with_config` function variants.
pub static DEFAULT_CONFIG: OnceLock<DecodeConfig> = OnceLock::new();

/// Return a reference to the global default ``DecodeConfig``, initializing it
/// on the first call.
#[must_use]
pub fn default_config() -> &'static DecodeConfig {
    DEFAULT_CONFIG.get_or_init(DecodeConfig::default)
}

// ---------------------------------------------------------------------------
// TransformConfig — runtime decode-parameter overrides (additive spike)
// ---------------------------------------------------------------------------

/// Crop rectangle for runtime decode overrides.
///
/// A width or height of 0 means 'use the remaining span from the offset',
/// mirroring the profile-level crop semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crop {
    /// Left offset in pixels.
    pub x: i32,
    /// Top offset in pixels.
    pub y: i32,
    /// Crop width in pixels (0 = remaining span).
    pub width: i32,
    /// Crop height in pixels (0 = remaining span).
    pub height: i32,
}

/// Runtime overrides for decode parameters (rotation, crop, channel swap,
/// chroma ordering) applied AFTER profile selection, at post-processing.
///
/// Every field is `Option` — `None` means "use the profile's value" (identity).
/// This is the additive counterpart to [`DecodeConfig`] (security/limits); it
/// never renames or replaces it, and it never conflates limits with params.
///
/// # Example
///
/// ```rust
/// use ithmb_core::config::TransformConfig;
///
/// let transform = TransformConfig::default().with_rotation(90);
/// assert_eq!(transform.rotation(), Some(90));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransformConfig {
    /// Clockwise rotation in degrees (0/90/180/270; others ignored).
    rotation: Option<i32>,
    /// Crop rectangle override.
    crop: Option<Crop>,
    /// Swap RGB channel order (BGRA <-> RGBA), if set.
    swap_rgb_channels: Option<bool>,
    /// Swap chroma planes, if set.
    swap_chroma_planes: Option<bool>,
    /// CLCL chroma-subsampling override, if set.
    clcl_chroma: Option<bool>,
    /// CL chroma-subsampling override, if set.
    cl_chroma: Option<bool>,
}

impl TransformConfig {
    /// Override rotation (clockwise degrees; 0/90/180/270).
    #[must_use]
    pub fn rotation(&self) -> Option<i32> {
        self.rotation
    }

    /// Override crop rectangle.
    #[must_use]
    pub fn crop(&self) -> Option<Crop> {
        self.crop
    }

    /// Override RGB channel swap.
    #[must_use]
    pub fn swap_rgb_channels(&self) -> Option<bool> {
        self.swap_rgb_channels
    }

    /// Override chroma-plane swap.
    #[must_use]
    pub fn swap_chroma_planes(&self) -> Option<bool> {
        self.swap_chroma_planes
    }

    /// Override CLCL chroma-subsampling flag.
    #[must_use]
    pub fn clcl_chroma(&self) -> Option<bool> {
        self.clcl_chroma
    }

    /// Override CL chroma-subsampling flag.
    #[must_use]
    pub fn cl_chroma(&self) -> Option<bool> {
        self.cl_chroma
    }
}

impl TransformConfig {
    /// Set the rotation override (clockwise degrees).
    #[must_use]
    pub fn with_rotation(mut self, val: i32) -> Self {
        self.rotation = Some(val);
        self
    }

    /// Set the crop rectangle override.
    #[must_use]
    pub fn with_crop(mut self, val: Crop) -> Self {
        self.crop = Some(val);
        self
    }

    /// Set the RGB channel-swap override.
    #[must_use]
    pub fn with_swap_rgb_channels(mut self, val: bool) -> Self {
        self.swap_rgb_channels = Some(val);
        self
    }

    /// Set the chroma-plane-swap override.
    #[must_use]
    pub fn with_swap_chroma_planes(mut self, val: bool) -> Self {
        self.swap_chroma_planes = Some(val);
        self
    }

    /// Set the CLCL chroma-subsampling override.
    #[must_use]
    pub fn with_clcl_chroma(mut self, val: bool) -> Self {
        self.clcl_chroma = Some(val);
        self
    }

    /// Set the CL chroma-subsampling override.
    #[must_use]
    pub fn with_cl_chroma(mut self, val: bool) -> Self {
        self.cl_chroma = Some(val);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Crop, TransformConfig};

    #[test]
    fn transform_config_default_is_identity() {
        let t = TransformConfig::default();
        assert_eq!(t.rotation(), None);
        assert_eq!(t.crop(), None);
        assert_eq!(t.swap_rgb_channels(), None);
        assert_eq!(t.swap_chroma_planes(), None);
        assert_eq!(t.clcl_chroma(), None);
        assert_eq!(t.cl_chroma(), None);
    }

    #[test]
    fn builder_sets_single_field_others_remain_none() {
        let t = TransformConfig::default().with_rotation(90);
        assert_eq!(t.rotation(), Some(90));
        assert_eq!(t.crop(), None);
        assert_eq!(t.swap_rgb_channels(), None);
    }

    #[test]
    fn crop_default_is_zero_and_roundtrips() {
        let c = Crop::default();
        assert_eq!((c.x, c.y, c.width, c.height), (0, 0, 0, 0));
        let built = Crop {
            x: 4,
            y: 2,
            width: 10,
            height: 20,
        };
        let t = TransformConfig::default().with_crop(built);
        assert_eq!(t.crop(), Some(built));
    }
}
