//! # ithmb-core
//!
//! Pure Rust decoder for Apple `.ithmb` thumbnail cache files — the format used
//! by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo
//! and album art thumbnails.
//!
//! This is the canonical reference implementation. All decoder logic lives here;
//! language-specific wrappers (C# FFI, WASM, Python bindings) call into this core.
pub mod cl;
pub mod clcl;
pub mod error;
pub mod jpeg;
pub mod profile;
pub mod profile_db;
pub mod profile_parser;
pub mod reordered_rgb555;
pub mod rgb555;
pub mod rgb565;
pub mod uyvy;
pub mod yuv;

pub mod ycbcr420;
pub use error::{DecodeError, DecodedImage};
pub mod pipeline;
pub use pipeline::decode_ithmb;
