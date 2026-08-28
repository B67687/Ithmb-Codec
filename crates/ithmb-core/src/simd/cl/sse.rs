//! CL SSE2 quad-level + row conversions.

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn cl_quad_to_bgra_sse2(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_cvtsi32_si128, _mm_set_epi32, _mm_setzero_si128, _mm_storeu_si128, _mm_sub_epi32,
        _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    let mut rc_arr = [0i32; 4];
    let mut gb_arr = [0i32; 4];
    let mut gr_arr = [0i32; 4];
    let mut bc_arr = [0i32; 4];
    for i in 0..4 {
        let raw = quad[4 + i];
        let cb = i32::from((raw & 0x0F) << 4) - 128;
        let cr = i32::from(raw & 0xF0) - 128;
        rc_arr[i] = (cr * 359) >> 8;
        gb_arr[i] = (cb * 88) >> 8;
        gr_arr[i] = (cr * 183) >> 8;
        bc_arr[i] = (cb * 454) >> 8;
    }

    let y_bytes = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y_words = _mm_unpacklo_epi8(y_bytes, _mm_setzero_si128());
    let y = _mm_unpacklo_epi16(y_words, _mm_setzero_si128());

    let rc = _mm_set_epi32(rc_arr[3], rc_arr[2], rc_arr[1], rc_arr[0]);
    let gb = _mm_set_epi32(gb_arr[3], gb_arr[2], gb_arr[1], gb_arr[0]);
    let gr = _mm_set_epi32(gr_arr[3], gr_arr[2], gr_arr[1], gr_arr[0]);
    let bc = _mm_set_epi32(bc_arr[3], bc_arr[2], bc_arr[1], bc_arr[0]);

    let r = _mm_add_epi32(y, rc);
    let g = _mm_sub_epi32(_mm_sub_epi32(y, gb), gr);
    let b = _mm_add_epi32(y, bc);

    let mut r_arr = [0i32; 4];
    let mut g_arr = [0i32; 4];
    let mut b_arr = [0i32; 4];
    _mm_storeu_si128(r_arr.as_mut_ptr().cast::<__m128i>(), r);
    _mm_storeu_si128(g_arr.as_mut_ptr().cast::<__m128i>(), g);
    _mm_storeu_si128(b_arr.as_mut_ptr().cast::<__m128i>(), b);

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4] = crate::pixel_utils::clamp_u8(b_arr[i]);
        out[i * 4 + 1] = crate::pixel_utils::clamp_u8(g_arr[i]);
        out[i * 4 + 2] = crate::pixel_utils::clamp_u8(r_arr[i]);
        out[i * 4 + 3] = 255;
    }
    out
}

/// SAFETY: must only be called on `x86`/`x86_64` where SSE2 is guaranteed.
#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
pub(crate) unsafe fn cl_row_to_bgra_sse2(src: &[u8], dst: &mut [u8]) {
    let n_pixels = src.len() / 2;
    let (y, chroma) = src.split_at(n_pixels);
    // Process 2 quads (8 pixels) per iteration.
    let full_end = (n_pixels / 8) * 8;
    let mut i = 0usize;
    while i < full_end {
        // Quad 0: pixels i..i+4
        let q0 = [
            y[i],
            y[i + 1],
            y[i + 2],
            y[i + 3],
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ];
        let out0 = cl_quad_to_bgra_sse2(&q0);
        // Quad 1: pixels i+4..i+8
        let q1 = [
            y[i + 4],
            y[i + 5],
            y[i + 6],
            y[i + 7],
            chroma[i + 4],
            chroma[i + 5],
            chroma[i + 6],
            chroma[i + 7],
        ];
        let out1 = cl_quad_to_bgra_sse2(&q1);
        let d_off = i * 4;
        dst[d_off..d_off + 16].copy_from_slice(&out0);
        dst[d_off + 16..d_off + 32].copy_from_slice(&out1);
        i += 8;
    }
    // Remainder: process one final quad if 4+ pixels remain.
    while i + 4 <= n_pixels {
        let q = [
            y[i],
            y[i + 1],
            y[i + 2],
            y[i + 3],
            chroma[i],
            chroma[i + 1],
            chroma[i + 2],
            chroma[i + 3],
        ];
        let out = cl_quad_to_bgra_sse2(&q);
        let d_off = i * 4;
        dst[d_off..d_off + 16].copy_from_slice(&out);
        i += 4;
    }
    // Remaining 0-3 pixels via scalar (can't form a full quad).
    for j in i..n_pixels {
        let cr = chroma[j] & 0xF0; // high nibble → Cr
        let cb = (chroma[j] & 0x0F) << 4; // low nibble → Cb
        let px = crate::yuv::yuv_to_bgra(y[j], cb, cr);
        let o = j * 4;
        dst[o..o + 4].copy_from_slice(&px);
    }
}

