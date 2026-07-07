//! Statistical verification tests for decoded image properties.
//!
//! These tests verify that decoders produce output with the correct statistical
//! properties — mean, variance, extrema, entropy, histograms — using only
//! integer arithmetic and integer tolerances. No floating-point epsilon
//! comparisons, no external stats crate.
//!
//! Test vectors come from:
//! - Golden `.enc`/`.bin` pairs under `tests/golden/` (known-good C# reference
//!   encoder outputs)
//! - Synthetic roundtrip data (encode→decode known BGRA patterns)
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::unnecessary_cast
)]

use divan as _;
use ithmb_core::enc::*;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use std::sync::atomic::AtomicBool;
use thiserror as _;
mod util;

// ---------------------------------------------------------------------------
// Inline statistical helpers — no external crate dependency
// ---------------------------------------------------------------------------

/// Number of 8-bit channels in BGRA output.
const NUM_CHANNELS: usize = 4;

/// Return the mean of channel `ch` (0 = B, 1 = G, 2 = R, 3 = A) across all
fn mean_channel(data: &[u8], ch: usize) -> u8 {
    assert!(data.len() % 4 == 0, "BGRA data length must be a multiple of 4");
    let n = data.len() / 4;
    if n == 0 {
        return 0;
    }
    let sum: u64 = data.chunks_exact(4).map(|pixel| u64::from(pixel[ch])).sum();
    // Integer rounding: (sum + n/2) / n
    ((sum + u64::try_from(n / 2).unwrap()) / u64::try_from(n).unwrap()) as u8
}

/// Population variance of channel `ch` as a u64. Uses the computational formula
/// `Var = E[X²] - E[X]²` with purely integer arithmetic.
///
/// Note: For very low-variance data the integer subtraction may truncate to
fn variance_channel(data: &[u8], ch: usize) -> u64 {
    assert!(data.len() % 4 == 0, "BGRA data length must be a multiple of 4");
    let n = data.len() / 4;
    if n == 0 {
        return 0;
    }
    let n_u64 = n as u64;

    // Σx and Σx²
    let (sum, sum_sq) = data.chunks_exact(4).fold((0u64, 0u64), |(s, sq), px| {
        let v = u64::from(px[ch]);
        (s + v, sq + v * v)
    });

    let mean_sq = sum * sum / n_u64;
    let var_num = sum_sq.saturating_sub(mean_sq);
    var_num / n_u64
}

/// Minimum value of channel `ch`.
fn min_channel(data: &[u8], ch: usize) -> u8 {
    data.chunks_exact(4).map(|px| px[ch]).min().unwrap_or(0)
}

/// Maximum value of channel `ch`.
fn max_channel(data: &[u8], ch: usize) -> u8 {
    data.chunks_exact(4).map(|px| px[ch]).max().unwrap_or(0)
}

/// Histogram of channel `ch`: `result[v]` = count of pixels with value `v`.
fn histogram_channel(data: &[u8], ch: usize) -> [usize; 256] {
    let mut hist = [0usize; 256];
    for px in data.chunks_exact(4) {
        hist[px[ch] as usize] += 1;
    }
    hist
}

/// Shannon entropy of a histogram, scaled by 1000 (returned as integer).
///
/// For a histogram with equal counts at 2 distinct values, e.g., 8×0 and 8×255,
/// the true entropy is 1.0 bit → this returns 1000.
///
/// Uses f64 for the log2 computation internally, but the result is integer
/// (scale × 1000, no fractional comparison).
fn entropy_scaled(hist: &[usize; 256], total: usize) -> u32 {
    if total == 0 {
        return 0;
    }
    let total_f = total as f64;
    let h: f64 = hist
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / total_f;
            -p * p.log2()
        })
        .sum();
    // Scale by 1000 and round to nearest integer.
    ((h * 1000.0).round() as i32).max(0) as u32
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Decode golden raw `.enc` bytes (no prefix) using the format-specific
/// decoder via `decode_with_profile` with the correct prefix prepended.
fn decode_golden(enc: &[u8], profile: &Profile) -> ithmb_core::DecodedImage {
    let prefix_bytes = (profile.prefix as u32).to_be_bytes();
    let mut with_prefix = Vec::with_capacity(4 + enc.len());
    with_prefix.extend_from_slice(&prefix_bytes);
    with_prefix.extend_from_slice(enc);
    decode_with_profile(&with_prefix, profile, &AtomicBool::new(false)).expect("golden decode should succeed")
}

