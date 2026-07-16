//! # ithmb-core
//!
//! Pure Rust decoder and encoder for Apple `.ithmb` thumbnail-cache files — the format
//! used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo
//! and album art thumbnails.
//!
//! This is the canonical reference implementation. All decoder logic lives here;
//! language-specific wrappers (C FFI, WASM, Python bindings) call into this core.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use ithmb_core::decode_ithmb;
//! use std::sync::atomic::AtomicBool;
//!
//! let data = std::fs::read("photo.ithmb").expect("read file");
//! let canceled = AtomicBool::new(false);
//! match decode_ithmb(&data, &canceled) {
//!     Ok(img) => println!("Decoded {}x{} image", img.width, img.height),
//!     Err(e) => eprintln!("Decode failed: {e}"),
//! }
//! ```
//!
//! # Feature flags
//!
//! | Feature | Description | Default |
//! |---|---|---|
//! | `cache` | LRU raw file cache | no |
//! | `metrics` | Decode timing counters | no |
//! | `c` | C ABI shared library (`#![crate_type = "cdylib"]`) | no |
//!
//! Enable features in `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! ithmb-core = { version = "1.9", features = ["cache"] }
//! ```
//!
//! # Architecture
//!
//! The library follows a pipelined decode flow:
//!
//! 1. **Peek prefix** — read the 4-byte big-endian format prefix.
//! 2. **JPEG scan** — check for JPEG SOI marker (`FF D8`).
//! 3. **Profile lookup** — match prefix against 54 built-in profiles.
//! 4. **Decode** — dispatch to the correct pixel or JPEG decoder.
//! 5. **Post-process** — apply dimension swap, crop, and rotation.
//! 6. **Output** — [`DecodedImage`] with BGRA8 pixel data.
//!
//! For files starting with `mhfd` (PhotoDB/ArtworkDB containers), the chunk tree
//! is parsed by [`photodb`] and individual thumbnails are extracted and decoded.

// When compiled with `--cfg test` (bench/test targets), divan is available as a
// dev-dependency but unused by the library itself — suppress the lint.
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(test, allow(unused_crate_dependencies))]

/// C ABI shared library for ImageGlass native plugin integration.
#[cfg_attr(docsrs, doc(cfg(feature = "c")))]
#[cfg(feature = "c")]
pub mod c_api;
/// Runtime configuration for decode parameters.
pub mod config;

/// LRU cache for repeatedly accessed raw `.ithmb` files.
#[cfg_attr(docsrs, doc(cfg(feature = "cache")))]
#[cfg(feature = "cache")]
pub mod cache;

/// Decode timing counters and performance metrics.
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
#[cfg(feature = "metrics")]
pub mod metrics;

/// CL (per-pixel chroma) YUV decoder.
pub mod cl;
/// CLCL (shared-nibble chroma) YUV decoder.
pub mod clcl;
/// Dimension validation and cancellation-check helpers.
pub mod decoder_helpers;
/// Device-to-format-ID lookup table for 18 iPod/iPhone models.
pub mod device_profiles;
/// Synthetic encoders for all raw pixel formats.
pub mod enc;
/// Error types ([`DecodeError`]) and decoded image container.
pub mod error;
/// JPEG-embedded stream decoder with EXIF orientation support.
pub mod jpeg;
/// PhotoDB/ArtworkDB chunk parser, writer, and integrity checker.
pub mod photodb;
/// BGRA pixel utility functions (MSB replication, clamping, cancellation).
pub mod pixel_utils;
/// Profile type ([`crate::profile::Profile`]) and built-in profile lookup.
pub mod profile;
/// Profile database — embedded JSON + external `profiles.json` support.
pub mod profile_db;
/// JSON parser for external `profiles.json` override files.
pub mod profile_parser;
/// Reordered (byte-swapped) RGB555 decoder.
pub mod reordered_rgb555;
/// RGB555 decoder (15-bit, 5 bits per channel).
pub mod rgb555;
/// RGB565 decoder (16-bit, 5/6/5 layout) — most common raw format.
pub mod rgb565;
/// SIMD-accelerated pixel conversions (SSE2/AVX2/NEON with runtime dispatch, scalar fallback).
pub mod simd;
/// UYVY 4:2:2 decoder (linear and interlaced variants).
pub mod uyvy;
/// YCbCr 4:2:0 planar decoder.
pub mod ycbcr420;
/// Shared YUV conversion helpers and color-space arithmetic.
pub mod yuv;

/// Re-export of [`DecodeError`] and [`DecodedImage`] for convenience.
pub use error::{DecodeError, DecodedImage};
/// Central decode dispatch — the primary entry point for `.ithmb` decoding.
pub mod pipeline;

/// Re-export of [`decode_ithmb`], [`open_ithmb`], [`encoding_name_for_prefix`],
/// and the `_with_config` variants for convenience.
pub use pipeline::{
    decode_ithmb, decode_ithmb_with_config, decode_with_profile, decode_with_profile_with_config,
    encoding_name_for_prefix, open_ithmb, open_ithmb_with_config,
};

/// Re-export of [`DeviceProfile`] for convenience.
pub use crate::device_profiles::DeviceProfile;
/// Re-export of [`Encoding`] and [`Profile`] for convenience.
pub use crate::profile::{Encoding, Profile};
/// Re-export of [`ProfileDb`] for convenience.
pub use crate::profile_db::ProfileDb;

/// Re-export of [`DecodeConfig`] for convenience.
pub use config::DecodeConfig;
