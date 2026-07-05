//! CLCL (separate Cb/Cr nibble-chroma planes) -> BGRA - SIMD-accelerated (SSE2 on `x86_64`).
#![allow(
    clippy::many_single_char_names,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::cast_sign_loss
)]

/// Process one row of CLCL data via SSE2.
///
/// Reads `width` Y bytes, `width/2` Cb bytes (nibble-packed, 2 pixels/byte),
/// `width/2` Cr bytes (same) and writes `width*4` BGRA bytes.
///
/// # Safety
///
/// - `y_ptr` must point to `width` valid bytes.
/// - `cb_ptr` must point to `width / 2` valid bytes.
/// - `cr_ptr` must point to `width / 2` valid bytes.
/// - `dst` must point to `width * 4` valid bytes.
/// - Requires `feature = "simd"` and `x86_64` target.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[allow(clippy::many_single_char_names)]
pub unsafe fn clcl_row_to_bgra_sse2(y_ptr: *const u8, cb_ptr: *const u8, cr_ptr: *const u8, width: u32, dst: *mut u8) {
    unsafe {
        use core::arch::x86_64::{
            __m128i, _mm_and_si128, _mm_cvtsi32_si128, _mm_loadl_epi64, _mm_set1_epi8, _mm_slli_epi16, _mm_srli_epi16,
            _mm_storeu_si128, _mm_unpacklo_epi8,
        };

        let w = width as usize;
        let full_batches = (w / 8) * 8;
        let mut i: usize = 0;
        let low_mask = _mm_set1_epi8(0x0F);

        while i < full_batches {
            // ---- Load 8 Y bytes ----
            let y_val = _mm_loadl_epi64(y_ptr.add(i).cast::<__m128i>());

            // ---- Load 4 Cb/Cr bytes (covering 8 pixels: 2 pixels per byte) ----
            let cb_word = core::ptr::read_unaligned(cb_ptr.add(i / 2).cast::<i32>());
            let cr_word = core::ptr::read_unaligned(cr_ptr.add(i / 2).cast::<i32>());
            let cb_raw = _mm_cvtsi32_si128(cb_word);
            let cr_raw = _mm_cvtsi32_si128(cr_word);

            // ---- Extract low/high nibbles ----
            // cb_raw = [Cb0, Cb1, Cb2, Cb3, 0...]
            // cb_lo  = [Cb0&0x0F, Cb1&0x0F, Cb2&0x0F, Cb3&0x0F, 0...]
            // cb_hi  = [Cb0>>4,   Cb1>>4,   Cb2>>4,   Cb3>>4,   0...]
            let cb_lo = _mm_and_si128(cb_raw, low_mask);
            let cb_hi = _mm_and_si128(_mm_srli_epi16(cb_raw, 4), low_mask);
            let cr_lo = _mm_and_si128(cr_raw, low_mask);
            let cr_hi = _mm_and_si128(_mm_srli_epi16(cr_raw, 4), low_mask);

            // ---- Interleave: [lo0, hi0, lo1, hi1, lo2, hi2, lo3, hi3] ----
            let cb_unpacked = _mm_unpacklo_epi8(cb_lo, cb_hi);
            let cr_unpacked = _mm_unpacklo_epi8(cr_lo, cr_hi);

            // ---- Expand nibbles to 8-bit: nibble << 4 ----
            // _mm_slli_epi16 shifts each 16-bit lane left by 4.
            // Each nibble value is 0-15, so 4-bit shift never overflows into
            // the next byte (max 15*16 = 240 < 256).
            let cb_exp = _mm_slli_epi16(cb_unpacked, 4);
            let cr_exp = _mm_slli_epi16(cr_unpacked, 4);

            // ---- Store to stack arrays for per-pixel YUV→BGRA ----
            let mut y_arr: [u8; 8] = [0u8; 8];
            let mut cb_arr: [u8; 8] = [0u8; 8];
            let mut cr_arr: [u8; 8] = [0u8; 8];
            _mm_storeu_si128(y_arr.as_mut_ptr().cast::<__m128i>(), y_val);
            _mm_storeu_si128(cb_arr.as_mut_ptr().cast::<__m128i>(), cb_exp);
            _mm_storeu_si128(cr_arr.as_mut_ptr().cast::<__m128i>(), cr_exp);

            // ---- BT.601 YUV→BGRA (8 pixels, each with own Cb/Cr) ----
            for px in 0..8usize {
                let y = i32::from(y_arr[px]);
                let cb = i32::from(cb_arr[px]).wrapping_sub(128);
                let cr = i32::from(cr_arr[px]).wrapping_sub(128);
                let r = (y + ((cr * 359) >> 8)).clamp(0, 255) as u8;
                let g = (y - ((cb * 88) >> 8) - ((cr * 183) >> 8)).clamp(0, 255) as u8;
                let b = (y + ((cb * 454) >> 8)).clamp(0, 255) as u8;
                let out = dst.add(i * 4 + px * 4);
                *out = b;
                *out.add(1) = g;
                *out.add(2) = r;
                *out.add(3) = 255;
            }

            i += 8;
        }

        // ---- Scalar remainder (< 8 pixels) ----
        while i < w {
            let y = i32::from(*y_ptr.add(i));
            let cb_byte = *cb_ptr.add(i / 2);
            let cr_byte = *cr_ptr.add(i / 2);
            let n_cb = if i & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
            let n_cr = if i & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
            let cb8 = i32::from(n_cb) << 4;
            let cr8 = i32::from(n_cr) << 4;
            let cb = cb8.wrapping_sub(128);
            let cr = cr8.wrapping_sub(128);
            let r = (y + ((cr * 359) >> 8)).clamp(0, 255) as u8;
            let g = (y - ((cb * 88) >> 8) - ((cr * 183) >> 8)).clamp(0, 255) as u8;
            let b = (y + ((cb * 454) >> 8)).clamp(0, 255) as u8;
            let out = dst.add(i * 4);
            *out = b;
            *out.add(1) = g;
            *out.add(2) = r;
            *out.add(3) = 255;
            i += 1;
        }
    }
}

