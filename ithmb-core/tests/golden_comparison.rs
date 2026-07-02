//! Golden-vector comparison tests.
//!
//! Each test reads a raw `.enc` input (produced by the C# reference encoder),
//! decodes it with the Rust decoder, and compares the BGRA output against the
//! reference `.bin` file — proving bit-exact compatibility.

use jpeg_decoder as _;
use thiserror as _;

use ithmb_core::DecodedImage;
use ithmb_core::profile::{Encoding, Profile};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn decode_rgb(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::rgb565::decode(enc, profile).unwrap()
}

fn decode_rgb555(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::rgb555::decode(enc, profile).unwrap()
}

fn decode_uyvy(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::uyvy::decode(enc, profile).unwrap()
}

fn decode_ycbcr420(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::ycbcr420::decode(enc, profile).unwrap()
}

fn decode_cl(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::cl::decode(enc, profile).unwrap()
}

fn decode_clcl(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::clcl::decode(enc, profile).unwrap()
}

fn decode_jpeg(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::jpeg::decode(enc, profile).unwrap()
}

fn check(enc: &[u8], expected: &[u8], w: u32, h: u32, result: &DecodedImage, label: &str) {
    assert_eq!(result.width, w, "{label}: width mismatch");
    assert_eq!(result.height, h, "{label}: height mismatch");
    assert_eq!(
        result.data.as_slice(),
        expected,
        "{label}: pixel data mismatch ({} enc bytes -> expected {} bin bytes, got {} res bytes)",
        enc.len(),
        expected.len(),
        result.data.len(),
    );
}

// ---------------------------------------------------------------------------
// RGB565
// ---------------------------------------------------------------------------

#[test]
fn golden_rgb565_solid_white_2x2() {
    let enc = include_bytes!("../../tests/golden/rgb565/solid_white_2x2.enc");
    let expected = include_bytes!("../../tests/golden/rgb565/solid_white_2x2.bin");
    let profile = Profile {
        width: 2,
        height: 2,
        encoding: Encoding::Rgb565,
        frame_byte_length: 8,
        ..Default::default()
    };
    let result = decode_rgb(enc, &profile);
    check(enc, expected, 2, 2, &result, "rgb565/solid_white_2x2");
}

#[test]
fn golden_rgb565_solid_red_2x2() {
    let enc = include_bytes!("../../tests/golden/rgb565/solid_red_2x2.enc");
    let expected = include_bytes!("../../tests/golden/rgb565/solid_red_2x2.bin");
    let profile = Profile {
        width: 2,
        height: 2,
        encoding: Encoding::Rgb565,
        frame_byte_length: 8,
        ..Default::default()
    };
    let result = decode_rgb(enc, &profile);
    check(enc, expected, 2, 2, &result, "rgb565/solid_red_2x2");
}

#[test]
fn golden_rgb565_gradient_4x4() {
    let enc = include_bytes!("../../tests/golden/rgb565/gradient_4x4.enc");
    let expected = include_bytes!("../../tests/golden/rgb565/gradient_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Rgb565,
        frame_byte_length: 32,
        ..Default::default()
    };
    let result = decode_rgb(enc, &profile);
    check(enc, expected, 4, 4, &result, "rgb565/gradient_4x4");
}

// ---------------------------------------------------------------------------
// RGB555
// ---------------------------------------------------------------------------

#[test]
fn golden_rgb555_solid_white_2x2() {
    let enc = include_bytes!("../../tests/golden/rgb555/solid_white_2x2.enc");
    let expected = include_bytes!("../../tests/golden/rgb555/solid_white_2x2.bin");
    let profile = Profile {
        width: 2,
        height: 2,
        encoding: Encoding::Rgb555,
        frame_byte_length: 8,
        ..Default::default()
    };
    let result = decode_rgb555(enc, &profile);
    check(enc, expected, 2, 2, &result, "rgb555/solid_white_2x2");
}

#[test]
fn golden_rgb555_gradient_4x4() {
    let enc = include_bytes!("../../tests/golden/rgb555/gradient_4x4.enc");
    let expected = include_bytes!("../../tests/golden/rgb555/gradient_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Rgb555,
        frame_byte_length: 32,
        ..Default::default()
    };
    let result = decode_rgb555(enc, &profile);
    check(enc, expected, 4, 4, &result, "rgb555/gradient_4x4");
}

