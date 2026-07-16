//! Reordered RGB555 -> BGRA pack - SIMD-accelerated (`x86_64`), scalar fallback.
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

// ---- RGB555 pack to BGRA (SSE2, 4 px) ----

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn rgb555_pack_to_bgra_sse2(pixels: &[[u8; 2]; 4], swap: bool) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_loadl_epi64, _mm_or_si128, _mm_packus_epi16, _mm_set1_epi8, _mm_set1_epi16,
        _mm_setzero_si128, _mm_slli_epi16, _mm_srli_epi16, _mm_storeu_si128, _mm_unpacklo_epi8, _mm_xor_si128,
    };

    // Load 4 u16 pixels (8 bytes) into lower 64 bits (native LE order).
    let v = _mm_loadl_epi64(pixels.as_ptr().cast::<__m128i>());

    let mask5 = _mm_set1_epi16(0x1F);
    let zero = _mm_setzero_si128();

    // Extract at default (non-swap) positions:
    //   Default: xRRRRRGGGGGBBBBB -> R=high 5, G=mid 5, B=low 5
    let r5_default = _mm_and_si128(_mm_srli_epi16(v, 10), mask5);
    let g5 = _mm_and_si128(_mm_srli_epi16(v, 5), mask5);
    let b5_default = _mm_and_si128(v, mask5);

    // Branchless R<->B swap for BGR15 mode via XOR-select.
    let swap_mask = if swap { _mm_set1_epi16(-1i16) } else { zero };
    let diff = _mm_xor_si128(r5_default, b5_default);
    let masked = _mm_and_si128(diff, swap_mask);
    let r5 = _mm_xor_si128(r5_default, masked);
    let b5 = _mm_xor_si128(b5_default, masked);

    // MSB replicate 5->8 bits: (v << 3) | (v >> 2)
    let r8 = _mm_or_si128(_mm_slli_epi16(r5, 3), _mm_srli_epi16(r5, 2));
    let g8 = _mm_or_si128(_mm_slli_epi16(g5, 3), _mm_srli_epi16(g5, 2));
    let b8 = _mm_or_si128(_mm_slli_epi16(b5, 3), _mm_srli_epi16(b5, 2));

    // Pack 16-bit channels to bytes (values are 0-255, truncation exact).
    let vb = _mm_packus_epi16(b8, zero);
    let vg = _mm_packus_epi16(g8, zero);
    let vr = _mm_packus_epi16(r8, zero);
    let va = _mm_set1_epi8(-1i8); // 0xFF

    // Byte-interleave to BGRA order:
    // B,R interleave -> [B0, R0, B1, R1, B2, R2, B3, R3, ...]
    let br = _mm_unpacklo_epi8(vb, vr);
    // G,A interleave -> [G0, FF, G1, FF, G2, FF, G3, FF, ...]
    let ga = _mm_unpacklo_epi8(vg, va);
    // Final interleave -> [B0,G0,R0,FF, B1,G1,R1,FF, B2,G2,R2,FF, B3,G3,R3,FF]
    let result = _mm_unpacklo_epi8(br, ga);

    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), result);
    out
}

