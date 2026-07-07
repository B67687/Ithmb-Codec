//! Golden-vector comparison tests.
//!
//! Each test reads a raw `.enc` input (produced by the C# reference encoder),
//! decodes it with the Rust decoder, and compares the BGRA output against the
//! reference `.bin` file — proving bit-exact compatibility.

use divan as _;
use jpeg_decoder as _;
use thiserror as _;

use ithmb_core::DecodedImage;
use ithmb_core::profile::{Encoding, Profile};
#[cfg(feature = "cache")]
use lru as _;
use std::sync::atomic::AtomicBool;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn decode_rgb(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::rgb565::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_rgb555(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::rgb555::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_uyvy(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::uyvy::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_ycbcr420(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::ycbcr420::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_cl(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::cl::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_clcl(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::clcl::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_jpeg(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::jpeg::decode(enc, profile, &AtomicBool::new(false)).unwrap()
}

fn decode_reordered_rgb555(enc: &[u8], profile: &Profile) -> DecodedImage {
    ithmb_core::reordered_rgb555::decode(enc, profile, &AtomicBool::new(false)).unwrap()
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
    let enc = include_bytes!("fixtures/rgb565_solid_white_2x2.enc");
    let expected = include_bytes!("fixtures/rgb565_solid_white_2x2.bin");
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
    let enc = include_bytes!("fixtures/rgb565_solid_red_2x2.enc");
    let expected = include_bytes!("fixtures/rgb565_solid_red_2x2.bin");
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
    let enc = include_bytes!("fixtures/rgb565_gradient_4x4.enc");
    let expected = include_bytes!("fixtures/rgb565_gradient_4x4.bin");
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
    let enc = include_bytes!("fixtures/rgb555_solid_white_2x2.enc");
    let expected = include_bytes!("fixtures/rgb555_solid_white_2x2.bin");
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
    let enc = include_bytes!("fixtures/rgb555_gradient_4x4.enc");
    let expected = include_bytes!("fixtures/rgb555_gradient_4x4.bin");
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
    let enc = include_bytes!("fixtures/uyvy_solid_white_2x2.enc");
    let expected = include_bytes!("fixtures/uyvy_solid_white_2x2.bin");
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
    let enc = include_bytes!("fixtures/uyvy_interlaced_4x4.enc");
    let expected = include_bytes!("fixtures/uyvy_interlaced_4x4.bin");
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
    let enc = include_bytes!("fixtures/ycbcr420_solid_white_4x4.enc");
    let expected = include_bytes!("fixtures/ycbcr420_solid_white_4x4.bin");
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
    let enc = include_bytes!("fixtures/ycbcr420_gradient_4x4.enc");
    let expected = include_bytes!("fixtures/ycbcr420_gradient_4x4.bin");
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
    let enc = include_bytes!("fixtures/cl_solid_white_4x4.enc");
    let expected = include_bytes!("fixtures/cl_solid_white_4x4.bin");
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
    let enc = include_bytes!("fixtures/clcl_solid_white_4x4.enc");
    let expected = include_bytes!("fixtures/clcl_solid_white_4x4.bin");
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
    let enc = include_bytes!("fixtures/jpeg_solid_white_2x2.enc");
    let expected = include_bytes!("fixtures/jpeg_solid_white_2x2.bin");
    let profile = Profile {
        encoding: Encoding::Jpeg,
        use_mhni_dimensions: true,
        ..Default::default()
    };
    let result = decode_jpeg(enc, &profile);
    check(enc, expected, 2, 2, &result, "jpeg/solid_white_2x2");
}

// ---------------------------------------------------------------------------
// Reordered RGB555  —  big-endian Z-order interleaved
// ---------------------------------------------------------------------------

#[test]
fn golden_reordered_rgb555_solid_white_2x2() {
    let enc = include_bytes!("fixtures/reordered_rgb555_solid_white_2x2.enc");
    let expected = include_bytes!("fixtures/reordered_rgb555_solid_white_2x2.bin");
    let profile = Profile {
        width: 2,
        height: 2,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: 8,
        little_endian: false,
        ..Default::default()
    };
    let result = decode_reordered_rgb555(enc, &profile);
    check(enc, expected, 2, 2, &result, "reordered_rgb555/solid_white_2x2");
}

#[test]
fn golden_reordered_rgb555_gradient_4x4() {
    let enc = include_bytes!("fixtures/reordered_rgb555_gradient_4x4.enc");
    let expected = include_bytes!("fixtures/reordered_rgb555_gradient_4x4.bin");
    let profile = Profile {
        width: 4,
        height: 4,
        encoding: Encoding::ReorderedRgb555,
        frame_byte_length: 32,
        little_endian: false,
        ..Default::default()
    };
    let result = decode_reordered_rgb555(enc, &profile);
    check(enc, expected, 4, 4, &result, "reordered_rgb555/gradient_4x4");
}
// ---------------------------------------------------------------------------
// Reuhno synthetic golden vectors — iPod Classic 6G (F1055, F1060, F1061)
// ---------------------------------------------------------------------------

static F1055_ITHMB: &[u8] = include_bytes!("fixtures/golden/F1055_1.ithmb");
static F1060_ITHMB: &[u8] = include_bytes!("fixtures/golden/F1060_1.ithmb");
static F1061_ITHMB: &[u8] = include_bytes!("fixtures/golden/F1061_1.ithmb");

fn frame_with_prefix(prefix: i32, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(data.len() + 4);
    v.extend_from_slice(&prefix.to_be_bytes());
    v.extend_from_slice(data);
    v
}

fn assert_bgra_matches_rgba(bgra: &[u8], rgba: &[u8], label: &str) {
    assert_eq!(bgra.len(), rgba.len(), "{label}: length mismatch");
    for (i, (bgra_chunk, rgba_chunk)) in bgra.chunks_exact(4).zip(rgba.chunks_exact(4)).enumerate() {
        assert_eq!(bgra_chunk[0], rgba_chunk[2], "{label}: B mismatch at pixel {i}");
        assert_eq!(bgra_chunk[1], rgba_chunk[1], "{label}: G mismatch at pixel {i}");
        assert_eq!(bgra_chunk[2], rgba_chunk[0], "{label}: R mismatch at pixel {i}");
        assert_eq!(bgra_chunk[3], rgba_chunk[3], "{label}: A mismatch at pixel {i}");
    }
}
#[test]
fn golden_f1061_frame0_off0() {
    let frame_data = &F1061_ITHMB[0..6160];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off0_decl55x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame0_off0: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame0_off0: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame0_off0");
}

#[test]
fn golden_f1055_frame0_off0() {
    let frame_data = &F1055_ITHMB[0..32_768];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off0_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame0_off0: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame0_off0: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame0_off0");
}

#[test]
fn golden_f1060_frame0_off0() {
    let frame_data = &F1060_ITHMB[0..204_800];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off0_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame0_off0: width mismatch");
    assert_eq!(decoded.height, 320u32, "golden_f1060_frame0_off0: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame0_off0");
}

#[test]
fn golden_f1061_frame1_off6160() {
    let frame_data = &F1061_ITHMB[6160..12_320];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off6160_decl55x54_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame1_off6160: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame1_off6160: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame1_off6160");
}

#[test]
fn golden_f1055_frame1_off32768() {
    let frame_data = &F1055_ITHMB[32_768..65_536];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off32768_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame1_off32768: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame1_off32768: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame1_off32768");
}

#[test]
fn golden_f1060_frame1_off204800() {
    let frame_data = &F1060_ITHMB[204_800..409_600];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off204800_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame1_off204800: width mismatch");
    assert_eq!(decoded.height, 320u32, "golden_f1060_frame1_off204800: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame1_off204800");
}

#[test]
fn golden_f1061_frame2_off12320() {
    let frame_data = &F1061_ITHMB[12_320..18_480];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off12320_decl55x52_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame2_off12320: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame2_off12320: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame2_off12320");
}

#[test]
fn golden_f1055_frame2_off65536() {
    let frame_data = &F1055_ITHMB[65_536..98_304];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off65536_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame2_off65536: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame2_off65536: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame2_off65536");
}

#[test]
fn golden_f1060_frame2_off409600() {
    let frame_data = &F1060_ITHMB[409_600..614_400];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off409600_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame2_off409600: width mismatch");
    assert_eq!(decoded.height, 320u32, "golden_f1060_frame2_off409600: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame2_off409600");
}

#[test]
fn golden_f1061_frame3_off18480() {
    let frame_data = &F1061_ITHMB[18_480..24_640];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off18480_decl54x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame3_off18480: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame3_off18480: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame3_off18480");
}

#[test]
fn golden_f1055_frame3_off98304() {
    let frame_data = &F1055_ITHMB[98_304..131_072];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off98304_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame3_off98304: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame3_off98304: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame3_off98304");
}

#[test]
fn golden_f1060_frame3_off614400() {
    let frame_data = &F1060_ITHMB[614_400..819_200];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off614400_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame3_off614400: width mismatch");
    assert_eq!(decoded.height, 320u32, "golden_f1060_frame3_off614400: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame3_off614400");
}

#[test]
fn golden_f1061_frame4_off24640() {
    let frame_data = &F1061_ITHMB[24_640..30_800];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off24640_decl48x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame4_off24640: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame4_off24640: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame4_off24640");
}

#[test]
fn golden_f1055_frame4_off131072() {
    let frame_data = &F1055_ITHMB[131_072..163_840];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off131072_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame4_off131072: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame4_off131072: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame4_off131072");
}

#[test]
fn golden_f1060_frame4_off819200() {
    let frame_data = &F1060_ITHMB[819_200..1_024_000];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off819200_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame4_off819200: width mismatch");
    assert_eq!(decoded.height, 320u32, "golden_f1060_frame4_off819200: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame4_off819200");
}

#[test]
fn golden_f1061_frame5_off30800() {
    let frame_data = &F1061_ITHMB[30_800..36_960];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off30800_decl43x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame5_off30800: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame5_off30800: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame5_off30800");
}

#[test]
fn golden_f1055_frame5_off163840() {
    let frame_data = &F1055_ITHMB[163_840..196_608];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off163840_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame5_off163840: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame5_off163840: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame5_off163840");
}

#[test]
fn golden_f1060_frame5_off1024000() {
    let frame_data = &F1060_ITHMB[1_024_000..1_228_800];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off1024000_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame5_off1024000: width mismatch");
    assert_eq!(
        decoded.height, 320u32,
        "golden_f1060_frame5_off1024000: height mismatch"
    );
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame5_off1024000");
}

#[test]
fn golden_f1061_frame6_off36960() {
    let frame_data = &F1061_ITHMB[36_960..43_120];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off36960_decl55x48_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame6_off36960: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame6_off36960: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame6_off36960");
}

#[test]
fn golden_f1055_frame6_off196608() {
    let frame_data = &F1055_ITHMB[196_608..229_376];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off196608_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame6_off196608: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame6_off196608: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame6_off196608");
}

#[test]
fn golden_f1060_frame6_off1228800() {
    let frame_data = &F1060_ITHMB[1_228_800..1_433_600];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off1228800_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame6_off1228800: width mismatch");
    assert_eq!(
        decoded.height, 320u32,
        "golden_f1060_frame6_off1228800: height mismatch"
    );
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame6_off1228800");
}

#[test]
fn golden_f1061_frame7_off43120() {
    let frame_data = &F1061_ITHMB[43_120..49_280];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off43120_decl44x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame7_off43120: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame7_off43120: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame7_off43120");
}

#[test]
fn golden_f1055_frame7_off229376() {
    let frame_data = &F1055_ITHMB[229_376..262_144];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off229376_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame7_off229376: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame7_off229376: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame7_off229376");
}

#[test]
fn golden_f1060_frame7_off1433600() {
    let frame_data = &F1060_ITHMB[1_433_600..1_638_400];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off1433600_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame7_off1433600: width mismatch");
    assert_eq!(
        decoded.height, 320u32,
        "golden_f1060_frame7_off1433600: height mismatch"
    );
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame7_off1433600");
}

#[test]
fn golden_f1061_frame8_off49280() {
    let frame_data = &F1061_ITHMB[49_280..55_440];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off49280_decl52x55_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame8_off49280: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame8_off49280: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame8_off49280");
}

#[test]
fn golden_f1055_frame8_off262144() {
    let frame_data = &F1055_ITHMB[262_144..294_912];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off262144_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame8_off262144: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame8_off262144: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame8_off262144");
}

#[test]
fn golden_f1060_frame8_off1638400() {
    let frame_data = &F1060_ITHMB[1_638_400..1_843_200];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off1638400_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame8_off1638400: width mismatch");
    assert_eq!(
        decoded.height, 320u32,
        "golden_f1060_frame8_off1638400: height mismatch"
    );
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame8_off1638400");
}

#[test]
fn golden_f1061_frame9_off55440() {
    let frame_data = &F1061_ITHMB[55_440..61_600];
    let prefixed = frame_with_prefix(1061, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1061).unwrap().clone();
    profile.width = 56;
    profile.height = 55;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1061_1_off55440_decl55x50_slot56x55.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 56u32, "golden_f1061_frame9_off55440: width mismatch");
    assert_eq!(decoded.height, 55u32, "golden_f1061_frame9_off55440: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1061_frame9_off55440");
}

#[test]
fn golden_f1055_frame9_off294912() {
    let frame_data = &F1055_ITHMB[294_912..327_680];
    let prefixed = frame_with_prefix(1055, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1055).unwrap().clone();
    profile.width = 128;
    profile.height = 128;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1055_1_off294912_decl128x128_slot128x128.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 128u32, "golden_f1055_frame9_off294912: width mismatch");
    assert_eq!(decoded.height, 128u32, "golden_f1055_frame9_off294912: height mismatch");
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1055_frame9_off294912");
}

#[test]
fn golden_f1060_frame9_off1843200() {
    let frame_data = &F1060_ITHMB[1_843_200..2_048_000];
    let prefixed = frame_with_prefix(1060, frame_data);

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().unwrap();
    let mut profile = db.get(1060).unwrap().clone();
    profile.width = 320;
    profile.height = 320;

    let decoded =
        ithmb_core::pipeline::decode_with_profile(&prefixed, &profile, &std::sync::atomic::AtomicBool::new(false))
            .unwrap();

    let png_bytes = include_bytes!("fixtures/golden/F1060_1_off1843200_decl320x320_slot320x320.png");
    let png = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    let expected = png.as_raw();

    assert_eq!(decoded.width, 320u32, "golden_f1060_frame9_off1843200: width mismatch");
    assert_eq!(
        decoded.height, 320u32,
        "golden_f1060_frame9_off1843200: height mismatch"
    );
    assert_bgra_matches_rgba(&decoded.data, expected, "golden_f1060_frame9_off1843200");
}