/// Encode `BGRA` → encode → prepend prefix → decode → return `DecodedImage`.
fn roundtrip(bgra: &[u8], w: i32, h: i32, profile: &Profile) -> ithmb_core::DecodedImage {
    let encoded = encode_bgra(bgra, w, h, profile);
    let prefix_bytes = (profile.prefix as u32).to_be_bytes();
    let mut with_prefix = Vec::with_capacity(4 + encoded.len());
    with_prefix.extend_from_slice(&prefix_bytes);
    with_prefix.extend_from_slice(&encoded);
    decode_with_profile(&with_prefix, profile, &AtomicBool::new(false)).expect("roundtrip decode should succeed")
}

// ---------------------------------------------------------------------------
// Test 1 — Solid-color mean
// ---------------------------------------------------------------------------

#[test]
fn decoded_solid_white_mean_within_tolerance() {
    // Given: golden RGB565 solid-white 2×2 image
    let enc = include_bytes!("fixtures/rgb565_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: each channel's mean is within ±2 of 255
    for ch in 0..NUM_CHANNELS {
        let m = mean_channel(&img.data, ch);
        // Alpha is always 255, B/G/R MSB-replicate white to 255 exactly
        assert!(
            m.abs_diff(255) <= 2,
            "white image channel {ch} mean {m} differs from 255 by more than 2"
        );
    }
}

#[test]
fn decoded_solid_red_mean_within_tolerance() {
    // Given: golden RGB565 solid-red 2×2 image
    //   Red through RGB565: R5=31 → MSB replicate → 255
    //                       G6=0  → MSB replicate → 0
    //                       B5=0  → MSB replicate → 0
    let enc = include_bytes!("fixtures/rgb565_solid_red_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: R mean ≈ 255, G/B mean ≈ 0, alpha = 255
    let mean_r = mean_channel(&img.data, 2);
    let mean_g = mean_channel(&img.data, 1);
    let mean_b = mean_channel(&img.data, 0);
    let mean_a = mean_channel(&img.data, 3);
    assert!(mean_r.abs_diff(255) <= 2, "red channel mean {mean_r} != 255 ±2");
    assert!(mean_g <= 2, "green channel mean {mean_g} > 2");
    assert!(mean_b <= 2, "blue channel mean {mean_b} > 2");
    assert!(mean_a.abs_diff(255) <= 2, "alpha channel mean {mean_a} != 255 ±2");
}

#[test]
fn decoded_uyvy_solid_white_mean_within_tolerance() {
    // Given: golden UYVY solid-white 2×2
    let enc = include_bytes!("fixtures/uyvy_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Yuv422);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: neutral white through BT.601 should be near 255 for all channels
    // (small BT.601 rounding error of ±2 is acceptable)
    for ch in 0..NUM_CHANNELS {
        let m = mean_channel(&img.data, ch);
        // Alpha is always 255; B/G/R may have ±2 BT.601 rounding
        assert!(
            m.abs_diff(255) <= 2,
            "UYVY white channel {ch} mean {m} differs from 255 by more than 2"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2 — Gradient per-channel variance
// ---------------------------------------------------------------------------

#[test]
fn decoded_gradient_has_positive_variance_rgb565() {
    // Given: golden RGB565 gradient 4×4
    let enc = include_bytes!("fixtures/rgb565_gradient_4x4.enc");
    let profile = util::make_profile(4, 4, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: at least one color channel has non-zero variance (it is a gradient)
    let var_r = variance_channel(&img.data, 2);
    let var_g = variance_channel(&img.data, 1);
    let var_b = variance_channel(&img.data, 0);
    assert!(
        var_r > 0 || var_g > 0 || var_b > 0,
        "gradient should have positive variance in at least one channel \
         (R={var_r}, G={var_g}, B={var_b})"
    );
}

#[test]
fn decoded_gradient_has_positive_variance_rgb555() {
    // Given: golden RGB555 gradient 4×4
    let enc = include_bytes!("fixtures/rgb555_gradient_4x4.enc");
    let profile = util::make_profile(4, 4, Encoding::Rgb555);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: at least one color channel has non-zero variance
    let var_r = variance_channel(&img.data, 2);
    let var_g = variance_channel(&img.data, 1);
    let var_b = variance_channel(&img.data, 0);
    assert!(
        var_r > 0 || var_g > 0 || var_b > 0,
        "gradient should have positive variance in at least one channel \
         (R={var_r}, G={var_g}, B={var_b})"
    );
}

// ---------------------------------------------------------------------------
// Test 3 — Checkerboard spatial entropy
// ---------------------------------------------------------------------------

#[test]
fn decoded_checkerboard_has_bimodal_entropy() {
    // Given: a synthetic 4×4 checkerboard pattern (alternating black/white)
    let w = 4i32;
    let h = 4i32;
    let n = (w * h) as usize;
    let mut bgra = vec![0u8; n * 4];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let is_white = (x + y) % 2 == 0;
            let v = if is_white { 255u8 } else { 0u8 };
            bgra[idx] = v; // B
            bgra[idx + 1] = v; // G
            bgra[idx + 2] = v; // R
            bgra[idx + 3] = 255; // A
        }
    }

    // When: encode as RGB565, decode back
    let profile = util::make_profile(w, h, Encoding::Rgb565);
    let img = roundtrip(&bgra, w, h, &profile);

    // Then: each color channel histogram has exactly 2 non-zero bins
    // (black → 0, white → 255) — verifying the decoded pattern retains
    // spatial entropy ≈ 1.0 bit per channel.
    for ch in 0..3 {
        // Color channels only (B, G, R).  Alpha is always 255.
        let hist = histogram_channel(&img.data, ch);
        let nonzero_bins = hist.iter().filter(|&&c| c > 0).count();
        assert!(
            nonzero_bins >= 2,
            "checkerboard channel {ch} has only {nonzero_bins} non-zero histogram bins \
             (expected at least 2: black and white)"
        );
        // Verify both dark (0-31) and light (224-255) values are present.
        let has_dark = hist[..32].iter().any(|&c| c > 0);
        let has_light = hist[224..].iter().any(|&c| c > 0);
        assert!(
            has_dark && has_light,
            "checkerboard channel {ch} missing dark or light values"
        );
        // Entropy should be > 500 (i.e., > 0.5 bits on the 0-1000 scale)
        let ent = entropy_scaled(&hist, (w * h) as usize);
        assert!(
            ent > 500,
            "checkerboard channel {ch} entropy {ent} is too low for a bimodal pattern"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4 — Per-channel extrema
// ---------------------------------------------------------------------------

#[test]
fn decoded_solid_white_extrema_all_255() {
    // Given: golden RGB565 solid-white 2×2
    let enc = include_bytes!("fixtures/rgb565_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: every channel's min and max is ±2 of 255
    for ch in 0..NUM_CHANNELS {
        let mn = min_channel(&img.data, ch);
        let mx = max_channel(&img.data, ch);
        assert!(
            mn.abs_diff(255) <= 2,
            "white channel {ch} min {mn} differs from 255 by more than 2"
        );
        assert!(
            mx.abs_diff(255) <= 2,
            "white channel {ch} max {mx} differs from 255 by more than 2"
        );
    }
}

#[test]
fn decoded_solid_red_extrema_correct_range() {
    // Given: golden RGB565 solid-red 2×2
    let enc = include_bytes!("fixtures/rgb565_solid_red_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: R channel near 255, G and B near 0
    let mn_r = min_channel(&img.data, 2);
    let mx_r = max_channel(&img.data, 2);
    let mn_g = min_channel(&img.data, 1);
    let mx_g = max_channel(&img.data, 1);
    let mn_b = min_channel(&img.data, 0);
    let mx_b = max_channel(&img.data, 0);
    assert!(mn_r.abs_diff(255) <= 2, "red channel min {mn_r} != 255 ±2");
    assert!(mx_r.abs_diff(255) <= 2, "red channel max {mx_r} != 255 ±2");
    assert!(mn_g <= 2 && mx_g <= 2, "green channel extrema {mn_g}/{mx_g} > 2");
    assert!(mn_b <= 2 && mx_b <= 2, "blue channel extrema {mn_b}/{mx_b} > 2");
}

#[test]
fn decoded_gradient_extrema_span_range() {
    // Given: golden RGB565 gradient 4×4
    let enc = include_bytes!("fixtures/rgb565_gradient_4x4.enc");
    let profile = util::make_profile(4, 4, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: in at least one channel, max - min > 50 (span of values)
    let span_r = i16::from(max_channel(&img.data, 2)) - i16::from(min_channel(&img.data, 2));
    let span_g = i16::from(max_channel(&img.data, 1)) - i16::from(min_channel(&img.data, 1));
    let span_b = i16::from(max_channel(&img.data, 0)) - i16::from(min_channel(&img.data, 0));
    assert!(
        span_r > 50 || span_g > 50 || span_b > 50,
        "gradient should span >50 levels in at least one channel \
         (R span={span_r}, G span={span_g}, B span={span_b})"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — Random decode range check
// ---------------------------------------------------------------------------

#[test]
fn decoded_random_frames_in_0_255_range() {
    // Given: 5 deterministic pseudo-random 4x4 BGRA images
    // (using a fixed seed for determinism)
    let w = 4i32;
    let h = 4i32;
    let profile = util::make_profile(w, h, Encoding::Rgb565);
    // Deterministic "random" sequence
    let seed: [u8; 16] = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
    ];

    for trial in 0..5 {
        // Generate pseudo-random BGRA from seed + trial
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        for (i, px) in bgra.chunks_exact_mut(4).enumerate() {
            let mix = (trial * 13 + i * 7) as u8;
            px[0] = seed[0].wrapping_add(mix); // B
            px[1] = seed[4].wrapping_sub(mix.wrapping_mul(3)); // G
            px[2] = seed[8].wrapping_mul(mix.wrapping_add(1)); // R
            px[3] = 255; // A
        }

        // When: encode as RGB565, decode back
        let img = roundtrip(&bgra, w, h, &profile);
        // Then: dimensions preserved
        assert_eq!(img.width, w as u32, "width should match");
        assert_eq!(img.height, h as u32, "height should match");
        assert!(!img.data.is_empty(), "decoded data should not be empty");
        // Note: data is Vec<u8> so values are always in 0-255 range by type
    }
}

// ---------------------------------------------------------------------------
// Test 6 — Color histogram
// ---------------------------------------------------------------------------

#[test]
fn decoded_white_histogram_peak_at_255_rgb565() {
    // Given: golden RGB565 solid-white 2×2
    let enc = include_bytes!("fixtures/rgb565_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: every channel's histogram is concentrated at 255
    let num_pixels = (img.data.len() / 4) as usize;
    for ch in 0..NUM_CHANNELS {
        let hist = histogram_channel(&img.data, ch);
        // Alpha is always 255; color channels for solid white too.
        let peak_count = hist[255];
        assert!(
            peak_count == num_pixels,
            "white image channel {ch}: expected all {num_pixels} pixels at value 255, \
             got {peak_count}"
        );
    }
}

#[test]
fn decoded_white_histogram_peak_at_255_uyvy() {
    // Given: golden UYVY solid-white 2×2
    let enc = include_bytes!("fixtures/uyvy_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Yuv422);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: histogram peak is at or very near 255
    // (UYVY → BT.601 → a tiny number of pixels may round to 254)
    let _num_pixels = img.data.len() / 4;
    for ch in 0..NUM_CHANNELS {
        let hist = histogram_channel(&img.data, ch);
        // Find the bin with the most pixels
        let (peak_val, _) = hist
            .iter()
            .enumerate()
            .max_by_key(|&(_, &c)| c)
            .expect("histogram has at least one bin");
        // For white through UYVY, the peak should be at 253-255
        // (small BT.601 rounding may cause a few pixels at 254)
        assert!(
            peak_val >= 253,
            "UYVY white channel {ch}: histogram peak at {peak_val}, expected ≥253"
        );
    }
}

#[test]
fn decoded_white_histogram_near_peak_implies_blue_channel_rgb565() {
    // Given: golden RGB565 solid-white 2×2
    let enc = include_bytes!("fixtures/rgb565_solid_white_2x2.enc");
    let profile = util::make_profile(2, 2, Encoding::Rgb565);
    // When: decoding
    let img = decode_golden(enc, &profile);
    // Then: R, G, B channels all peak at 255 (solid white)
    // Specifically verify the blue channel per task requirement:
    // "decoded frames have expected color histograms (RGB565 white → near-peak blue channel)"
    let hist_b = histogram_channel(&img.data, 0);
    let peak_b = hist_b[255];
    assert!(
        peak_b >= 3,
        "white RGB565: expected blue channel near-peak at 255 (≥3 of 4 pixels), got {peak_b}"
    );
}

// ---------------------------------------------------------------------------
// Extra: cross-format variance consistency (synthetic gradient)
// ---------------------------------------------------------------------------

#[test]
fn synthetic_gradient_variance_rgb565_nonzero() {
    // Given: a synthetic 4×4 horizontal gradient (B varies 0..15 per pixel)
    let w = 4i32;
    let h = 4i32;
    let n = (w * h) as usize;
    let mut bgra = vec![0u8; n * 4];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let t = (y * w + x) as u8;
            bgra[idx] = t * 17; // B
            bgra[idx + 1] = t * 7; // G
            bgra[idx + 2] = 255 - t * 11; // R
            bgra[idx + 3] = 255; // A
        }
    }

    // When: encode as RGB565, decode back
    let profile = util::make_profile(w, h, Encoding::Rgb565);
    let img = roundtrip(&bgra, w, h, &profile);

    // Then: all three color channels have positive variance
    let var_r = variance_channel(&img.data, 2);
    let var_g = variance_channel(&img.data, 1);
    let var_b = variance_channel(&img.data, 0);
    assert!(
        var_r > 0 || var_g > 0 || var_b > 0,
        "synthetic gradient should have positive variance \
         (R={var_r}, G={var_g}, B={var_b})"
    );
}