// ---------------------------------------------------------------------------
// UYVY
// ---------------------------------------------------------------------------

#[test]
fn golden_uyvy_solid_white_2x2() {
    let enc = include_bytes!("../../tests/golden/uyvy/solid_white_2x2.enc");
    let expected = include_bytes!("../../tests/golden/uyvy/solid_white_2x2.bin");
    let profile = Profile {
        width: 2,
        height: 2,
        encoding: Encoding::Yuv422,
        frame_byte_length: 8,
        ..Default::default()
    };
    let result = decode_uyvy(enc, &profile);
    check(enc, expected, 2, 2, &result, "uyvy/solid_white_2x2");
}

#[test]
fn golden_uyvy_interlaced_4x4() {
    let enc = include_bytes!("../../tests/golden/uyvy/interlaced_4x4.enc");
    let expected = include_bytes!("../../tests/golden/uyvy/interlaced_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Yuv422,
        frame_byte_length: 32,
        is_interlaced: true,
        ..Default::default()
    };
    let result = decode_uyvy(enc, &profile);
    check(enc, expected, 4, 4, &result, "uyvy/interlaced_4x4");
}

// ---------------------------------------------------------------------------
// YCbCr 4:2:0
// ---------------------------------------------------------------------------

#[test]
fn golden_ycbcr420_solid_white_4x4() {
    let enc = include_bytes!("../../tests/golden/ycbcr420/solid_white_4x4.enc");
    let expected = include_bytes!("../../tests/golden/ycbcr420/solid_white_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: 24,
        ..Default::default()
    };
    let result = decode_ycbcr420(enc, &profile);
    check(enc, expected, 4, 4, &result, "ycbcr420/solid_white_4x4");
}

#[test]
fn golden_ycbcr420_gradient_4x4() {
    let enc = include_bytes!("../../tests/golden/ycbcr420/gradient_4x4.enc");
    let expected = include_bytes!("../../tests/golden/ycbcr420/gradient_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Ycbcr420,
        frame_byte_length: 24,
        ..Default::default()
    };
    let result = decode_ycbcr420(enc, &profile);
    check(enc, expected, 4, 4, &result, "ycbcr420/gradient_4x4");
}

// ---------------------------------------------------------------------------
// CL (per-pixel nibble chroma)
// ---------------------------------------------------------------------------

#[test]
fn golden_cl_solid_white_4x4() {
    let enc = include_bytes!("../../tests/golden/cl/solid_white_4x4.enc");
    let expected = include_bytes!("../../tests/golden/cl/solid_white_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Yuv422,
        frame_byte_length: 32,
        cl_chroma: true,
        ..Default::default()
    };
    let result = decode_cl(enc, &profile);
    check(enc, expected, 4, 4, &result, "cl/solid_white_4x4");
}

// ---------------------------------------------------------------------------
// CLCL (shared-nibble chroma)
// ---------------------------------------------------------------------------

#[test]
fn golden_clcl_solid_white_4x4() {
    let enc = include_bytes!("../../tests/golden/clcl/solid_white_4x4.enc");
    let expected = include_bytes!("../../tests/golden/clcl/solid_white_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::Yuv422,
        frame_byte_length: 32,
        clcl_chroma: true,
        ..Default::default()
    };
    let result = decode_clcl(enc, &profile);
    check(enc, expected, 4, 4, &result, "clcl/solid_white_4x4");
}

// ---------------------------------------------------------------------------
// JPEG
// ---------------------------------------------------------------------------

#[test]
fn golden_jpeg_solid_white_2x2() {
    let enc = include_bytes!("../../tests/golden/jpeg/solid_white_2x2.enc");
    let expected = include_bytes!("../../tests/golden/jpeg/solid_white_2x2.bin");
    let profile = Profile {
        encoding: Encoding::Jpeg,
        use_mhni_dimensions: true,
        ..Default::default()
    };
    let result = decode_jpeg(enc, &profile);
    check(enc, expected, 2, 2, &result, "jpeg/solid_white_2x2");
}
