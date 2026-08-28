//! Runtime configuration for decode-parameter overrides (rotation, crop,
//! channel swap, chroma ordering).
//!
//! Applied AFTER profile selection, at post-processing. Every field is
//! `Option` — `None` means "use the profile's value" (identity).

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
/// This is the additive counterpart to [`super::DecodeConfig`] (security/limits); it
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
    /// NOTE: not consumed by any current decoder — kept for API completeness;
    /// do not delete without a semver-aware contract check.
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
