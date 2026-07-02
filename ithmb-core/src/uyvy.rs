//! UYVY (YUV 4:2:2) decoder.
//!
//! UYVY is a packed YUV format where each 4-byte group encodes two pixels
//! that share U (Cb) and V (Cr) chroma values.
//!
//! ```text
//! Byte 0: U0 (Cb)     — shared chroma blue-difference
//! Byte 1: Y0          — luma for pixel 0
//! Byte 2: V0 (Cr)     — shared chroma red-difference
//! Byte 3: Y1          — luma for pixel 1
//! ```
//!
//! Color conversion uses BT.601 coefficients via [`crate::yuv::yuv_to_bgra`].

use crate::error::{DecodeError, DecodedImage};
use crate::profile::Profile;
use crate::yuv;
/// Decode a UYVY frame to BGRA8.
///
/// # Arguments
///
/// * `src` — Raw UYVY data (2 bytes per pixel in the format above).
/// * `profile` — Frame profile providing dimensions and interlacing flag.
///
/// # Errors
///
/// | Variant | Condition |
/// |---|---|
/// | `InvalidFormat` | Non-positive width or height, or odd height with interlacing |
/// | `BufferTooShort` | Input is smaller than `w × h × 2` |
pub fn decode(src: &[u8], profile: &Profile) -> Result<DecodedImage, DecodeError> {
    let w_i32 = profile.width;
    let h_i32 = profile.height;

    if w_i32 <= 0 || h_i32 <= 0 {
        return Err(DecodeError::InvalidFormat("width and height must be positive".into()));
    }

    #[allow(clippy::cast_sign_loss)]
    let w = w_i32 as usize;
    #[allow(clippy::cast_sign_loss)]
    let h = h_i32 as usize;

    let expected = w * h * 2;
    if src.len() < expected {
        return Err(DecodeError::BufferTooShort {
            expected,
            actual: src.len(),
        });
    }

    let mut dst = vec![0u8; w * h * 4];

    if profile.is_interlaced {
        if !h.is_multiple_of(2) {
            return Err(DecodeError::InvalidFormat(
                "interlaced UYVY requires an even height".into(),
            ));
        }
        decode_interlaced(src, w, h, &mut dst);
    } else {
        decode_progressive(src, w, h, &mut dst);
    }

    #[allow(clippy::cast_possible_truncation)]
    Ok(DecodedImage {
        data: dst,
        width: w as u32,
        height: h as u32,
    })
}

/// Decode a non-interlaced UYVY frame row by row.
fn decode_progressive(src: &[u8], w: usize, h: usize, dst: &mut [u8]) {
    let row_stride = w * 2;
    for y in 0..h {
        let src_off = y * row_stride;
        let dst_off = y * w * 4;
        decode_row(&src[src_off..], w, &mut dst[dst_off..]);
    }
}

/// Decode an interlaced UYVY frame with separate even/odd fields.
///
/// The first half of `src` holds even rows (0, 2, 4, …) and the second
/// half holds odd rows (1, 3, 5, …). After decoding both fields the
/// rows are weaved into the final frame.
fn decode_interlaced(src: &[u8], w: usize, h: usize, dst: &mut [u8]) {
    let half_rows = h / 2;
    let field_bytes = w * half_rows * 2;

    // Even rows (0, 2, 4, …) from the first field.
    for i in 0..half_rows {
        let src_off = i * w * 2;
        let dst_row = i * 2;
        decode_row(&src[src_off..], w, &mut dst[dst_row * w * 4..]);
    }

    // Odd rows (1, 3, 5, …) from the second field.
    for i in 0..half_rows {
        let src_off = field_bytes + i * w * 2;
        let dst_row = i * 2 + 1;
        decode_row(&src[src_off..], w, &mut dst[dst_row * w * 4..]);
    }
}

