//! Decode-timing metrics for ithmb-core.
//!
//! This module provides [`DecodeMetrics`], a set of lightweight atomic counters
//! that track cumulative nanoseconds spent decoding each pixel format.  All
//! operations use [`core::sync::atomic::Ordering::Relaxed`] — the counters are
//! statistical observability aids and do not participate in cross-thread
//! synchronisation.
//!
//! # Format index constants
//!
//! | Index | Constant                | [`Encoding`] variant      |
//! |-------|-------------------------|---------------------------|
//! | 0     | [`M_RGB565`]           | [`Rgb565`]               |
//! | 1     | [`M_RGB555`]           | [`Rgb555`]               |
//! | 2     | [`M_RGB555_REORDERED`] | [`ReorderedRgb555`]      |
//! | 3     | [`M_UYVY`]             | [`Yuv422`]               |
//! | 4     | [`M_YCBCR420`]         | [`Ycbcr420`]             |
//! | 5     | [`M_CLCL`]             | *(reserved)*             |
//! | 6     | [`M_CL`]               | *(reserved)*             |
//! | 7     | [`M_JPEG`]             | [`Jpeg`]                 |
//! | 8     | [`M_TOTAL`]            | —                        |
//!
//! [`Encoding`]: crate::profile::Encoding
//! [`Rgb565`]: crate::profile::Encoding::Rgb565
//! [`Rgb555`]: crate::profile::Encoding::Rgb555
//! [`ReorderedRgb555`]: crate::profile::Encoding::ReorderedRgb555
//! [`Yuv422`]: crate::profile::Encoding::Yuv422
//! [`Ycbcr420`]: crate::profile::Encoding::Ycbcr420
//! [`Jpeg`]: crate::profile::Encoding::Jpeg

use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

use crate::profile::Encoding;

// ---------------------------------------------------------------------------
// Index constants
// ---------------------------------------------------------------------------

/// Index for the RGB565 format counter.
pub const M_RGB565: usize = 0;

/// Index for the RGB555 format counter.
pub const M_RGB555: usize = 1;

/// Index for the Reordered RGB555 format counter.
pub const M_RGB555_REORDERED: usize = 2;

/// Index for the UYVY (YUV 4:2:2) format counter.
pub const M_UYVY: usize = 3;

/// Index for the YCbCr 4:2:0 format counter.
pub const M_YCBCR420: usize = 4;

/// Index for the CLCL nibble-chroma format counter (reserved for future use).
pub const M_CLCL: usize = 5;

/// Index for the CL per-pixel chroma format counter (reserved for future use).
pub const M_CL: usize = 6;

/// Index for the JPEG format counter.
pub const M_JPEG: usize = 7;

/// Index for the total counter (aggregate of all formats).
pub const M_TOTAL: usize = 8;

/// Number of tracked counters: 8 format-specific slots + 1 total.
const NUM_COUNTERS: usize = 9;

// ---------------------------------------------------------------------------
// Encoding → index mapping
// ---------------------------------------------------------------------------

/// Convert an [`Encoding`] to its corresponding metrics array index.
const fn encoding_to_idx(encoding: Encoding) -> usize {
    match encoding {
        Encoding::Rgb565 => M_RGB565,
        Encoding::Rgb555 => M_RGB555,
        Encoding::ReorderedRgb555 => M_RGB555_REORDERED,
        Encoding::Yuv422 => M_UYVY,
        Encoding::Ycbcr420 => M_YCBCR420,
        Encoding::Jpeg => M_JPEG,
    }
}

// ---------------------------------------------------------------------------
// DecodeMetrics
// ---------------------------------------------------------------------------