/// Decode one row of CLCL data (separate Y/Cb/Cr planes) to BGRA.
///
/// On `x86_64` with `simd` feature: uses SSE2 (processes 8 px/iter).
/// Otherwise: uses scalar fallback.
#[cfg(feature = "simd")]
#[inline]
pub fn clcl_row_to_bgra(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SSE2 is guaranteed on x86_64 (baseline).
        // SAFETY: The raw pointers are derived from valid slices with correct lengths.
        unsafe {
            clcl_row_to_bgra_sse2(y.as_ptr(), cb.as_ptr(), cr.as_ptr(), width as u32, dst.as_mut_ptr());
        }
        return;
    }
    // Fallback (non-x86_64) — scalar
    #[allow(unreachable_code)]
    scalar_fallback(y, cb, cr, width, dst);
}

#[cfg(not(feature = "simd"))]
#[inline]
pub fn clcl_row_to_bgra(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    scalar_fallback(y, cb, cr, width, dst);
}

/// Scalar fallback for CLCL row decode.
#[allow(dead_code)]
fn scalar_fallback(y: &[u8], cb: &[u8], cr: &[u8], width: usize, dst: &mut [u8]) {
    for i in 0..width {
        let cb_byte = cb[i / 2];
        let cr_byte = cr[i / 2];
        let n_cb = if i & 1 == 0 { cb_byte & 0x0F } else { cb_byte >> 4 };
        let n_cr = if i & 1 == 0 { cr_byte & 0x0F } else { cr_byte >> 4 };
        let pixel = crate::yuv::yuv_to_bgra(y[i], n_cb << 4, n_cr << 4);
        let out = i * 4;
        dst[out..out + 4].copy_from_slice(&pixel);
    }
}
