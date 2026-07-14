//! WASM bindings for ithmb-core.
//!
//! Exposes `#[wasm_bindgen]` functions:
//! - `decode_ithmb(bytes) -> Option<Vec<u8>>`
//! - `peek_prefix(bytes) -> u32`
//! - `get_encoding_name(prefix) -> String`

use wasm_bindgen::prelude::*;

/// Decode a `.ithmb` file from raw bytes into RGBA pixel data.
///
/// Returns `Some(buffer)` on success where the buffer layout is:
///   [width: 4 bytes LE][height: 4 bytes LE][RGBA pixel data ...]
///
/// Returns `None` if decoding fails (unsupported format, corrupt data, etc.).
#[must_use]
#[wasm_bindgen]
pub fn decode_ithmb(bytes: &[u8]) -> Option<Vec<u8>> {
    let canceled = std::sync::atomic::AtomicBool::new(false);
    let img = ithmb_core::decode_ithmb(bytes, &canceled).ok()?;

    // Encode as [u32 width LE][u32 height LE][BGRA → RGBA pixels]
    let pixel_count = (img.width as usize) * (img.height as usize);
    let mut out = Vec::with_capacity(8 + pixel_count * 4);

    out.extend_from_slice(&img.width.to_le_bytes());
    out.extend_from_slice(&img.height.to_le_bytes());

    // Convert BGRA → RGBA since canvas is RGBA
    for chunk in img.data.chunks_exact(4) {
        out.push(chunk[2]); // R
        out.push(chunk[1]); // G
        out.push(chunk[0]); // B
        out.push(chunk[3]); // A
    }

    Some(out)
}

/// Read the 4-byte big-endian format prefix from a byte slice.
///
/// Returns 0 if the slice is shorter than 4 bytes.
#[must_use]
#[wasm_bindgen]
pub fn peek_prefix(bytes: &[u8]) -> u32 {
    if bytes.len() < 4 {
        return 0;
    }
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Look up the human-readable encoding name for a given format prefix.
///
/// Returns `"Unknown format"` if the prefix is not recognized.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
#[wasm_bindgen]
pub fn get_encoding_name(prefix: u32) -> String {
    ithmb_core::encoding_name_for_prefix(prefix as i32)
}
