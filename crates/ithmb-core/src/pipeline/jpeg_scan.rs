//! Embedded JPEG scanning -- SOI-to-EOI extraction with marker validation.

use std::sync::atomic::AtomicBool;

/// JFIF marker bytes (including null terminator).
pub(crate) const JFIF_MARKER: &[u8] = b"JFIF\x00";

/// Exif marker bytes (including nulls).
pub(crate) const EXIF_MARKER: &[u8] = b"Exif\x00\x00";

/// Check if a valid JFIF or Exif marker exists within the scan window after SOI.
pub(crate) fn has_jpeg_marker(src: &[u8], soi_pos: usize, jfif_exif_scan_window: usize) -> bool {
    let end = (soi_pos + jfif_exif_scan_window).min(src.len());
    let window = &src[soi_pos..end];

    window.windows(JFIF_MARKER.len()).any(|w| w == JFIF_MARKER)
        || window.windows(EXIF_MARKER.len()).any(|w| w == EXIF_MARKER)
}

/// Scan the buffer for an embedded JPEG stream (SOI to EOI) with marker validation.
///
/// Returns the JPEG data slice if found. Some .ithmb files have unregistered
/// format prefixes but contain a complete JPEG stream within the pixel data.
/// This function validates that a JFIF or Exif marker exists near the SOI to
/// avoid false positives from random pixel data containing 0xFF 0xD8.
pub(crate) fn scan_for_embedded_jpeg<'a>(
    src: &'a [u8],
    canceled: &AtomicBool,
    jpeg_scan_limit: usize,
    cancel_check_interval: usize,
    jfif_exif_scan_window: usize,
) -> Option<&'a [u8]> {
    let scan_limit = src.len().min(jpeg_scan_limit);
    let scan_src = &src[..scan_limit];
    let mut search_start = 0;
    let mut bytes_since_check: usize = 0;
    loop {
        let soi = scan_src[search_start..].windows(2).position(|w| w == b"\xff\xd8")?;
        let soi_abs = search_start + soi;

        if has_jpeg_marker(scan_src, soi_abs, jfif_exif_scan_window) {
            let after_soi = &src[soi_abs + 2..];
            if let Some(eoi) = after_soi.windows(2).position(|w| w == b"\xff\xd9") {
                return Some(&src[soi_abs..=soi_abs + 2 + eoi + 1]);
            }
        }
        // Skip past this SOI and continue scanning for the next one.
        search_start = soi_abs + 2;

        // Periodic cancellation check.
        bytes_since_check += 2;
        if bytes_since_check >= cancel_check_interval {
            if canceled.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            bytes_since_check = 0;
        }
    }
}
