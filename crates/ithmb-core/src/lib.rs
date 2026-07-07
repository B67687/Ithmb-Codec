#![warn(missing_docs)]
//! # ithmb-core
//!
//! Pure Rust decoder for Apple `.ithmb` thumbnail cache files — the format used
//! by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo
//! and album art thumbnails.
//!
//! This is the canonical reference implementation. All decoder logic lives here;
//! language-specific wrappers (C# FFI, WASM, Python bindings) call into this core.
// When compiled with `--cfg test` (bench/test targets), divan is available as a
// dev-dependency but unused by the library itself — suppress the lint.
#![cfg_attr(test, allow(unused_crate_dependencies))]
#[cfg(feature = "cache")]
pub mod cache;
#[cfg(feature = "metrics")]
pub mod metrics;

pub mod cl;
pub mod clcl;
pub mod decoder_helpers;
pub mod device_profiles;
pub mod enc;
pub mod enc_helpers;
pub mod error;
pub mod jpeg;
pub mod photodb;
pub mod pixel_utils;
pub mod profile;
pub mod profile_db;
pub mod profile_parser;
pub mod reordered_rgb555;
pub mod rgb555;
pub mod rgb565;
pub mod simd;
pub mod uyvy;
pub mod yuv;

pub mod ycbcr420;
/// Re-export error types for convenience.
pub use error::{DecodeError, DecodedImage};
pub mod pipeline;
/// Re-export the primary decoding pipeline functions for convenience.
pub use pipeline::{decode_ithmb, open_ithmb};
