//! Profile types — the Rust equivalent of `IthmbVariantProfile`.
//!
//! A profile describes how to decode a raw .ithmb frame identified by a 4-byte
//! prefix (format ID). Each profile specifies dimensions, encoding, byte order,
//! and optional post-processing (rotation, crop, padded slots).

/// Known encoding variants for raw .ithmb frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Encoding {
    /// 16-bit RGB (5 bits red, 6 bits green, 5 bits blue).
    Rgb565,
    /// 15-bit RGB (5 bits per channel, 1 unused bit per pixel).
    Rgb555,
    /// Byte-swapped RGB555 variant used by some iPod models.
    ReorderedRgb555,
    /// UYVY 4:2:2 packed chroma subsampling.
    Yuv422,
    /// YCbCr 4:2:0 planar chroma subsampling.
    Ycbcr420,
    /// JPEG-compressed frame embedded within the raw file.
    Jpeg,
}

/// Decoding profile for a raw .ithmb frame format.
///
/// Maps to `IthmbVariantProfile` in the C# implementation. Each field that
/// defaults to `false`/`0`/`None` can be omitted — the decoder uses the
/// standard behaviour unless overridden.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct Profile {
    /// Big-endian 4-byte prefix identifying this format.
    pub prefix: i32,

    /// Frame width in pixels.
    pub width: i32,

    /// Frame height in pixels.
    pub height: i32,

    /// Pixel encoding.
    pub encoding: Encoding,

    /// Number of bytes in one complete frame (not counting padding).
    pub frame_byte_length: i32,

    // ---- Optional overrides ----
    /// If true, width and height are swapped after decode.
    pub swaps_dimensions: bool,

    /// If false (default), pixel data is little-endian.
    pub little_endian: bool,

    /// If true, frame occupies a fixed-size slot with padding.
    pub is_padded: bool,

    /// If true, even/odd scanlines are stored as separate fields.
    pub is_interlaced: bool,

    /// If true, chroma uses CLCL shared-nibble layout.
    pub clcl_chroma: bool,

    /// If true, swap Cb/Cr plane order in YCbCr 4:2:0.
    pub swap_chroma_planes: bool,

    /// If true, chroma uses CL per-pixel nibble layout.
    pub cl_chroma: bool,

    /// If true, swap R and B channels (BGR15 iPhone compatibility).
    pub swap_rgb_channels: bool,

    /// Post-decode clockwise rotation in degrees (0, 90, 180, 270).
    pub rotation: i32,

    /// Visible-region X offset (applied after rotation).
    pub crop_x: i32,

    /// Visible-region Y offset (applied after rotation).
    pub crop_y: i32,

    /// Visible-region width (0 = full width).
    pub crop_width: i32,

    /// Visible-region height (0 = full height).
    pub crop_height: i32,

    /// Slot size in bytes for padded profiles.
    pub slot_size: i32,

    /// If true, use actual Width/Height from MHNI chunk instead of fixed values.
    pub use_mhni_dimensions: bool,

    /// Ordered list of fallback encodings to try if primary decode fails.
    pub fallback_encodings: Option<Vec<Encoding>>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            prefix: 0,
            width: 0,
            height: 0,
            encoding: Encoding::Rgb565,
            frame_byte_length: 0,
            swaps_dimensions: false,
            little_endian: true,
            is_padded: false,
            is_interlaced: false,
            clcl_chroma: false,
            swap_chroma_planes: false,
            cl_chroma: false,
            swap_rgb_channels: false,
            rotation: 0,
            crop_x: 0,
            crop_y: 0,
            crop_width: 0,
            crop_height: 0,
            slot_size: 0,
            use_mhni_dimensions: false,
            fallback_encodings: None,
        }
    }
}

impl Profile {
    /// The number of bytes per frame, accounting for slot padding.
    ///
    /// For padded profiles the frame stride is `slot_size`; for unpadded it
    /// equals `frame_byte_length`.
    #[must_use]
    pub fn frame_size(&self) -> i32 {
        if self.is_padded && self.slot_size > 0 {
            self.slot_size
        } else {
            self.frame_byte_length
        }
    }

    /// The number of pixel columns in the frame.
    #[must_use]
    pub fn display_width(&self) -> i32 {
        if self.swaps_dimensions { self.height } else { self.width }
    }

    /// The number of pixel rows in the frame.
    #[must_use]
    pub fn display_height(&self) -> i32 {
        if self.swaps_dimensions { self.width } else { self.height }
    }
}

// ---------------------------------------------------------------------------
// Profile database access
// ---------------------------------------------------------------------------

/// Returns the built-in set of known profiles.
/// Returns a list of all known built-in profiles.
///
/// If the embedded profile database cannot be parsed (JSON corruption),
/// an empty `Vec` is returned instead of panicking. Use [`crate::profile_db::ProfileDb::load_builtin`]
#[must_use]
pub fn built_in_profiles() -> Vec<Profile> {
    match crate::profile_db::ProfileDb::load_builtin() {
        Ok(db) => db.all().values().cloned().collect(),
        Err(_) => vec![],
    }
}