// ---- RGB555 pack to BGRA (SSSE3, 4 px) ----
//
/// SSSE3-accelerated RGB555 pack-to-BGRA using `_mm_shuffle_epi8` (pshufb)
/// for 5-bit to 8-bit MSB replication via a 16-entry lookup table + high-bit offset.
///
/// Processes 4 packed RGB555 pixels (8-byte input → 16-byte BGRA output).
///
/// # SAFETY
///
/// Caller must ensure SSSE3 is available (`is_x86_feature_detected!("ssse3")`).
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[target_feature(enable = "ssse3")]
#[cfg(test)]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn rgb555_pack_to_bgra_ssse3(pixels: &[[u8; 2]; 4], swap: bool) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi8, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadl_epi64, _mm_packus_epi16, _mm_set1_epi8,
        _mm_set1_epi16, _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_srli_epi16, _mm_storeu_si128,
        _mm_unpacklo_epi8, _mm_xor_si128,
    };
    // Load 8 bytes (4 u16 LE pixels) into lower 64 bits.
    let v = _mm_loadl_epi64(pixels.as_ptr().cast::<__m128i>());

    let mask5 = _mm_set1_epi16(0x1F);
    let zero = _mm_setzero_si128();

    // Extract R5 (bits 14-10), G5 (bits 9-5), B5 (bits 4-0) — default positions.
    let r5_default = _mm_and_si128(_mm_srli_epi16(v, 10), mask5);
    let g5 = _mm_and_si128(_mm_srli_epi16(v, 5), mask5);
    let b5_default = _mm_and_si128(v, mask5);

    // Branchless R<->B swap for BGR15 mode.
    let swap_mask = if swap { _mm_set1_epi16(-1i16) } else { zero };
    let diff = _mm_xor_si128(r5_default, b5_default);
    let masked = _mm_and_si128(diff, swap_mask);
    let r5 = _mm_xor_si128(r5_default, masked);
    let b5 = _mm_xor_si128(b5_default, masked);

    // Pack 5-bit values to bytes (values are 0-31, truncation exact).
    let r5_u8 = _mm_packus_epi16(r5, zero);
    let g5_u8 = _mm_packus_epi16(g5, zero);
    let b5_u8 = _mm_packus_epi16(b5, zero);

    // ---- SSSE3 pshufb-based 5-bit to 8-bit MSB replication ----
    // expanded[v] = ((v << 3) | (v >> 2)) for v = 0..15
    // expanded[16 + w] = expanded[w] + 0x84  for w = 0..15
    let expand_lut = _mm_setr_epi8(0, 8, 16, 24, 33, 41, 49, 57, 66, 74, 82, 90, 99, 107, 115, 123);
    let hi_bit_mask = _mm_set1_epi8(0x10i8);
    let hi_offset = _mm_set1_epi8(-124i8); // 0x84 = 132 = expanded[16] - expanded[0]
    let nibble_mask = _mm_set1_epi8(0x0Fi8);

    // Expand R channel: low nibble -> LUT lookup, bit 4 -> hi_offset
    let r_lo = _mm_and_si128(r5_u8, nibble_mask);
    let r_hi = _mm_and_si128(r5_u8, hi_bit_mask);
    let r_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(r_hi, hi_bit_mask));
    let r8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, r_lo), r_hi_sel);

    // Expand G channel
    let g_lo = _mm_and_si128(g5_u8, nibble_mask);
    let g_hi = _mm_and_si128(g5_u8, hi_bit_mask);
    let g_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(g_hi, hi_bit_mask));
    let g8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, g_lo), g_hi_sel);

    // Expand B channel
    let b_lo = _mm_and_si128(b5_u8, nibble_mask);
    let b_hi = _mm_and_si128(b5_u8, hi_bit_mask);
    let b_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(b_hi, hi_bit_mask));
    let b8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, b_lo), b_hi_sel);

    // Alpha
    let a = _mm_set1_epi8(-1i8); // 0xFF

    // Interleave to BGRA: [B, G, R, A] × 4
    let br = _mm_unpacklo_epi8(b8, r8);
    let ga = _mm_unpacklo_epi8(g8, a);
    let bgra = _mm_unpacklo_epi8(br, ga);

    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), bgra);
    out
}

// ---- RGB555 pack to BGRA (AVX2) ----

