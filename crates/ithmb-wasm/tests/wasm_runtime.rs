#![cfg(target_arch = "wasm32")] // these tests only run under wasm-pack (node/browser)
#![allow(unused_crate_dependencies)] // deps used under wasm only; native builds skip this file's bodies

//! Runtime smoke tests for the WASM bindings.
//!
//! These instantiate the compiled wasm and call the exported functions,
//! verifying that the bindings actually work at runtime (not just compile).
//! Run with: `wasm-pack test --node` or `wasm-pack test --headless --chrome`.

use ithmb_core as _; // mark used under deny(unused_crate_dependencies)
use ithmb_wasm::{decode_ithmb, get_encoding_name, peek_prefix};
use wasm_bindgen as _; // mark used under deny(unused_crate_dependencies)
use wasm_bindgen_test as _; // mark used under deny(unused_crate_dependencies)
use wasm_bindgen_test::*;

/// The synthetic RGB565 sample from `samples/synthetic/sample.ithmb`.
/// Embedded as bytes so the test runs standalone in any wasm runner.
const SAMPLE_ITHMB: &[u8] = include_bytes!("../../../samples/synthetic/sample.ithmb");

#[wasm_bindgen_test]
fn decode_ithmb_returns_image_metadata() {
    let out = decode_ithmb(SAMPLE_ITHMB).expect("sample should decode");
    // Layout: [width u32 LE][height u32 LE][RGBA pixels ...]
    assert!(out.len() >= 8, "output too small: {} bytes", out.len());
    let width = u32::from_le_bytes([out[0], out[1], out[2], out[3]]);
    let height = u32::from_le_bytes([out[4], out[5], out[6], out[7]]);
    assert!(width > 0 && height > 0, "expected nonzero dims, got {width}x{height}");
    assert_eq!(
        out.len(),
        8 + (width * height * 4) as usize,
        "pixel buffer length must match declared dimensions"
    );
}

#[wasm_bindgen_test]
fn decode_ithmb_produces_nonzero_pixels() {
    let out = decode_ithmb(SAMPLE_ITHMB).expect("sample should decode");
    let pixel_data = &out[8..];
    // The sample is a generated pattern — it must not be all-zero or all-one-color.
    let mut colors = std::collections::HashSet::new();
    for chunk in pixel_data.chunks_exact(4).take(4096) {
        colors.insert([chunk[0], chunk[1], chunk[2]]);
    }
    assert!(
        colors.len() >= 2,
        "expected a varied pattern, got {} distinct colors",
        colors.len()
    );
}

#[wasm_bindgen_test]
fn peek_prefix_reads_be_u32() {
    let prefix = peek_prefix(SAMPLE_ITHMB);
    assert!(prefix > 0, "sample should have a nonzero format prefix");
}

#[wasm_bindgen_test]
fn peek_prefix_short_slice_returns_zero() {
    assert_eq!(peek_prefix(&[0x00, 0x01]), 0, "short slice must yield 0");
}

#[wasm_bindgen_test]
fn get_encoding_name_known_prefix() {
    // RGB565 (or whichever prefix the sample uses) must have a non-"Unknown" name.
    let prefix = peek_prefix(SAMPLE_ITHMB);
    let name = get_encoding_name(prefix);
    assert!(
        !name.contains("Unknown"),
        "prefix {prefix} should have a known encoding name, got: {name}"
    );
}

#[wasm_bindgen_test]
fn decode_garbage_returns_none() {
    let garbage = [0xFFu8; 64];
    assert!(decode_ithmb(&garbage).is_none(), "garbage must not decode");
}
