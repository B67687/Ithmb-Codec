//! JPEG detection and passthrough decoder for `.ithmb` files.
//!
//! T-prefix `.ithmb` files contain an embedded JPEG stream rather than raw pixel
//! data. This module detects JPEG streams by their SOI marker, decodes them
//! via the `zune_jpeg` crate, applies EXIF orientation if present, and
//! outputs BGRA8 pixel data.

mod exif;

pub(crate) use exif::{extract_exif_orientation, rotate_bgra};

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
#[cfg(feature = "logging")]
use log::debug;
use std::io::Cursor;
use std::sync::atomic::AtomicBool;

/// Maximum decoded pixel-buffer budget for a single JPEG, in the worst-case
/// bytes-per-pixel factor (w×h×11: RGB out + BGRA conversion + rotation copy).
/// ~256 MiB allows roughly 4940×4940 px — far beyond any real ithmb
/// thumbnail (≤ 2048 px) — while bounding hostile progressive-JPEG
/// allocations (CWE-400: a 166-byte SOF2-65535×65535 stream once triggered
/// an ~8 GiB allocation that aborted the process).
const MAX_JPEG_PIXEL_BYTES: u64 = 256 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if `src` starts with a JPEG SOI marker (`0xFF`, `0xD8`).
#[must_use]
pub fn is_jpeg(src: &[u8]) -> bool {
    src.first().is_some_and(|&b| b == 0xFF) && src.get(1).is_some_and(|&b| b == 0xD8)
}