/// SAFETY: must only be called on `x86_64` where SSE4.1 is guaranteed.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[inline]
#[allow(unsafe_op_in_unsafe_fn, clippy::similar_names)]
pub(crate) unsafe fn cl_quad_to_bgra_sse41(quad: &[u8; 8]) -> [u8; 16] {
    use core::arch::x86_64::{
        __m128i, _mm_add_epi32, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_max_epi32, _mm_min_epi32, _mm_packus_epi16,
        _mm_packus_epi32, _mm_set_epi32, _mm_set1_epi16, _mm_set1_epi32, _mm_setzero_si128, _mm_storeu_si128,
        _mm_sub_epi32, _mm_unpacklo_epi8, _mm_unpacklo_epi16,
    };

    // ---- Precompute per-pixel chroma contributions ----
    let mut rc_arr = [0i32; 4];
    let mut gb_arr = [0i32; 4];
    let mut gr_arr = [0i32; 4];
    let mut bc_arr = [0i32; 4];
    for i in 0..4 {
        let raw = quad[4 + i];
        let cb = i32::from((raw & 0x0F) << 4) - 128;
        let cr = i32::from(raw & 0xF0) - 128;
        rc_arr[i] = (cr * 359) >> 8;
        gb_arr[i] = (cb * 88) >> 8;
        gr_arr[i] = (cr * 183) >> 8;
        bc_arr[i] = (cb * 454) >> 8;
    }

    // ---- Load 4 Y values and SSE4.1 zero-extend to 32-bit ----
    let y_chunk = _mm_cvtsi32_si128(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]));
    let y = _mm_cvtepu8_epi32(y_chunk);

    // ---- Load per-pixel chroma contributions ----
    let rc = _mm_set_epi32(rc_arr[3], rc_arr[2], rc_arr[1], rc_arr[0]);
    let gb = _mm_set_epi32(gb_arr[3], gb_arr[2], gb_arr[1], gb_arr[0]);
    let gr = _mm_set_epi32(gr_arr[3], gr_arr[2], gr_arr[1], gr_arr[0]);
    let bc = _mm_set_epi32(bc_arr[3], bc_arr[2], bc_arr[1], bc_arr[0]);

    // ---- Compute R/G/B with packed arithmetic ----
    let r = _mm_add_epi32(y, rc);
    let g = _mm_sub_epi32(_mm_sub_epi32(y, gb), gr);
    let b = _mm_add_epi32(y, bc);

    // ---- Clamp to [0, 255] with packed min/max ----
    let zero = _mm_setzero_si128();
    let max_val = _mm_set1_epi32(255);
    let r_c = _mm_max_epi32(_mm_min_epi32(r, max_val), zero);
    let g_c = _mm_max_epi32(_mm_min_epi32(g, max_val), zero);
    let b_c = _mm_max_epi32(_mm_min_epi32(b, max_val), zero);

    // ---- Pack i32 -> u16 -> u8 with interleave to BGRA order ----
    let b16 = _mm_packus_epi32(b_c, zero);
    let g16 = _mm_packus_epi32(g_c, zero);
    let r16 = _mm_packus_epi32(r_c, zero);
    let a16 = _mm_set1_epi16(255i16);

    let br = _mm_unpacklo_epi16(b16, r16);
    let ga = _mm_unpacklo_epi16(g16, a16);

    let br_u8 = _mm_packus_epi16(br, zero);
    let ga_u8 = _mm_packus_epi16(ga, zero);

    let result = _mm_unpacklo_epi8(br_u8, ga_u8);

    let mut out = [0u8; 16];
    _mm_storeu_si128(out.as_mut_ptr().cast::<__m128i>(), result);
    out
}
