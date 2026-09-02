#![no_main]

use ithmb_core::config::{default_config, Crop, TransformConfig};
use ithmb_core::pipeline::decode_ithmb_with_transform;
use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

/// Fuzz target: drive the full decode pipeline with runtime transform
/// overrides (rotation + crop) derived from the fuzz input.
///
/// Exercises the crop/rotation post-processing layer that plain
/// `decode_ithmb` (see `fuzz_decode_ithmb`) does not reach, over arbitrary
/// candidate streams (.ithmb raw frames, embedded JPEG, PhotoDB).
///
/// Input layout:
/// ```text
/// byte[0]    — rotation selector (0..=3 → 0/90/180/270 degrees)
/// byte[1]    — crop x      (0..=255)
/// byte[2]    — crop y      (0..=255)
/// byte[3]    — crop width  (clamped to ≤ 2048)
/// byte[4]    — crop height (clamped to ≤ 2048)
/// byte[5..]  — the candidate decode stream
/// ```
///
/// Crop values are kept small on purpose: `apply_crop_with` clamps everything
/// to the image bounds anyway, so the target's job is to stress rotation +
/// crop over arbitrary decode outcomes — not to exercise degenerate crop math
/// (which is already clamped).
fuzz_target!(|data: &[u8]| {
    if data.len() < 5 {
        return;
    }
    let canceled = AtomicBool::new(false);

    // Errors from the decoder itself are expected fuzz outcomes — ignore them.
    let rotation = i32::from(data[0] % 4) * 90;
    let crop = Crop {
        x: i32::from(data[1]),
        y: i32::from(data[2]),
        width: i32::from(data[3]).min(2048),
        height: i32::from(data[4]).min(2048),
    };
    let transform = TransformConfig::default().with_rotation(rotation).with_crop(crop);

    let _ = decode_ithmb_with_transform(&data[5..], &canceled, default_config(), &transform);
});