/// Decodes a JPEG stream to BGRA8 output.
///
/// # Errors
///
/// Returns [`DecodeError::BufferTooShort`] if the input is shorter than the
/// SOI marker. Returns [`DecodeError::InvalidFormat`] if the input is not a
/// valid JPEG stream. Returns [`DecodeError::Jpeg`] if the underlying JPEG
/// decoder fails or the frame dimensions exceed the decode budget.
///
/// # Panics
///
/// Never in practice: the 256 MiB decode budget always fits a `usize` on
/// every supported target (including 32-bit wasm).
pub fn decode(src: &[u8], _profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    #[cfg(feature = "logging")]
    debug!("jpeg::decode: len={}", src.len());
    if src.len() < 2 {
        return Err(DecodeError::BufferTooShort {
            expected: 2,
            actual: src.len(),
        });
    }
    if !is_jpeg(src) {
        return Err(DecodeError::InvalidFormat("not a JPEG stream".into()));
    }

    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
        Cursor::new(src),
        // Belt-and-braces: cap the per-axis dimensions so the decoder's own
        // SOF-time limit never admits a frame our explicit w·h·11 budget
        // below would reject. u16::MAX is the largest representable JPEG
        // dimension, so this check is inert for real streams and the explicit
        // pre-check remains the sole gate — identical to jpeg-decoder 0.3.2.
        zune_jpeg::zune_core::options::DecoderOptions::default()
            .set_max_width(usize::from(u16::MAX))
            .set_max_height(usize::from(u16::MAX)),
    );

    // Security: reject oversized frames BEFORE decoding (CWE-400). Both
    // jpeg-decoder 0.3.2 and zune-jpeg allocate the progressive-JPEG
    // coefficient buffer from the frame dimensions alone — a 166-byte
    // SOF2-65535×65535 stream triggers an ~8 GiB allocation that aborts the
    // process. decode_headers() parses only the frame header (SOF) with zero
    // pixel allocations, so we can check dimensions first.
    decoder.decode_headers().map_err(|e| DecodeError::Jpeg(e.to_string()))?;
    let info = decoder
        .info()
        .ok_or_else(|| DecodeError::Jpeg("no JPEG metadata".into()))?;
    let frame_w = u64::from(info.width);
    let frame_h = u64::from(info.height);
    // The budget is on w×h×11 — the measured worst-case bytes-per-pixel for
    // a decoded JPEG (3 RGB out + 4 BGRA conversion + 4 EXIF-rotation copy;
    // the old w×h×3 check let a ~9450×9450 frame transiently allocate ~944
    // MiB). This bounds the peak (coeff ≤ budget + RGB ≤ budget + BGRA +
    // rotation copy) at ~256 MiB while still admitting every real ithmb
    // thumbnail (≤ 2048 px).
    if frame_w.saturating_mul(frame_h).saturating_mul(11) > MAX_JPEG_PIXEL_BYTES {
        return Err(DecodeError::Jpeg(format!(
            "JPEG dimensions {}x{} exceed the {} byte decode budget",
            info.width, info.height, MAX_JPEG_PIXEL_BYTES,
        )));
    }

    let pixels = decoder.decode().map_err(|e| DecodeError::Jpeg(e.to_string()))?;

    let w = u32::from(info.width);
    let h = u32::from(info.height);

    // `pixels` is RGB (3 bytes per pixel) — convert to BGRA8.
    let pixel_count = (w * h) as usize;
    let mut data = vec![0u8; pixel_count * 4];

    for (i, chunk) in pixels.chunks_exact(3).enumerate() {
        crate::pixel_utils::check_canceled(canceled, "jpeg decode canceled")?;
        if i >= pixel_count {
            break;
        }
        let dst = i * 4;
        data[dst] = chunk[2]; // B
        data[dst + 1] = chunk[1]; // G
        data[dst + 2] = chunk[0]; // R
        data[dst + 3] = 255; // A
    }

    // Check EXIF orientation and rotate if needed.
    let orientation = extract_exif_orientation(src);
    if orientation > 1 {
        let (rotated_data, rw, rh) = rotate_bgra(&data, w, h, orientation);
        return Ok(DecodedImage {
            data: rotated_data,
            width: rw,
            height: rh,
        });
    }

    Ok(DecodedImage {
        data,
        width: w,
        height: h,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// A valid 2×2 black JPEG generated by ffmpeg.
    const TEST_JPEG: &[u8] = &[
        0xff, 0xd8, 0xff, 0xfe, 0x00, 0x0f, 0x4c, 0x61, 0x76, 0x63, 0x36, 0x31, 0x2e, 0x33, 0x2e, 0x31, 0x30, 0x30,
        0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
        0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x07, 0x07, 0x07, 0x08, 0x08,
        0x08, 0x07, 0x07, 0x07, 0x06, 0x06, 0x07, 0x07, 0x08, 0x08, 0x08, 0x08, 0x09, 0x09, 0x09, 0x08, 0x08, 0x08,
        0x08, 0x09, 0x09, 0x0a, 0x0a, 0x0a, 0x0c, 0x0c, 0x0b, 0x0b, 0x0e, 0x0e, 0x0e, 0x11, 0x11, 0x14, 0xff, 0xc4,
        0x00, 0x4b, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x02, 0x03, 0x01, 0x12, 0x00, 0x02, 0x12,
        0x00, 0x03, 0x12, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00,
        0x9f, 0xc0, 0x07, 0xff, 0xd9,
    ];

    /// A JPEG with EXIF orientation tag = 6 (rotate 90° CW).
    const TEST_JPEG_EXIF6: &[u8] = &[
        0xff, 0xd8, 0xff, 0xe1, 0x00, 0x20, 0x45, 0x78, 0x69, 0x66, 0x00, 0x00, 0x49, 0x49, 0x2a, 0x00, 0x08, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xfe, 0x00, 0x0f, 0x4c, 0x61, 0x76, 0x63, 0x36, 0x31, 0x2e, 0x33, 0x2e, 0x31, 0x30, 0x30, 0x00, 0xff,
        0xdb, 0x00, 0x43, 0x00, 0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x06, 0x06,
        0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x07, 0x07, 0x07, 0x08, 0x08, 0x08, 0x07, 0x07, 0x07,
        0x06, 0x06, 0x07, 0x07, 0x08, 0x08, 0x08, 0x08, 0x09, 0x09, 0x09, 0x08, 0x08, 0x08, 0x08, 0x09, 0x09, 0x0a,
        0x0a, 0x0a, 0x0c, 0x0c, 0x0b, 0x0b, 0x0e, 0x0e, 0x0e, 0x11, 0x11, 0x14, 0xff, 0xc4, 0x00, 0x4b, 0x00, 0x01,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x01,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
        0xc0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x02, 0x03, 0x01, 0x12, 0x00, 0x02, 0x12, 0x00, 0x03, 0x12, 0x00,
        0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0x9f, 0xc0, 0x07, 0xff,
        0xd9,
    ];

    #[test]
    fn is_jpeg_detects_soi() {
        assert!(is_jpeg(&[0xFF, 0xD8]));
        assert!(is_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(is_jpeg(TEST_JPEG));
    }

    #[test]
    fn is_jpeg_rejects_non_jpeg() {
        assert!(!is_jpeg(&[0x00, 0x00]));
        assert!(!is_jpeg(&[0xFF, 0x00]));
        assert!(!is_jpeg(&[0x00, 0xD8]));
    }

    #[test]
    fn is_jpeg_rejects_short_input() {
        assert!(!is_jpeg(&[]));
        assert!(!is_jpeg(&[0xFF]));
    }

    #[test]
    fn decode_short_input_returns_buffer_too_short() {
        let profile = Profile::default();
        let result = decode(&[], &profile, &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 2, actual: 0 })
        ));

        let result = decode(&[0xFF], &profile, &AtomicBool::new(false));
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { expected: 2, actual: 1 })
        ));
    }

    #[test]
    fn decode_non_jpeg_returns_invalid_format() {
        let profile = Profile::default();
        let result = decode(&[0x00, 0x00, 0x00, 0x00], &profile, &AtomicBool::new(false));
        assert!(matches!(result, Err(DecodeError::InvalidFormat(..))));
    }

    #[test]
    fn decode_invalid_jpeg_returns_jpeg_error() {
        // SOI marker present but no valid JPEG structure after it.
        let profile = Profile::default();
        let result = decode(&[0xFF, 0xD8, 0xFF, 0xD9], &profile, &AtomicBool::new(false));
        assert!(matches!(result, Err(DecodeError::Jpeg(..))));
    }

    #[test]
    fn decode_valid_jpeg() {
        let profile = Profile::default();
        let result = decode(TEST_JPEG, &profile, &AtomicBool::new(false));
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
        let img = result.unwrap();
        // 2×2 image, 16 bytes of BGRA data.
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.data.len(), 2 * 2 * 4);
    }

    #[test]
    fn decode_bgra_output_format() {
        let profile = Profile::default();
        let img = decode(TEST_JPEG, &profile, &AtomicBool::new(false)).unwrap();
        // Every pixel should be 4 bytes with alpha = 255.
        for chunk in img.data.chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }
        // Data length should be width * height * 4.
        assert_eq!(img.data.len(), (img.width * img.height * 4) as usize);
    }

    /// Builds a minimal progressive JPEG (`SOF2`) declaring 65535×65535 — the
    /// `CWE-400` regression fixture. `jpeg-decoder` 0.3.2 allocated an ~8 GiB
    /// coefficient buffer from these headers alone (`SIGABRT`) before the
    /// `read_info` pre-check existed. Mirrors the byte-identical 193-byte
    /// artifact verified by the security-research `PoC` engineers.
    fn huge_progressive_jpeg() -> Vec<u8> {
        fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
            let mut out = Vec::with_capacity(payload.len() + 4);
            out.extend_from_slice(&[0xFF, marker]);
            let seg_len = u16::try_from(payload.len() + 2).expect("test segment fits u16");
            out.extend_from_slice(&seg_len.to_be_bytes());
            out.extend_from_slice(payload);
            out
        }
        // DQT: precision 0, tables 0 and 1, all elements nonzero.
        let mut dqt = vec![0x00];
        dqt.extend_from_slice(&[0x01; 64]);
        dqt.push(0x01);
        dqt.extend_from_slice(&[0x01; 64]);
        let dqt = segment(0xDB, &dqt);
        // SOF2 (progressive): prec=8, H=65535, W=65535, 3 components.
        let sof2 = segment(
            0xC2,
            &[8, 0xFF, 0xFF, 0xFF, 0xFF, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1],
        );
        // DHT: one DC table (class 0, index 1), one symbol.
        let mut huff = vec![0x01, 0x01];
        huff.extend_from_slice(&[0x00; 15]);
        huff.push(0x00);
        let huff = segment(0xC4, &huff);
        // SOS: DC-only progressive first scan.
        let sos = segment(0xDA, &[3, 1, 0x10, 2, 0x10, 3, 0x10, 0, 0, 0]);
        let mut jpeg = vec![0xFF, 0xD8];
        jpeg.extend_from_slice(&dqt);
        jpeg.extend_from_slice(&sof2);
        jpeg.extend_from_slice(&huff);
        jpeg.extend_from_slice(&sos);
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        jpeg
    }

    #[test]
    fn decode_rejects_oversized_progressive_jpeg() {
        // The read_info pre-check must reject the frame BEFORE decode() can
        // allocate the ~8 GiB progressive coefficient buffer. Regression for
        // CWE-400 (SIGABRT on a 166-byte SOF2-65535×65535 stream).
        let profile = Profile::default();
        let jpeg = huge_progressive_jpeg();
        assert_eq!(jpeg.len(), 193, "fixture drifted from the verified artifact");
        let result = decode(&jpeg, &profile, &AtomicBool::new(false));
        assert!(
            matches!(result, Err(DecodeError::Jpeg(ref msg)) if msg.contains("exceed")),
            "expected dimension-budget rejection, got {result:?}",
        );
    }

    #[test]
    fn extract_exif_orientation_normal_when_no_exif() {
        // JPEG without EXIF data should return orientation 1.
        assert_eq!(extract_exif_orientation(TEST_JPEG), 1);
    }

    #[test]
    fn extract_exif_orientation_found() {
        // TEST_JPEG_EXIF6 has orientation = 6.
        assert_eq!(extract_exif_orientation(TEST_JPEG_EXIF6), 6);
    }

    #[test]
    fn extract_exif_orientation_short_input() {
        assert_eq!(extract_exif_orientation(&[]), 1);
        assert_eq!(extract_exif_orientation(&[0xFF, 0xD8]), 1);
    }

    #[test]
    fn exif_rotation_180() {
        // Create 2×2 pixel data: top-left=red, top-right=green,
        // bottom-left=blue, bottom-right=white.
        let w = 2u32;
        let h = 2u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // Pixel layout: [R, G, B, A] in BGRA format.
        // Top-left (0,0): red → B=0, G=0, R=255, A=255
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // Top-right (1,0): green → B=0, G=255, R=0, A=255
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // Bottom-left (0,1): blue → B=255, G=0, R=0, A=255
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // Bottom-right (1,1): white → B=255, G=255, R=255, A=255
        data[12..16].copy_from_slice(&[255, 255, 255, 255]);

        // After 180° rotation, pixel mapping:
        // (0,0) → (1,1), (1,0) → (0,1), (0,1) → (1,0), (1,1) → (0,0)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 3);
        assert_eq!(rw, 2);
        assert_eq!(rh, 2);
        // (1,1) should be red
        assert_eq!(&rotated[12..16], &[0, 0, 255, 255]);
        // (0,1) should be green
        assert_eq!(&rotated[8..12], &[0, 255, 0, 255]);
        // (1,0) should be blue
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]);
        // (0,0) should be white
        assert_eq!(&rotated[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn exif_rotation_90cw() {
        // 2×3 pixel data for 90° CW rotation test.
        // Using a 2-wide, 3-tall image to make dimension swap obvious.
        let w = 2u32;
        let h = 3u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // Fill with distinct colors: row-major order.
        // (0,0): red
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // (1,0): green
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // (0,1): blue
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // (1,1): yellow (R+G)
        data[12..16].copy_from_slice(&[0, 255, 255, 255]);
        // (0,2): cyan (G+B)
        data[16..20].copy_from_slice(&[255, 255, 0, 255]);
        // (1,2): magenta (R+B)
        data[20..24].copy_from_slice(&[255, 0, 255, 255]);

        // 90° CW: old (x, y) → new (h-1-y, x)
        // (0,0)→(2,0), (1,0)→(2,1), (0,1)→(1,0), (1,1)→(1,1), (0,2)→(0,0), (1,2)→(0,1)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 6);
        // Dimensions swap: new width = h = 3, new height = w = 2
        assert_eq!(rw, 3);
        assert_eq!(rh, 2);

        // Output row-major layout (width=3, height=2):
        // (0,0) = old(0,2) = cyan
        assert_eq!(&rotated[0..4], &[255, 255, 0, 255]);
        // (1,0) = old(0,1) = blue
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]);
        // (2,0) = old(0,0) = red
        assert_eq!(&rotated[8..12], &[0, 0, 255, 255]);
        // (0,1) = old(1,2) = magenta
        assert_eq!(&rotated[12..16], &[255, 0, 255, 255]);
        // (1,1) = old(1,1) = yellow
        assert_eq!(&rotated[16..20], &[0, 255, 255, 255]);
        // (2,1) = old(1,0) = green
        assert_eq!(&rotated[20..24], &[0, 255, 0, 255]);
    }

    #[test]
    fn exif_rotation_270cw() {
        let w = 2u32;
        let h = 3u32;
        let mut data = vec![0u8; (w * h * 4) as usize];
        // (0,0): red
        data[0..4].copy_from_slice(&[0, 0, 255, 255]);
        // (1,0): green
        data[4..8].copy_from_slice(&[0, 255, 0, 255]);
        // (0,1): blue
        data[8..12].copy_from_slice(&[255, 0, 0, 255]);
        // (1,1): yellow
        data[12..16].copy_from_slice(&[0, 255, 255, 255]);
        // (0,2): cyan
        data[16..20].copy_from_slice(&[255, 255, 0, 255]);
        // (1,2): magenta
        data[20..24].copy_from_slice(&[255, 0, 255, 255]);

        // 270° CW: old (x, y) → new (y, w-1-x)
        // (0,0)→(0,1), (1,0)→(0,0), (0,1)→(1,1), (1,1)→(1,0), (0,2)→(2,1), (1,2)→(2,0)
        let (rotated, rw, rh) = rotate_bgra(&data, w, h, 8);
        assert_eq!(rw, 3);
        assert_eq!(rh, 2);

        // Output row-major (width=3, height=2):
        // (0,0) = old(1,0) = green
        assert_eq!(&rotated[0..4], &[0, 255, 0, 255]);
        // (1,0) = old(1,1) = yellow
        assert_eq!(&rotated[4..8], &[0, 255, 255, 255]);
        // (2,0) = old(1,2) = magenta
        assert_eq!(&rotated[8..12], &[255, 0, 255, 255]);
        // (0,1) = old(0,0) = red
        assert_eq!(&rotated[12..16], &[0, 0, 255, 255]);
        // (1,1) = old(0,1) = blue
        assert_eq!(&rotated[16..20], &[255, 0, 0, 255]);
        // (2,1) = old(0,2) = cyan
        assert_eq!(&rotated[20..24], &[255, 255, 0, 255]);
    }

    #[test]
    #[ignore = "TD-006: TEST_JPEG_EXIF6 has malformed Huffman data — fix with valid fixture"]
    fn decode_with_exif_rotation() {
        let profile = Profile::default();
        let result = decode(TEST_JPEG_EXIF6, &profile, &AtomicBool::new(false));
        // The JPEG has EXIF orientation = 6, so dimensions should be swapped
        // (2×2 → still 2×2 since it's square, but rotation logic applies).
        assert!(result.is_ok(), "decode failed: {:?}", result.err());
    }

    #[test]
    fn rotate_bgra_noop_for_normal_orientation() {
        let data = vec![0u8; 16];
        let (rotated, rw, rh) = rotate_bgra(&data, 2, 2, 1);
        assert_eq!(rotated, data);
        assert_eq!(rw, 2);
        assert_eq!(rh, 2);
    }
}
