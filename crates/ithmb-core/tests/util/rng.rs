//! A single shared deterministic RNG for reproducible test and benchmark vectors.
//!
//! Uses the **splitmix64** algorithm (higher quality than LCG) to produce
//! reproducible pseudo-random byte sequences from a 64-bit seed.  All tests
//! and benchmarks that need random pixel data share this one implementation.
#![allow(clippy::pedantic, clippy::unwrap_used)]

/// Minimal deterministic splitmix64 RNG.
pub struct SeededRng(u64);

impl SeededRng {
    /// Create a new RNG from a 64-bit seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Generate the next 64-bit pseudo-random value (splitmix64 core).
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Generate the next 32-bit pseudo-random value.
    #[must_use]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() & 0xFFFF_FFFF) as u32
    }

    /// Fill a BGRA buffer (`[B, G, R, A] × N`) with random-ish bytes.
    ///
    /// Every 4th byte (the alpha channel) is set to 255 so the pixels are
    /// fully opaque.  The buffer length must be a multiple of 4.
    ///
    /// # Panics
    /// Panics if `buf.len()` is not a multiple of 4.
    pub fn fill_bgra(&mut self, buf: &mut [u8]) {
        assert!(
            buf.len() % 4 == 0,
            "fill_bgra: buffer length ({}) must be a multiple of 4",
            buf.len()
        );
        for chunk in buf.chunks_exact_mut(4) {
            // BGRA byte order.
            chunk[0] = (self.next_u32() & 0xFF) as u8; // B
            chunk[1] = (self.next_u32() & 0xFF) as u8; // G
            chunk[2] = (self.next_u32() & 0xFF) as u8; // R
            chunk[3] = 255; // A
        }
    }
}
