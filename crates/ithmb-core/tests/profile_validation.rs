//! Validation tests for all 54 built-in profiles.
//!
//! Loads every profile from the built-in database and checks each against eight
//! consistency constraints to catch data-entry errors, encoding mismatches, and
//! impossible dimensions early.
//!
//! Checks performed:
//!   1. Primary encoding is not `Jpeg` (Jpeg is valid only as a fallback)
//!   2. Width and height are in `(0, 4096]` — plausible thumbnail bounds
//!   3. `frame_byte_length` is in `(0, 256 MiB]`
//!   4. No two profiles share the same `prefix` value
//!   5. `cl_chroma` or `clcl_chroma` ⇒ encoding is `Yuv422`
//!   6. `is_interlaced` ⇒ encoding is `Yuv422`
//!   7. `swap_chroma_planes` ⇒ encoding is `Ycbcr420`
//!   8. `frame_byte_length` matches the formula for the given encoding/dimensions
#![allow(clippy::pedantic, clippy::unwrap_used)]
//!      (with tolerance for `is_padded` profiles — they may exceed the minimum)

use std::collections::HashSet;

use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::profile_db::ProfileDb;
use ithmb_core::profile_parser::parse_profiles_json;
// Dev-dependency lint silence (same convention as other test files in this crate).
use divan as _;
use image as _;
use jpeg_decoder as _;
#[cfg(feature = "cache")]
use lru as _;
use thiserror as _;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the expected raw frame byte length for the given encoding and
/// dimensions.  Returns `None` for encodings whose size cannot be predicted
/// (e.g. JPEG).
fn expected_frame_size(w: i32, h: i32, encoding: Encoding) -> Option<i32> {
    match encoding {
        Encoding::Rgb565 | Encoding::Rgb555 | Encoding::ReorderedRgb555 | Encoding::Yuv422 => Some(w * h * 2),
        Encoding::Ycbcr420 => {
            // Y plane: w*h bytes
            // Cb plane: ceil(w/2) * ceil(h/2) bytes
            // Cr plane: ceil(w/2) * ceil(h/2) bytes
            let y_plane = w * h;
            let chroma_w = (w + 1) / 2; // ceiling division
            let chroma_h = (h + 1) / 2;
            Some(y_plane + chroma_w * chroma_h * 2)
        }
        _ => None, // Jpeg size depends on compression ratio, falling back to _ coverage
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn validate_all_builtin_profiles() {
    // Load via `parse_profiles_json` first — this gives us the raw Vec *before*
    // HashMap dedup so we can detect duplicate prefix values.
    let json = include_str!("../data/profiles.json");
    let raw_profiles = parse_profiles_json(json).expect("built-in profiles.json must parse");

    // Also load through the normal DB path to exercise that code path as well.
    let db = ProfileDb::load_builtin().expect("ProfileDb::load_builtin must succeed");
    let db_profiles: Vec<&Profile> = db.all().values().collect();

    let mut errors: Vec<String> = Vec::new();

    // ---- Check 4: No duplicate Prefix values ----
    // (Run against the raw, pre-HashMap list to catch silent overwrites.)
    let mut seen_prefixes: HashSet<i32> = HashSet::new();
    for p in &raw_profiles {
        if !seen_prefixes.insert(p.prefix) {
            errors.push(format!("Profile prefix {} is duplicated in profiles.json", p.prefix));
        }
    }

    // ---- Per-profile checks against the DB (canonical access path) ----
    for p in &db_profiles {
        let prefix = p.prefix;

        // -- Check 1: Primary encoding must not be Jpeg --
        if p.encoding == Encoding::Jpeg {
            errors.push(format!(
                "Profile {prefix}: primary encoding is Jpeg (only valid as a fallback)",
            ));
        }

        // -- Check 2: 0 < width/height ≤ 4096 --
        if p.width <= 0 {
            errors.push(format!("Profile {}: width ({}) must be positive", prefix, p.width,));
        } else if p.width > 4096 {
            errors.push(format!("Profile {}: width ({}) exceeds maximum 4096", prefix, p.width,));
        }

        if p.height <= 0 {
            errors.push(format!("Profile {}: height ({}) must be positive", prefix, p.height,));
        } else if p.height > 4096 {
            errors.push(format!(
                "Profile {}: height ({}) exceeds maximum 4096",
                prefix, p.height,
            ));
        }

        // -- Check 3: 0 < frame_byte_length ≤ 256 MiB --
        if p.frame_byte_length <= 0 {
            errors.push(format!(
                "Profile {}: frame_byte_length ({}) must be positive",
                prefix, p.frame_byte_length,
            ));
        } else if p.frame_byte_length > 256 * 1024 * 1024 {
            errors.push(format!(
                "Profile {}: frame_byte_length ({}) exceeds 256 MiB",
                prefix, p.frame_byte_length,
            ));
        }

        // -- Check 5: cl_chroma / clcl_chroma ⇒ encoding is Yuv422 --
        if p.cl_chroma && p.encoding != Encoding::Yuv422 {
            errors.push(format!(
                "Profile {}: cl_chroma is true but encoding is {:?}, expected Yuv422",
                prefix, p.encoding,
            ));
        }
        if p.clcl_chroma && p.encoding != Encoding::Yuv422 {
            errors.push(format!(
                "Profile {}: clcl_chroma is true but encoding is {:?}, expected Yuv422",
                prefix, p.encoding,
            ));
        }

        // -- Check 6: is_interlaced ⇒ encoding is Yuv422 --
        if p.is_interlaced && p.encoding != Encoding::Yuv422 {
            errors.push(format!(
                "Profile {}: is_interlaced is true but encoding is {:?}, expected Yuv422",
                prefix, p.encoding,
            ));
        }

        // -- Check 7: swap_chroma_planes ⇒ encoding is Ycbcr420 --
        if p.swap_chroma_planes && p.encoding != Encoding::Ycbcr420 {
            errors.push(format!(
                "Profile {}: swap_chroma_planes is true but encoding is {:?}, expected Ycbcr420",
                prefix, p.encoding,
            ));
        }

        // -- Check 8: frame_byte_length consistent with encoding & dimensions --
        // Profiles with use_mhni_dimensions have variable dimensions determined at
        // decode time from the MHNI chunk; stored width/height are just defaults
        // whose pixel count does not necessarily match frame_byte_length.
        if !p.use_mhni_dimensions {
            if let Some(expected) = expected_frame_size(p.width, p.height, p.encoding) {
                if p.frame_byte_length != expected {
                    if p.is_padded {
                        // Padded profiles may have frame_byte_length >= expected
                        // (actual pixel data + padding bytes).
                        if p.frame_byte_length < expected {
                            errors.push(format!(
                                "Profile {} (padded): frame_byte_length ({}) is smaller \
                                 than the minimum expected ({}) for {:?} {}x{}",
                                prefix, p.frame_byte_length, expected, p.encoding, p.width, p.height,
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "Profile {}: frame_byte_length ({}) does not match \
                             expected ({}) for {:?} {}x{}",
                            prefix, p.frame_byte_length, expected, p.encoding, p.width, p.height,
                        ));
                    }
                }
            }
        }
    }

    // ---- All violations collected — assert clean ----
    assert!(
        errors.is_empty(),
        "Profile validation failed ({} violation(s)):\n  {}",
        errors.len(),
        errors.join("\n  "),
    );
}
