//! PhotoDB/ArtworkDB — Apple's iPod/iPhone thumbnail database format.
//!
//! Binary chunk-based format (`MHFD`→`MHSD`→`MHNI` entries) supporting
//! read, write, and integrity checking of artwork databases.

/// Builder for constructing synthetic PhotoDB/ArtworkDB binaries.
pub mod builder;

/// Parser for reading PhotoDB/ArtworkDB binary data.
pub mod parser;

/// Shared type definitions (header structs, constants, helpers).
pub mod types;
