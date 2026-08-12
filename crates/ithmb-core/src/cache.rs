//! LRU cache for decoded `.ithmb` file data.
//!
//! Wraps [`lru::LruCache`] behind a [`std::sync::RwLock`], keyed by a `SipHash` of the raw input
//! bytes. Cache hit avoids re-decoding; miss delegates to [`crate::pipeline::decode_with_profile`]
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Encoding;
    use std::sync::atomic::AtomicBool;

    /// Build a valid 1×1 RGB565 frame payload (`prefix` + 2 pixel bytes) whose
    /// little-endian pixel value is `pixel`. Distinct `pixel` values yield
    /// distinct cache keys.
    fn rgb565_payload(pixel: u8) -> (Profile, Vec<u8>) {
        let profile = Profile {
            prefix: 0x1000_0001,
            width: 1,
            height: 1,
            encoding: Encoding::Rgb565,
            frame_byte_length: 2,
            little_endian: true,
            ..Default::default()
        };
        let mut data = profile.prefix.to_be_bytes().to_vec();
        data.extend_from_slice(&[pixel, pixel]);
        (profile, data)
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn decode_with_cache_miss_then_hit_returns_identical_image() {
        let decoder = CachedDecoder::new();
        let canceled = AtomicBool::new(false);
        let (profile, data) = rgb565_payload(0xFF);

        // Miss: decodes through the pipeline and stores the result.
        let first = decoder
            .decode_with_cache(&profile, &data, &canceled)
            .expect("first decode should succeed");
        assert_eq!(first.width, 1);
        assert_eq!(first.height, 1);
        assert_eq!(first.data, vec![255, 255, 255, 255], "0xFFFF RGB565 is white");
        assert_eq!(decoder.len(), 1);
        assert!(!decoder.is_empty());

        // Hit: same input must return the identical image without re-decoding.
        let second = decoder
            .decode_with_cache(&profile, &data, &canceled)
            .expect("cache hit should succeed");
        assert_eq!(second, first, "cache hit must return the identical image");
        assert_eq!(decoder.len(), 1, "a hit must not add a second entry");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn distinct_inputs_are_distinct_entries() {
        let decoder = CachedDecoder::new();
        let canceled = AtomicBool::new(false);
        let (profile_a, data_a) = rgb565_payload(0x11);
        let (profile_b, data_b) = rgb565_payload(0x22);
        decoder
            .decode_with_cache(&profile_a, &data_a, &canceled)
            .expect("decode a");
        decoder
            .decode_with_cache(&profile_b, &data_b, &canceled)
            .expect("decode b");
        assert_eq!(decoder.len(), 2);
        assert!(!decoder.is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn clear_evicts_all_entries_and_cache_stays_usable() {
        let decoder = CachedDecoder::new();
        let canceled = AtomicBool::new(false);
        let (profile, data) = rgb565_payload(0x00);
        let image = decoder.decode_with_cache(&profile, &data, &canceled).expect("decode");
        assert_eq!(decoder.len(), 1);

        decoder.clear();
        assert_eq!(decoder.len(), 0);
        assert!(decoder.is_empty());

        // A subsequent decode must still work after clear().
        let again = decoder
            .decode_with_cache(&profile, &data, &canceled)
            .expect("decode after clear");
        assert_eq!(again, image);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn lru_evicts_down_to_capacity() {
        let decoder = CachedDecoder::new();
        let canceled = AtomicBool::new(false);
        // Capacity is 64; insert 65 distinct frames so exactly one is evicted.
        for pixel in 0_u8..65 {
            let (profile, data) = rgb565_payload(pixel);
            decoder.decode_with_cache(&profile, &data, &canceled).expect("decode");
        }
        assert_eq!(decoder.len(), 64, "LRU must evict down to capacity");
    }

    #[test]
    fn decode_with_cache_propagates_errors_without_caching() {
        let decoder = CachedDecoder::new();
        let canceled = AtomicBool::new(false);
        let result = decoder.decode_with_cache(&Profile::default(), b"", &canceled);
        assert!(matches!(
            result,
            Err(DecodeError::BufferTooShort { .. } | DecodeError::InvalidFormat(..))
        ));
        assert_eq!(decoder.len(), 0, "failed decodes must not be cached");
        assert!(decoder.is_empty());
    }

    #[test]
    fn entry_serialization_roundtrips_identity() {
        let img = DecodedImage {
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
            width: 2,
            height: 1,
        };
        assert_eq!(deserialize_entry(&serialize_entry(&img)), img);
    }

    #[test]
    fn entry_serialization_roundtrips_empty_and_max_dimensions() {
        let empty = DecodedImage {
            data: vec![],
            width: 0,
            height: 0,
        };
        assert_eq!(deserialize_entry(&serialize_entry(&empty)), empty);
        let max_dims = DecodedImage {
            data: vec![0u8; 4],
            width: u32::MAX,
            height: u32::MAX,
        };
        assert_eq!(deserialize_entry(&serialize_entry(&max_dims)), max_dims);
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn deserialize_entry_panics_on_short_input() {
        deserialize_entry(&[0u8; 7]);
    }
}
