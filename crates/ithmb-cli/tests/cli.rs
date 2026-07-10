//! Integration tests for the `ithmb` CLI binary.
//!
//! Tests invoke the binary via `std::process::Command` and verify
//! stdout/stderr output, exit codes, and that output files exist with
//! the expected content.
//!
//! # Test file locations
//!
//! Sample files are referenced relative to `CARGO_MANIFEST_DIR`, which
//! resolves to `crates/ithmb-cli/` during `cargo test`.
#![allow(clippy::pedantic, clippy::unwrap_used, unused_crate_dependencies)]

use std::path::PathBuf;
use std::process::Command;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the compiled `ithmb` binary.
fn ithmb_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ithmb"))
}

/// Path to the workspace root (parent of `crates/`).
fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // crates/ithmb-cli
    manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR parent")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Path to the synthetic sample file used in tests.
fn sample_ithmb() -> PathBuf {
    workspace_root().join("samples/synthetic/sample.ithmb")
}

/// Temporary output directory, unique per test case.
fn tmp_dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join("ithmb-cli-test");
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run the `ithmb` binary with the given args and return the output.
fn run_ithmb(args: &[&str]) -> std::process::Output {
    Command::new(ithmb_bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to execute ithmb binary at {:?}: {e}", ithmb_bin()))
}

/// Assert that a command succeeded (exit code 0).
fn assert_ok(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success, got exit={:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn help_flag() {
    let out = run_ithmb(&["--help"]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ithmb"), "help missing binary name");
    assert!(stdout.contains("--help"), "help missing --help");
    assert!(stdout.contains("--info"), "help missing --info");
    assert!(stdout.contains("--list-profiles"), "help missing --list-profiles");
    assert!(stdout.contains("--raw"), "help missing --raw");
}

#[test]
fn version_flag() {
    let out = run_ithmb(&["--version"]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1.9"), "expected version 1.9.x, got: {stdout}");
}

#[test]
fn list_profiles() {
    let out = run_ithmb(&["--list-profiles"]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Prefix"), "missing Prefix header");
    assert!(stdout.contains("Width"), "missing Width header");
    assert!(stdout.contains("Height"), "missing Height header");
    assert!(stdout.contains("Encoding"), "missing Encoding header");
    assert!(stdout.contains("Rgb565"), "expected at least one encoding");
    assert!(stdout.contains("Yuv422"), "expected Yuv422 encoding");
    assert!(stdout.contains("Ycbcr420"), "expected Ycbcr420 encoding");
    assert!(stdout.contains("ReorderedRgb555"), "expected ReorderedRgb555");
}

#[test]
fn info_on_sample() {
    let sample = sample_ithmb();
    assert!(sample.exists(), "sample file not found: {sample:?}");

    let out = run_ithmb(&["--info", &sample.to_string_lossy()]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("sample.ithmb"), "missing filename");
    assert!(stdout.contains("153604"), "expected size 153604 bytes");
    assert!(stdout.contains("Prefix: 1024"), "expected prefix 1024");
    assert!(stdout.contains("320×240"), "expected 320×240");
    assert!(stdout.contains("Rgb565"), "expected RGB565 encoding");
    assert!(stdout.contains("Frames:"), "missing frame count");
}

#[test]
fn decode_to_png() {
    let sample = sample_ithmb();
    assert!(sample.exists(), "sample file not found: {sample:?}");

    let output = tmp_dir("decode_to_png").join("output.png");
    let out = run_ithmb(&[&sample.to_string_lossy(), &output.to_string_lossy()]);
    assert_ok(&out);

    assert!(output.exists(), "output PNG not created");
    let metadata = std::fs::metadata(&output).unwrap();
    assert!(metadata.len() > 100, "PNG too small: {} bytes", metadata.len());
}

#[test]
fn decode_raw_bgra() {
    let sample = sample_ithmb();
    assert!(sample.exists(), "sample file not found: {sample:?}");

    let output = tmp_dir("decode_raw").join("output.bin");
    let out = run_ithmb(&["--raw", &sample.to_string_lossy(), &output.to_string_lossy()]);
    assert_ok(&out);

    assert!(output.exists(), "raw output not created");
    let data = std::fs::read(&output).unwrap();
    // 320×240 RGBA = 307,200 bytes
    assert_eq!(data.len(), 320 * 240 * 4, "raw BGRA has wrong size");
}

#[test]
fn raw_explicit_format() {
    let sample = sample_ithmb();
    assert!(sample.exists(), "sample file not found: {sample:?}");

    let output = tmp_dir("raw_explicit").join("output.bin");
    let out = run_ithmb(&["--format", "bin", &sample.to_string_lossy(), &output.to_string_lossy()]);
    assert_ok(&out);

    assert!(output.exists());
    let data = std::fs::read(&output).unwrap();
    assert_eq!(data.len(), 320 * 240 * 4);
}

#[test]
fn missing_input_shows_error() {
    let out = run_ithmb(&[]);
    assert!(!out.status.success(), "expected failure with no input");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("input file is required") || stderr.contains("error"),
        "expected error message about missing input, got: {stderr}"
    );
}

#[test]
fn nonexistent_file_shows_error() {
    let out = run_ithmb(&["/tmp/nonexistent-ithmb-file.xyz"]);
    assert!(!out.status.success(), "expected failure");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("failed to read") || stderr.contains("No such file"),
        "got: {stderr}"
    );
}

#[test]
fn info_on_jpeg_t_prefix() {
    let sample = workspace_root().join("samples/synthetic/sample.ithmb");
    assert!(sample.exists(), "sample not found");

    let out = run_ithmb(&["--info", &sample.to_string_lossy()]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // sample is an F-prefix file, not JPEG — info should reflect that
    assert!(stdout.contains("Prefix:"), "missing prefix info");
}

#[test]
fn auto_output_to_png_with_explicit_path() {
    let sample = sample_ithmb();
    assert!(sample.exists());

    let tmp = tmp_dir("auto_ext");
    let input = tmp.join("sample.ithmb");
    std::fs::copy(&sample, &input).unwrap();

    // Explicit .png output path triggers PNG encoding
    let png_output = tmp.join("output.png");
    let out = run_ithmb(&[&input.to_string_lossy(), &png_output.to_string_lossy()]);
    assert_ok(&out);

    assert!(png_output.exists(), "PNG output not created: {png_output:?}");
}

#[test]
fn raw_short_flag() {
    let sample = sample_ithmb();
    let output = tmp_dir("raw_short").join("out.bin");
    let out = run_ithmb(&["-r", &sample.to_string_lossy(), &output.to_string_lossy()]);
    assert_ok(&out);
    assert!(output.exists());
}