/// Convert one row of UYVY data to BGRA.
///
/// Processes pixels in groups of 2 using the 4-byte UYVY group format.
/// For odd widths the last pixel reads its Y and U from the trailing
/// incomplete group and reuses V from the preceding complete group.
fn decode_row(row_src: &[u8], w: usize, row_dst: &mut [u8]) {
    let groups = w / 2;
    for g in 0..groups {
        let src_idx = g * 4;
        let u = row_src[src_idx];
        let y0 = row_src[src_idx + 1];
        let v = row_src[src_idx + 2];
        let y1 = row_src[src_idx + 3];

        let px0 = yuv::yuv_to_bgra(y0, u, v);
        let d0 = g * 8;
        row_dst[d0..d0 + 4].copy_from_slice(&px0);

        let px1 = yuv::yuv_to_bgra(y1, u, v);
        row_dst[d0 + 4..d0 + 8].copy_from_slice(&px1);
    }

    // Odd width: the incomplete trailing pair provides [U, Y] but no V.
    if !w.is_multiple_of(2) {
        let last_src = groups * 4;
        let y = row_src[last_src + 1];
        let u = row_src[last_src];
        // Reuse V from the last complete group, or neutral if no groups.
        let v = if groups > 0 { row_src[groups * 4 - 2] } else { 128 };
        let px = yuv::yuv_to_bgra(y, u, v);
        let d_off = groups * 8;
        row_dst[d_off..d_off + 4].copy_from_slice(&px);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Encoding;

    /// Helper: build a minimal UYVY profile.
    fn make_profile(w: i32, h: i32, interlaced: bool) -> Profile {
        Profile {
            prefix: 0,
            width: w,
            height: h,
            encoding: Encoding::Yuv422,
            frame_byte_length: w * h * 2,
            is_interlaced: interlaced,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // Error cases
    // -----------------------------------------------------------------------

    #[test]
    fn zero_width() {
        let p = make_profile(0, 100, false);
        match decode(&[], &p) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn zero_height() {
        let p = make_profile(100, 0, false);
        match decode(&[], &p) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    #[test]
    fn buffer_too_short() {
        let p = make_profile(4, 4, false);
        match decode(&[0u8; 4], &p) {
            Err(DecodeError::BufferTooShort { expected, actual }) => {
                assert_eq!(expected, 4 * 4 * 2);
                assert_eq!(actual, 4);
            }
            other => panic!("expected BufferTooShort, got {other:?}"),
        }
    }

    #[test]
    fn interlaced_odd_height() {
        let p = make_profile(4, 3, true);
        match decode(&[0u8; 4 * 3 * 2], &p) {
            Err(DecodeError::InvalidFormat(_)) => {}
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Basic decode — gray neutral chroma
    // -----------------------------------------------------------------------

    #[test]
    fn gray_pair_2x1() {
        // U=128, Y0=128, V=128, Y1=128  →  two gray pixels
        let src = [128u8, 128, 128, 128];
        let p = make_profile(2, 1, false);
        let img = decode(&src, &p).unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        let expected = [128u8, 128, 128, 255, 128, 128, 128, 255];
        assert_eq!(img.data, expected);
    }

    #[test]
    fn black_pair_2x1() {
        // U=128, Y=0, V=128, Y=0  →  two black pixels
        let src = [128u8, 0, 128, 0];
        let p = make_profile(2, 1, false);
        let img = decode(&src, &p).unwrap();
        assert_eq!(img.data, [0u8, 0, 0, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn white_pair_2x1() {
        // U=128, Y=255, V=128, Y=255  →  two white pixels
        let src = [128u8, 255, 128, 255];
        let p = make_profile(2, 1, false);
        let img = decode(&src, &p).unwrap();
        assert_eq!(img.data, [255u8, 255, 255, 255, 255, 255, 255, 255]);
    }

    // -----------------------------------------------------------------------
    // 4x4 known pattern — verify BGRA alignment
    // -----------------------------------------------------------------------

    #[test]
    fn four_by_four_pattern() {
        // Build a 4×4 image where each pixel pair shares chroma.
        // Row 0: U=100, Y=10, V=200, Y=20, U=110, Y=30, V=210, Y=40
        // Row 1: U=120, Y=50, V=220, Y=60, U=130, Y=70, V=230, Y=80
        // Row 2: U=140, Y=90, V=240, Y=100, U=150, Y=110, V=250, Y=120
        // Row 3: U=160, Y=130, V=10, Y=140, U=170, Y=150, V=20, Y=160
        let src: Vec<u8> = vec![
            100, 10, 200, 20, 110, 30, 210, 40, 120, 50, 220, 60, 130, 70, 230, 80, 140, 90, 240, 100, 150, 110, 250,
            120, 160, 130, 10, 140, 170, 150, 20, 160,
        ];

        let p = make_profile(4, 4, false);
        let img = decode(&src, &p).unwrap();
        assert_eq!(img.data.len(), 4 * 4 * 4);

        // Verify pixel (row=0, col=0): Y=10, U=100, V=200
        let px00 = yuv::yuv_to_bgra(10, 100, 200);
        assert_eq!(img.data[0..4], px00);

        // Verify pixel (row=0, col=1): Y=20, U=100, V=200
        let px01 = yuv::yuv_to_bgra(20, 100, 200);
        assert_eq!(img.data[4..8], px01);

        // Verify pixel (row=0, col=2): Y=30, U=110, V=210
        let px02 = yuv::yuv_to_bgra(30, 110, 210);
        assert_eq!(img.data[8..12], px02);

        // Verify pixel (row=0, col=3): Y=40, U=110, V=210
        let px03 = yuv::yuv_to_bgra(40, 110, 210);
        assert_eq!(img.data[12..16], px03);

        // Verify pixel (row=3, col=0): Y=130, U=160, V=10
        let px30 = yuv::yuv_to_bgra(130, 160, 10);
        assert_eq!(img.data[48..52], px30);

        // Verify pixel (row=3, col=3): Y=160, U=170, V=20
        let px33 = yuv::yuv_to_bgra(160, 170, 20);
        assert_eq!(img.data[60..64], px33);
    }

    // -----------------------------------------------------------------------
    // Interlaced decode
    // -----------------------------------------------------------------------

    #[test]
    fn interlaced_4x2() {
        // 4×2 interlaced: 2 fields of 1 row each (4 pixels per row).
        // Field 0 (even rows — written to row 0):
        //   [U=100, Y=10, V=200, Y=20, U=110, Y=30, V=210, Y=40]
        // Field 1 (odd rows  — written to row 1):
        //   [U=150, Y=50, V=250, Y=60, U=160, Y=70, V=10,  Y=80]
        let even_field: Vec<u8> = vec![100, 10, 200, 20, 110, 30, 210, 40];
        let odd_field: Vec<u8> = vec![150, 50, 250, 60, 160, 70, 10, 80];
        let src: Vec<u8> = [even_field.as_slice(), odd_field.as_slice()].concat();

        let p = make_profile(4, 2, true);
        let img = decode(&src, &p).unwrap();
        assert_eq!(img.data.len(), 4 * 2 * 4);

        // Row 0 should match the even field.
        let px00 = yuv::yuv_to_bgra(10, 100, 200);
        let px01 = yuv::yuv_to_bgra(20, 100, 200);
        let px02 = yuv::yuv_to_bgra(30, 110, 210);
        let px03 = yuv::yuv_to_bgra(40, 110, 210);
        assert_eq!(img.data[0..16], [px00, px01, px02, px03].concat());

        // Row 1 should match the odd field.
        let px10 = yuv::yuv_to_bgra(50, 150, 250);
        let px11 = yuv::yuv_to_bgra(60, 150, 250);
        let px12 = yuv::yuv_to_bgra(70, 160, 10);
        let px13 = yuv::yuv_to_bgra(80, 160, 10);
        assert_eq!(img.data[16..32], [px10, px11, px12, px13].concat());
    }

    #[test]
    fn interlaced_matches_manual_weave() {
        // 6×4 interlaced: 2 fields of 2 rows each.
        // Manually decode each field with decode_progressive, then weave.
        let w = 6usize;
        let h = 4usize;
        let half = h / 2;

        let mut field0 = Vec::with_capacity(w * half * 2);
        let mut field1 = Vec::with_capacity(w * half * 2);
        for row in 0..half {
            for px in 0..w / 2 {
                let u = (row * 50 + px * 30 + 10) as u8;
                let y0 = (row * 40 + px * 20 + 5) as u8;
                let v = (row * 50 + px * 30 + 20) as u8;
                let y1 = (row * 40 + px * 20 + 15) as u8;
                field0.extend_from_slice(&[u, y0, v, y1]);
            }
        }
        for row in 0..half {
            for px in 0..w / 2 {
                let u = (row * 70 + px * 40 + 100) as u8;
                let y0 = (row * 60 + px * 30 + 50) as u8;
                let v = (row * 70 + px * 40 + 120) as u8;
                let y1 = (row * 60 + px * 30 + 60) as u8;
                field1.extend_from_slice(&[u, y0, v, y1]);
            }
        }

        let interlaced_src: Vec<u8> = [field0.as_slice(), field1.as_slice()].concat();
        let p = make_profile(w as i32, h as i32, true);
        let img = decode(&interlaced_src, &p).unwrap();

        // Manually weave: decode fields separately, interleave rows.
        let mut manual = vec![0u8; w * h * 4];
        let mut row0_dst = vec![0u8; w * half * 4];
        let mut row1_dst = vec![0u8; w * half * 4];
        decode_progressive(&field0, w, half, &mut row0_dst);
        decode_progressive(&field1, w, half, &mut row1_dst);

        for i in 0..half {
            let dst_off_even = i * 2 * w * 4;
            let dst_off_odd = (i * 2 + 1) * w * 4;
            let src_off = i * w * 4;
            manual[dst_off_even..dst_off_even + w * 4].copy_from_slice(&row0_dst[src_off..src_off + w * 4]);
            manual[dst_off_odd..dst_off_odd + w * 4].copy_from_slice(&row1_dst[src_off..src_off + w * 4]);
        }

        assert_eq!(img.data, manual);
    }

    // -----------------------------------------------------------------------
    // BGRA output alignment — each pixel is exactly 4 bytes
    // -----------------------------------------------------------------------

    #[test]
    fn bgra_output_alpha_is_always_255() {
        let mut src: Vec<u8> = Vec::new();
        // 2×2 image = 2 rows × 4 bytes = 8 bytes
        for y in 0..2i32 {
            for x in 0..2 / 2 {
                src.push(100); // U
                src.push((y * 50 + x * 30) as u8); // Y0
                src.push(200); // V
                src.push((y * 50 + x * 30 + 20) as u8); // Y1
            }
        }
        let p = make_profile(2, 2, false);
        let img = decode(&src, &p).unwrap();
        for (i, chunk) in img.data.chunks_exact(4).enumerate() {
            assert_eq!(chunk[3], 255, "alpha must be 255 at pixel {i}, got {}", chunk[3]);
        }
    }
}