/// Cumulative decode-timing counters, one per pixel format plus a total.
///
/// All counters are stored as [`AtomicU64`] values representing cumulative
/// nanoseconds.  Every atomic access uses [`Ordering::Relaxed`] — metrics are
/// statistical observability, not correctness-critical synchronisation.
///
/// # Example
///
/// ```rust
/// use ithmb_core::metrics::DecodeMetrics;
/// use ithmb_core::profile::Encoding;
/// use core::time::Duration;
///
/// let m = DecodeMetrics::new();
/// m.record(Encoding::Rgb565, Duration::from_micros(42));
/// assert_eq!(m.rgb565_nanos(), 42_000);
/// assert_eq!(m.nanos(8), 42_000); // total
/// ```
#[derive(Debug)]
pub struct DecodeMetrics {
    counters: [AtomicU64; NUM_COUNTERS],
}

impl DecodeMetrics {
    /// Create a new `DecodeMetrics` with all counters initialised to zero.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            counters: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    /// Record a decode `duration` for the given `encoding`.
    ///
    /// Both the format-specific counter and the total counter are incremented
    /// by the nanosecond equivalent of `duration`.
    pub fn record(&self, encoding: Encoding, duration: Duration) {
        let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.counters[encoding_to_idx(encoding)].fetch_add(nanos, Ordering::Relaxed);
        self.counters[M_TOTAL].fetch_add(nanos, Ordering::Relaxed);
    }

    /// Read the raw nanosecond value of the counter at `idx`.
    ///
    /// # Panics
    ///
    /// Panics if `idx >= NUM_COUNTERS`.
    #[must_use]
    pub fn nanos(&self, idx: usize) -> u64 {
        self.counters.get(idx).map_or(0, |c| c.load(Ordering::Relaxed))
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        for c in &self.counters {
            c.store(0, Ordering::Relaxed);
        }
    }

    /// Take an atomic snapshot of all counters.
    ///
    /// Returns a fixed-size array `[u64; 9]` where the first 8 elements are
    /// the per-format totals (in the order defined by the `M_*` constants)
    /// and the last element is the aggregate total.
    #[must_use]
    pub fn snapshot(&self) -> [u64; NUM_COUNTERS] {
        [
            self.counters[0].load(Ordering::Relaxed),
            self.counters[1].load(Ordering::Relaxed),
            self.counters[2].load(Ordering::Relaxed),
            self.counters[3].load(Ordering::Relaxed),
            self.counters[4].load(Ordering::Relaxed),
            self.counters[5].load(Ordering::Relaxed),
            self.counters[6].load(Ordering::Relaxed),
            self.counters[7].load(Ordering::Relaxed),
            self.counters[8].load(Ordering::Relaxed),
        ]
    }

    // -- Per-format convenience accessors -----------------------------------

    /// Cumulative decode time for RGB565, in nanoseconds.
    #[must_use]
    pub fn rgb565_nanos(&self) -> u64 {
        self.nanos(M_RGB565)
    }

    /// Cumulative decode time for RGB555, in nanoseconds.
    #[must_use]
    pub fn rgb555_nanos(&self) -> u64 {
        self.nanos(M_RGB555)
    }

    /// Cumulative decode time for Reordered RGB555, in nanoseconds.
    #[must_use]
    pub fn rgb555_reordered_nanos(&self) -> u64 {
        self.nanos(M_RGB555_REORDERED)
    }

    /// Cumulative decode time for UYVY (YUV 4:2:2), in nanoseconds.
    #[must_use]
    pub fn uyvy_nanos(&self) -> u64 {
        self.nanos(M_UYVY)
    }

    /// Cumulative decode time for YCbCr 4:2:0, in nanoseconds.
    #[must_use]
    pub fn ycbcr420_nanos(&self) -> u64 {
        self.nanos(M_YCBCR420)
    }

    /// Cumulative decode time for CLCL nibble chroma, in nanoseconds.
    #[must_use]
    pub fn clcl_nanos(&self) -> u64 {
        self.nanos(M_CLCL)
    }

    /// Cumulative decode time for CL per-pixel chroma, in nanoseconds.
    #[must_use]
    pub fn cl_nanos(&self) -> u64 {
        self.nanos(M_CL)
    }

    /// Cumulative decode time for JPEG, in nanoseconds.
    #[must_use]
    pub fn jpeg_nanos(&self) -> u64 {
        self.nanos(M_JPEG)
    }
}
