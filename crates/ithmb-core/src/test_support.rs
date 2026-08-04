//! Shared decode test suite for the RGB555 family decoders.
//!
//! `rgb555.rs` and `reordered_rgb555.rs` historically block-copied ~700 lines
//! of near-identical unit tests. The shared assertions live here once; each
//! decoder module runs the suite against its own `decode` + profile factory,
//! keeping coverage identical (the assertions are byte-for-byte the historic
//! ones). Module-specific tests (endianness, Morton order, golden vectors)
//! stay in their own modules.

#![allow(clippy::pedantic, clippy::unwrap_used)]

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use std::sync::atomic::AtomicBool;

/// Runs the shared RGB555-family assertion suite.
///
/// * `decode` — the decoder under test (`fn(&[u8], &Profile, &AtomicBool)`).
/// * `make` — profile factory `(width, height, swap_rgb_channels)` with
///   little-endian byte order (the shared suite only exercises LE inputs).
pub(crate) fn run_rgb555_family_suite(
    decode: impl Fn(&[u8], &Profile, &AtomicBool) -> Result<DecodedImage, DecodeError>,
    make: impl Fn(i32, i32, bool) -> Profile,
) {
    // 1. zero_dimensions_returns_err
    let result = decode(b"", &make(0, 100, false), &AtomicBool::new(false));
    assert!(result.is_err());
    assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));

    // 2. negative_dimension_returns_err
    let result = decode(b"", &make(-1, 100, false), &AtomicBool::new(false));
    assert!(result.is_err());
    assert!(matches!(result, Err(DecodeError::InvalidFormat(_))));

    // 3. too_short_returns_buffer_too_short
    let profile = make(100, 100, false);
    let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
    assert!(result.is_err());
    assert!(matches!(result, Err(DecodeError::BufferTooShort { .. })));

    // 4. buffer_too_short_reports_exact_counts
    let profile = make(14, 10, false);
    // 14*10*2 = 280 needed, deficit=270 > 256 → still BufferTooShort
    let result = decode(&[0u8; 10], &profile, &AtomicBool::new(false));
    match result {
        Err(DecodeError::BufferTooShort {
            expected: 280,
            actual: 10,
        }) => {}
        other => panic!("expected BufferTooShort(280, 10), got {other:?}"),
    }

    // 5. dst_allocation_matches_geometry
    let profile = make(3, 2, false);
    let pixels = vec![0u8; 3 * 2 * 2];
    let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data.len(), 3 * 2 * 4);
    assert_eq!(img.width, 3);
    assert_eq!(img.height, 2);

    // 6. solid_white_pixel
    let profile = make(1, 1, false);
    let img = decode(&[0xFF, 0x7F], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0xFF, 0xFF, 0xFF, 255]);

    // 7. solid_black_pixel
    let profile = make(1, 1, false);
    let img = decode(&[0x00, 0x00], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0, 0, 0, 255]);

    // 8. solid_red_pixel
    // Layout xRRRRRGGGGGBBBBB, R=31 → 0x7C00, LE: [0x00, 0x7C]
    let profile = make(1, 1, false);
    let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0, 0, 0xFF, 255]);

    // 9. solid_blue_pixel
    // B=31 → 0x001F, LE: [0x1F, 0x00]
    let profile = make(1, 1, false);
    let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0xFF, 0, 0, 255]);

    // 10. solid_green_pixel
    // G=31 → 0x03E0, LE: [0xE0, 0x03]
    let profile = make(1, 1, false);
    let img = decode(&[0xE0, 0x03], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0, 0xFF, 0, 255]);

    // 11. decode_swap_rgb_channels
    // swap: layout becomes xBBBBBGGGGGRRRRR; high bits = blue → B_out=255
    let profile = make(1, 1, true);
    let img = decode(&[0x00, 0x7C], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0xFF, 0, 0, 255]);

    // 12. swap_mode_red_stays_low
    // swap: low bits = red → R_out=255
    let profile = make(1, 1, true);
    let img = decode(&[0x1F, 0x00], &profile, &AtomicBool::new(false)).unwrap();
    assert_eq!(img.data, vec![0, 0, 0xFF, 255]);

    // 13. msb_replicate_clamping
    assert_eq!(crate::pixel_utils::msb_replicate_5(31), 0xFF);
    assert_eq!(crate::pixel_utils::msb_replicate_5(0), 0x00);
    assert_eq!(crate::pixel_utils::msb_replicate_5(16), 0x84);
    assert_eq!(crate::pixel_utils::msb_replicate_5(8), 0x42);
    assert_eq!(crate::pixel_utils::msb_replicate_5(1), 0x08);

    // 14. output_has_correct_alpha
    let profile = make(2, 2, false);
    let pixels = vec![0u8; 8];
    let img = decode(&pixels, &profile, &AtomicBool::new(false)).unwrap();
    for i in 0..4 {
        assert_eq!(img.data[i * 4 + 3], 255);
    }
}
