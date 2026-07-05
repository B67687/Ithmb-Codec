//! YCbCr 4:2:0 planar decoder.
//!
//! Decodes a planar YCbCr 4:2:0 frame to BGRA8 output. The input consists of
//! three separate planes:
//!
//! * **Y plane** — full resolution (`width × height` bytes).
//! * **Cb plane** — quarter resolution (`(width / 2) × (height / 2)` bytes).
//! * **Cr plane** — quarter resolution (`(width / 2) × (height / 2)` bytes).
//!
//! The Cb and Cr plane order is controlled by
//! [`Profile::swap_chroma_planes`](crate::profile::Profile::swap_chroma_planes):
//!
//! | `swap_chroma_planes` | Plane order |
//! |---|---|
//! | `false` (default) | Y, Cb, Cr |
//! | `true` | Y, Cr, Cb |
//!
//! Chroma upsampling uses nearest-neighbour: each Cb/Cr sample covers a 2×2
//! block of luma pixels. Color conversion uses BT.601 coefficients via
//! [`crate::yuv::yuv_to_bgra`].

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
#[allow(unused_imports)]
use crate::yuv;
use std::sync::atomic::AtomicBool;

/// Decode a YCbCr 4:2:0 planar frame to BGRA8.
///
/// # Arguments
///
/// * `src` — Raw planar data (Y plane, then Cb and Cr planes in the order
///   determined by [`Profile::swap_chroma_planes`]).
/// * `profile` — Frame profile providing dimensions and plane-order flags.
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `InvalidFormat` | Non-positive, or odd width or height |
/// | `BufferTooShort` | Input is smaller than `w×h + (w/2)×(h/2)×2` |
pub fn decode(src: &[u8], profile: &Profile, canceled: &AtomicBool) -> Result<DecodedImage, DecodeError> {
    let (w, h) = crate::decoder_helpers::validate_dimensions(src, profile, "width and height must be positive", 0)?;

    // YCbCr 4:2:0 chroma subsampling requires even dimensions.
    if w % 2 != 0 || h % 2 != 0 {
        return Err(DecodeError::InvalidFormat(
            "YCbCr 4:2:0 requires even width and height".into(),
        ));
    }

    let cb_w = w / 2;
    let cb_h = h / 2;
    let cb_size = cb_w * cb_h;

    let y_size = w * h;
    let expected = y_size + cb_size * 2;

    if src.len() < expected {
        return Err(DecodeError::BufferTooShort {
            expected,
            actual: src.len(),
        });
    }

    // Slice the three planes from the input respecting chroma plane order.
    let y_plane = &src[..y_size];
    let (cb_plane, cr_buf) = if profile.swap_chroma_planes {
        // Cr first, Cb second.
        (
            &src[y_size + cb_size..y_size + cb_size * 2],
            &src[y_size..y_size + cb_size],
        )
    } else {
        // Cb first, Cr second (default).
        (
            &src[y_size..y_size + cb_size],
            &src[y_size + cb_size..y_size + cb_size * 2],
        )
    };

    let mut dst = vec![0u8; w * h * 4];

    #[cfg(feature = "simd")]
    {
        // SIMD path: process 2x2 macroblocks in row-pair batches via
        // `crate::simd::yuv420_row_pair_to_bgra`, eliminating per-quad dispatch.
        let block_h = h / 2;
        for cy in 0..block_h {
            crate::pixel_utils::check_canceled(canceled, "ycbcr420 decode canceled")?;
            let y_base = cy * 2 * w;
            let cb_base = cy * cb_w;
            crate::simd::yuv420_row_pair_to_bgra(
                &y_plane[y_base..y_base + 2 * w],
                &cb_plane[cb_base..cb_base + cb_w],
                &cr_buf[cb_base..cb_base + cb_w],
                &mut dst[y_base * 4..(y_base + 2 * w) * 4],
                w,
                cb_w,
            );
        }
    }

    #[cfg(not(feature = "simd"))]
    {
        for y in 0..h {
            crate::pixel_utils::check_canceled(canceled, "ycbcr420 decode canceled")?;
            let cy = y / 2;
            for x in 0..w {
                let cx = x / 2;
                let y_val = y_plane[y * w + x];
                let cb = cb_plane[cy * cb_w + cx];
                let cr = cr_buf[cy * cb_w + cx];
                let pixel = yuv::yuv_to_bgra(y_val, cb, cr);
                let dst_off = (y * w + x) * 4;
                dst[dst_off..dst_off + 4].copy_from_slice(&pixel);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    Ok(DecodedImage {
        data: dst,
        width: w as u32,
        height: h as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Encoding;
    use std::sync::atomic::AtomicBool;

    /// Helper: build a minimal YCbCr 4:2:0 profile.
    fn make_profile(w: i32, h: i32) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::Ycbcr420,
            frame_byte_length: {
                let cb_size = (w / 2) * (h / 2);
                w * h + cb_size * 2
            },
            ..Default::default()
        }
    }

    /// Helper: build a profile with swapped chroma planes.
    fn make_profile_swapped(w: i32, h: i32) -> Profile {
        Profile {
            swap_chroma_planes: true,
            ..make_profile(w, h)
        }
    }

    // -----------------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------------

    #[test]
    fn zero_width() {
        let p = make_profile(0, 4);
        match decode(&[], &p, &AtomicBool::new(false)) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn zero_height() {
        let p = make_profile(4, 0);
        match decode(&[], &p, &AtomicBool::new(false)) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn odd_width() {
        let p = make_profile(3, 4);
        match decode(&[0u8; 3 * 4 + 2 * 2 * 2], &p, &AtomicBool::new(false)) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn odd_height() {
        let p = make_profile(4, 3);
        match decode(&[0u8; 4 * 3 + 2 * 2 * 2], &p, &AtomicBool::new(false)) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn buffer_too_short() {
        let p = make_profile(4, 4);
        // FIXED match decode
        match decode(&[0u8; 4], &p, &AtomicBool::new(false)) {
            Err(DecodeError::BufferTooShort { expected, actual }) => {
                assert_eq!(expected, 4 * 4 + 2 * 2 * 2);
                assert_eq!(actual, 4);
            }
            other => panic!("expected BufferTooShort, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Chroma upsampling — a single Cb/Cr pair covers a 2×2 pixel block
    // -----------------------------------------------------------------------

    #[test]
    fn chroma_upsampling_2x2() {
        // 2×2 image. Y = [10, 20, 30, 40], Cb = [150], Cr = [200].
        // Every pixel should use Cb=150, Cr=200.
        let src: Vec<u8> = vec![
            10, 20, 30, 40,  // Y plane (4 bytes)
            150, // Cb plane (1 byte)
            200, // Cr plane (1 byte)
        ];
        let p = make_profile(2, 2);
        let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();

        let px00 = yuv::yuv_to_bgra(10, 150, 200);
        let px10 = yuv::yuv_to_bgra(20, 150, 200);
        let px01 = yuv::yuv_to_bgra(30, 150, 200);
        let px11 = yuv::yuv_to_bgra(40, 150, 200);
        let expected: Vec<u8> = [px00, px10, px01, px11].concat();
        assert_eq!(img.data, expected);
    }

    // -----------------------------------------------------------------------
    // 4×4 known pattern — verify BGRA output, chroma grid, and plane order
    // -----------------------------------------------------------------------

    #[allow(clippy::similar_names, clippy::cast_possible_truncation)]
    #[test]
    fn four_by_four_default_order() {
        // 4×4 image with distinct chroma per 2×2 block.
        //
        // Chroma grid (2×2):
        //   Cb = [[100, 110], [120, 130]]
        //   Cr = [[200, 210], [220, 230]]
        //
        // Y = 0..15 row-major.
        let y: Vec<u8> = (0..16).collect();
        let cb: Vec<u8> = vec![100, 110, 120, 130];
        let cr: Vec<u8> = vec![200, 210, 220, 230];

        let src: Vec<u8> = [y.as_slice(), cb.as_slice(), cr.as_slice()].concat();
        let p = make_profile(4, 4);
        let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data.len(), 4 * 4 * 4);

        // Build expected output pixel by pixel.
        let mut expected = Vec::with_capacity(64);
        for y in 0..4usize {
            for x in 0..4usize {
                let cx = x / 2;
                let cy = y / 2;
                let y_val = (y * 4 + x) as u8;
                let cb_val = cb[cy * 2 + cx];
                let cr_val = cr[cy * 2 + cx];
                expected.extend_from_slice(&yuv::yuv_to_bgra(y_val, cb_val, cr_val));
            }
        }
        assert_eq!(img.data, expected);
    }

    #[allow(clippy::similar_names, clippy::cast_possible_truncation)]
    #[test]
    fn four_by_four_swap_chroma_planes() {
        // Same as above but with Cr before Cb in the payload and
        // swap_chroma_planes=true.
        let y: Vec<u8> = (0..16).collect();
        let cb: Vec<u8> = vec![100, 110, 120, 130];
        let cr: Vec<u8> = vec![200, 210, 220, 230];

        // Payload: Y, Cr, Cb (swapped order).
        let src: Vec<u8> = [y.as_slice(), cr.as_slice(), cb.as_slice()].concat();
        let p = make_profile_swapped(4, 4);
        let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();
        assert_eq!(img.data.len(), 4 * 4 * 4);

        // Expected output is identical to the default-order test.
        let mut expected = Vec::with_capacity(64);
        for y in 0..4usize {
            for x in 0..4usize {
                let cx = x / 2;
                let cy = y / 2;
                let y_val = (y * 4 + x) as u8;
                let cb_val = cb[cy * 2 + cx];
                let cr_val = cr[cy * 2 + cx];
                expected.extend_from_slice(&yuv::yuv_to_bgra(y_val, cb_val, cr_val));
            }
        }
        assert_eq!(img.data, expected);
    }

    // -----------------------------------------------------------------------
    // Gray / neutral chroma
    // -----------------------------------------------------------------------

    #[test]
    fn gray_2x2() {
        // Y=128 everywhere, Cb=128, Cr=128 → every pixel is mid-gray.
        let src: Vec<u8> = vec![
            128u8; 4 + 1 + 1 // Y(4) + Cb(1) + Cr(1)
        ];
        let p = make_profile(2, 2);
        let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();
        for chunk in img.data.chunks_exact(4) {
            assert_eq!(chunk, [128, 128, 128, 255]);
        }
    }

    // -----------------------------------------------------------------------
    // BGRA output invariant
    // -----------------------------------------------------------------------

    #[test]
    fn alpha_is_always_255() {
        let y: Vec<u8> = (0..16).collect();
        let cb: Vec<u8> = vec![100, 110, 120, 130];
        let cr: Vec<u8> = vec![200, 210, 220, 230];
        let src: Vec<u8> = [y.as_slice(), cb.as_slice(), cr.as_slice()].concat();
        let p = make_profile(4, 4);
        let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();
        for (i, chunk) in img.data.chunks_exact(4).enumerate() {
            assert_eq!(chunk[3], 255, "alpha must be 255 at pixel {i}, got {}", chunk[3]);
        }
    }

    // -----------------------------------------------------------------------
    // SIMD cross-validation (1000 random 16×16 images)
    // -----------------------------------------------------------------------

    #[cfg(feature = "simd")]
    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::similar_names
    )]
    fn simd_matches_scalar_1000_random() {
        /// Minimal PRNG for reproducible test data.
        struct QuickRand(u64);

        impl QuickRand {
            fn new(seed: u64) -> Self {
                Self(seed)
            }

            #[allow(clippy::cast_possible_truncation)]
            fn next_u8(&mut self) -> u8 {
                self.0 = self
                    .0
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (self.0 >> 32) as u8
            }
        }

        let w = 16usize;
        let h = 16usize;
        let y_size = w * h;
        let cb_size = (w / 2) * (h / 2);
        let cb_w = w / 2;
        let p = make_profile(w as i32, h as i32);

        for trial in 0..1000 {
            let mut rng = QuickRand::new(42 + trial as u64);

            let y_plane: Vec<u8> = (0..y_size).map(|_| rng.next_u8()).collect();
            let cb_plane: Vec<u8> = (0..cb_size).map(|_| rng.next_u8()).collect();
            let cr_plane: Vec<u8> = (0..cb_size).map(|_| rng.next_u8()).collect();

            let src: Vec<u8> = [y_plane.as_slice(), cb_plane.as_slice(), cr_plane.as_slice()].concat();

            let img = decode(&src, &p, &AtomicBool::new(false)).unwrap();

            // Build expected output using scalar yuv_to_bgra.
            let mut expected = Vec::with_capacity(y_size * 4);
            for y in 0..h {
                let cy = y / 2;
                for x in 0..w {
                    let cx = x / 2;
                    let y_val = y_plane[y * w + x];
                    let cb = cb_plane[cy * cb_w + cx];
                    let cr = cr_plane[cy * cb_w + cx];
                    expected.extend_from_slice(&yuv::yuv_to_bgra(y_val, cb, cr));
                }
            }

            assert_eq!(img.data, expected, "SIMD vs scalar mismatch on trial {trial}");
        }
    }
}
