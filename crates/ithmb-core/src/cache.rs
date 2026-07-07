//! LRU cache for decoded `.ithmb` file data.
//!
//! Wraps [`LruCache`] behind a [`RwLock`], keyed by a `SipHash` of the raw input
//! bytes. Cache hit avoids re-decoding; miss delegates to [`decode_with_profile`]
//! and stores the result.
//!
//! The cache stores the full [`DecodedImage`] (pixel data + dimensions) encoded
//! as a flat `Vec<u8>`: 4&nbsp;bytes little-endian width, 4&nbsp;bytes
//! little-endian height, then the BGRA pixel data.
//!
//! # Feature gate
//!
//! ```toml
//! [features]
//! cache = []
//! ```
//!
//! Requires `lru = "0.13"` (behind the `cache` feature).
//!
//! # Example
//!
//! ```rust
//! # use ithmb_core::cache::CachedDecoder;
//! # use std::sync::atomic::AtomicBool;
//! let decoder = CachedDecoder::new();
//! let canceled = AtomicBool::new(false);
//! // decoder.decode_with_cache(&profile, data, &canceled);
//! ```

use crate::error::{DecodeError, DecodedImage};
use crate::pipeline::decode_with_profile;
use crate::profile::Profile;
use lru::LruCache;
use std::hash::Hasher;
use std::num::NonZeroUsize;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;

/// Least-recently-used decode cache for raw `.ithmb` frame data.
///
/// Cache entries are keyed by a `SipHash` of the raw input bytes (content
/// addressable, not path-based). On a cache hit the pre-decoded pixel data
/// is returned without re-decoding; on a miss [`decode_with_profile`] is
/// called and the result is stored.
///
/// # Example
///
/// ```rust
/// # use ithmb_core::cache::CachedDecoder;
/// # use std::sync::atomic::AtomicBool;
/// let decoder = CachedDecoder::new();
/// let canceled = AtomicBool::new(false);
/// // let image = decoder.decode_with_cache(&profile, data, &canceled)
/// //     .expect("decode failed");
/// ```
#[derive(Debug)]
pub struct CachedDecoder {
    cache: RwLock<LruCache<u64, Vec<u8>>>,
}

impl CachedDecoder {
    /// Create a new `CachedDecoder` with a capacity of 64 entries.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned (another thread panicked
    /// while holding the lock).
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(LruCache::new(NonZeroUsize::new(64).expect("64 is non-zero"))),
        }
    }

    /// Decode `data` using `profile`, consulting the LRU cache first.
    ///
    /// On a cache hit the pre-decoded result is returned immediately.
    /// On a cache miss [`decode_with_profile`] is called, the result is
    /// inserted into the cache, and then returned.
    ///
    /// Cache entries are keyed by a `SipHash` of `data` (the raw input bytes),
    /// so the same input always produces the same key.
    ///
    /// # Note on dimensions
    ///
    /// The cached entry stores the full [`DecodedImage`] including
    /// post-processed dimensions (width/height after rotation and crop).
    /// A cache hit returns the exact same dimensions as the original decode.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if the underlying [`decode_with_profile`] fails.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned (another thread panicked
    /// while holding the lock).
    #[allow(clippy::missing_panics_doc)]
    pub fn decode_with_cache(
        &self,
        profile: &Profile,
        data: &[u8],
        canceled: &AtomicBool,
    ) -> Result<DecodedImage, DecodeError> {
        let key = {
            // DefaultHasher is SipHash-based (SipHash-1-3), providing a high-quality
            // content-addressable key with minimal overhead.
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            hasher.write(data);
            hasher.finish()
        };

        // Cache lookup — requires write lock so LruCache can update LRU ordering.
        {
            let mut cache = self.cache.write().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(entry) = cache.get(&key) {
                return Ok(deserialize_entry(entry));
            }
        }

        // Cache miss — decode via the standard pipeline.
        let image = decode_with_profile(data, profile, canceled)?;

        // Store result.
        {
            let mut cache = self.cache.write().unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.put(key, serialize_entry(&image));
        }

        Ok(image)
    }

    /// Evict all entries from the cache.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[allow(clippy::missing_panics_doc)]
    pub fn clear(&self) {
        let mut cache = self.cache.write().expect("cache lock poisoned");
        cache.clear();
    }

    /// Return the number of entries currently in the cache.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn len(&self) -> usize {
        let cache = self.cache.read().expect("cache lock poisoned");
        cache.len()
    }

    /// Return `true` if the cache contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for CachedDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers — flat-encoding for cache entries
// ---------------------------------------------------------------------------

/// Encode a [`DecodedImage`] into a flat `Vec<u8>` for cache storage.
///
/// Wire format (little-endian):
/// ```text
/// [0..4)  width   (u32)
/// [4..8)  height  (u32)
/// [8..)   BGRA pixel data
/// ```
fn serialize_entry(img: &DecodedImage) -> Vec<u8> {
    let width_bytes = img.width.to_le_bytes();
    let height_bytes = img.height.to_le_bytes();
    let mut buf = Vec::with_capacity(8 + img.data.len());
    buf.extend_from_slice(&width_bytes);
    buf.extend_from_slice(&height_bytes);
    buf.extend_from_slice(&img.data);
    buf
}

/// Decode a [`DecodedImage`] from a flat `Vec<u8>` produced by
/// [`serialize_entry`].
///
/// # Panics
///
/// Panics if `entry` is shorter than 8 bytes (should never happen for
/// entries that were created by [`serialize_entry`]).
fn deserialize_entry(entry: &[u8]) -> DecodedImage {
    let width = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]);
    let height = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
    DecodedImage {
        data: entry[8..].to_vec(),
        width,
        height,
    }
}