/// SAFETY: must only be called on `x86_64` where AVX2 is guaranteed.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn rgb555_pack_to_bgra_avx2(pixels: &[[u8; 2]; 4], swap: bool) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi8, _mm_and_si128, _mm_cmpeq_epi8, _mm_loadl_epi64, _mm_packus_epi16, _mm_set1_epi8,
        _mm_setr_epi8, _mm_setzero_si128, _mm_shuffle_epi8, _mm_storeu_si128, _mm_unpacklo_epi8, _mm256_and_si256,
        _mm256_cvtepu16_epi32, _mm256_extracti128_si256, _mm256_packus_epi32, _mm256_set1_epi32, _mm256_setzero_si256,
        _mm256_srli_epi32, _mm256_xor_si256,
    };

    // Load 8 bytes (4 u16 LE pixels) — native LE, no byte-swap needed.
    let v128 = _mm_loadl_epi64(pixels.as_ptr().cast::<__m128i>());
    // Zero-extend 4 x u16 -> 4 x i32 (AVX2).
    let w = _mm256_cvtepu16_epi32(v128);

    let mask5 = _mm256_set1_epi32(0x1F);
    let zero = _mm256_setzero_si256();

    // Default positions: R=high 5 bits (>>10), G=mid 5 (>>5), B=low 5 (no shift).
    let r5_default = _mm256_and_si256(_mm256_srli_epi32(w, 10), mask5);
    let g5 = _mm256_and_si256(_mm256_srli_epi32(w, 5), mask5);
    let b5_default = _mm256_and_si256(w, mask5);

    // Branchless R<->B swap.
    let swap_vec = if swap { _mm256_set1_epi32(-1) } else { zero };
    let diff = _mm256_xor_si256(r5_default, b5_default);
    let masked = _mm256_and_si256(diff, swap_vec);
    let r5 = _mm256_xor_si256(r5_default, masked);
    let b5 = _mm256_xor_si256(b5_default, masked);

    // (5-bit to 8-bit MSB replication deferred to pshufb after packing.)

    // Pack the RAW 5-bit values (not expanded) down to bytes.
    let r16x8 = _mm256_packus_epi32(r5, zero);
    let g16x8 = _mm256_packus_epi32(g5, zero);
    let b16x8 = _mm256_packus_epi32(b5, zero);

    let r_lo = _mm256_extracti128_si256(r16x8, 0);
    let g_lo = _mm256_extracti128_si256(g16x8, 0);
    let b_lo = _mm256_extracti128_si256(b16x8, 0);

    let r5_u8 = _mm_packus_epi16(r_lo, _mm_setzero_si128());
    let g5_u8 = _mm_packus_epi16(g_lo, _mm_setzero_si128());
    let b5_u8 = _mm_packus_epi16(b_lo, _mm_setzero_si128());

    // ---- SSSE3 pshufb-based 5-bit to 8-bit expansion ----
    let expand_lut = _mm_setr_epi8(0, 8, 16, 24, 33, 41, 49, 57, 66, 74, 82, 90, 99, 107, 115, 123);
    let hi_bit_mask = _mm_set1_epi8(0x10i8);
    let hi_offset = _mm_set1_epi8(-124i8); // 0x84
    let nibble_mask = _mm_set1_epi8(0x0Fi8);

    let r_lo_nib = _mm_and_si128(r5_u8, nibble_mask);
    let r_hi = _mm_and_si128(r5_u8, hi_bit_mask);
    let r_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(r_hi, hi_bit_mask));
    let r8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, r_lo_nib), r_hi_sel);

    let g_lo_nib = _mm_and_si128(g5_u8, nibble_mask);
    let g_hi = _mm_and_si128(g5_u8, hi_bit_mask);
    let g_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(g_hi, hi_bit_mask));
    let g8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, g_lo_nib), g_hi_sel);

    let b_lo_nib = _mm_and_si128(b5_u8, nibble_mask);
    let b_hi = _mm_and_si128(b5_u8, hi_bit_mask);
    let b_hi_sel = _mm_and_si128(hi_offset, _mm_cmpeq_epi8(b_hi, hi_bit_mask));
    let b8 = _mm_add_epi8(_mm_shuffle_epi8(expand_lut, b_lo_nib), b_hi_sel);

    // Alpha + interleave to BGRA
    let a = _mm_set1_epi8(-1i8);
    let br = _mm_unpacklo_epi8(b8, r8);
    let ga = _mm_unpacklo_epi8(g8, a);
    let bgra = _mm_unpacklo_epi8(br, ga);

    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), bgra);
    out
}

// ---- RGB555 pack to BGRA dispatch ----

#[must_use]
pub fn rgb555_pack_to_bgra(pixels: [[u8; 2]; 4], swap: bool) -> [u8; 16] {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: checked by is_x86_feature_detected! below.
    if is_x86_feature_detected!("avx2") {
        unsafe {
            return rgb555_pack_to_bgra_avx2(&pixels, swap);
        }
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    // SAFETY: x86_64/x86 guarantees SSE2.
    unsafe {
        rgb555_pack_to_bgra_sse2(&pixels, swap)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    // Portable scalar
    super::scalar::rgb555_pack_to_bgra(pixels, swap)
}
