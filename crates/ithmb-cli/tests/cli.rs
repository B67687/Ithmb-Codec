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

/// Path to a golden multi-frame F-file fixture (prefix-less raw payload).
fn golden_f_file(name: &str) -> PathBuf {
    workspace_root()
        .join("crates/ithmb-core/tests/fixtures/golden")
        .join(name)
}

/// Build a prefixed two-frame 320×240 RGB565 buffer using the crate's encoder.
fn two_frame_prefixed_buffer() -> Vec<u8> {
    use ithmb_core::enc::encode_bgra;

    let db = ithmb_core::ProfileDb::load_builtin().unwrap();
    let profile = db.get(1024).unwrap().clone();
    let bgra: Vec<u8> = (0..320 * 240 * 4).map(|i| (i % 256) as u8).collect();
    let frame0 = encode_bgra(&bgra, 320, 240, &profile);
    let frame1 = encode_bgra(&bgra, 320, 240, &profile);
    let mut buf = 1024_u32.to_be_bytes().to_vec();
    buf.extend_from_slice(&frame0);
    buf.extend_from_slice(&frame1);
    buf
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
    assert!(stdout.contains("--frame-count"), "help missing --frame-count");
    assert!(stdout.contains("--extract-all"), "help missing --extract-all");
}

#[test]
fn version_flag() {
    let out = run_ithmb(&["--version"]);
    assert_ok(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Assert a semantic version (X.Y.Z) is printed rather than a hardcoded
    // prefix that breaks on every bump (e.g. 1.9 -> 1.10).
    let has_semver = stdout
        .chars()
        .zip(stdout.chars().skip(1))
        .any(|(a, b)| a.is_ascii_digit() && b == '.');
    assert!(has_semver, "expected semantic version, got: {stdout}");
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

// ---------------------------------------------------------------------------
// --frame-count and --extract-all
// ---------------------------------------------------------------------------

#[test]
fn frame_count_on_single_frame_sample() {
    let sample = sample_ithmb();
    assert!(sample.exists(), "sample file not found: {sample:?}");

    let out = run_ithmb(&["--frame-count", &sample.to_string_lossy()]);
    assert_ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
}

#[test]
fn frame_count_on_multi_frame_f_file() {
    let fixture = golden_f_file("F1061_1.ithmb");
    assert!(fixture.exists(), "fixture not found: {fixture:?}");

    let out = run_ithmb(&["--frame-count", &fixture.to_string_lossy()]);
    assert_ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10");
}

#[test]
fn frame_count_on_multi_frame_prefixed_file() {
    let dir = tmp_dir("frame_count_prefixed");
    let input = dir.join("two_frames.ithmb");
    std::fs::write(&input, two_frame_prefixed_buffer()).unwrap();

    let out = run_ithmb(&["--frame-count", &input.to_string_lossy()]);
    assert_ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "2");
}

#[test]
fn frame_count_unknown_format_errors() {
    let dir = tmp_dir("frame_count_unknown");
    let input = dir.join("mystery.ithmb");
    std::fs::write(&input, [0x12u8, 0x34, 0x56, 0x78, 0x00]).unwrap();

    let out = run_ithmb(&["--frame-count", &input.to_string_lossy()]);
    assert!(!out.status.success(), "expected failure for unknown format");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot determine format"), "got: {stderr}");
}

#[test]
fn extract_all_on_multi_frame_f_file() {
    let dir = tmp_dir("extract_all_f");
    let input = dir.join("F1061_1.ithmb");
    let fixture = golden_f_file("F1061_1.ithmb");
    std::fs::copy(&fixture, &input).unwrap();

    let out = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&out);

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Wrote"), "expected per-file progress");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Extracted 10 frame(s)"), "got: {stderr}");

    let fixture_data = std::fs::read(&fixture).unwrap();
    for i in 0..10 {
        let path = dir.join("F1061_1").join(format!("F1061_1_{:04}.ithmb", i + 1));
        assert!(path.exists(), "missing extracted file: {path:?}");
        let bytes = std::fs::read(&path).unwrap();
        // Each extracted file = 1061 format-id prefix + that frame's payload.
        let mut expected = 1061_u32.to_be_bytes().to_vec();
        expected.extend_from_slice(&fixture_data[i * 6160..(i + 1) * 6160]);
        assert_eq!(bytes, expected, "frame {i} content mismatch");
    }
}

#[test]
fn extract_all_on_prefixed_multi_frame() {
    let dir = tmp_dir("extract_all_prefixed");
    let input = dir.join("two_frames.ithmb");
    let buffer = two_frame_prefixed_buffer();
    std::fs::write(&input, &buffer).unwrap();

    let out = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&out);

    for i in 0..2 {
        let path = dir.join("two_frames").join(format!("two_frames_{:04}.ithmb", i + 1));
        assert!(path.exists(), "missing extracted file: {path:?}");
    }
    // An extracted frame is a valid standalone .ithmb file: it decodes.
    let first = dir.join("two_frames").join("two_frames_0001.ithmb");
    let png_out = dir.join("first.png");
    let dec = run_ithmb(&[&first.to_string_lossy(), &png_out.to_string_lossy()]);
    assert_ok(&dec);
    assert!(png_out.exists(), "extracted frame should decode to PNG");
}

#[test]
fn extract_all_on_single_frame_sample() {
    let dir = tmp_dir("extract_all_single");
    let input = dir.join("sample.ithmb");
    let sample = sample_ithmb();
    std::fs::copy(&sample, &input).unwrap();

    let out = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&out);

    // Single-frame prefixed file: one output that round-trips byte-for-byte.
    let path = dir.join("sample").join("sample_0001.ithmb");
    assert!(path.exists(), "missing extracted file: {path:?}");
    let original = std::fs::read(&sample).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Extracted 1 frame(s)"), "got: {stderr}");
}

#[test]
fn extract_all_overwrites_existing_outputs() {
    let dir = tmp_dir("extract_all_overwrite");
    let input = dir.join("F1061_1.ithmb");
    std::fs::copy(golden_f_file("F1061_1.ithmb"), &input).unwrap();

    let out1 = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&out1);
    let out2 = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&out2);

    let fixture = std::fs::read(golden_f_file("F1061_1.ithmb")).unwrap();
    let path = dir.join("F1061_1").join("F1061_1_0001.ithmb");
    assert!(path.exists());
    let mut expected = 1061_u32.to_be_bytes().to_vec();
    expected.extend_from_slice(&fixture[0..6160]);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        expected,
        "overwrite must rewrite content"
    );
    // Exactly the 10 frame files — no duplicates from the second run.
    let count = std::fs::read_dir(dir.join("F1061_1")).unwrap().count();
    assert_eq!(count, 10);
}

#[test]
fn frame_count_and_extract_all_on_short_payload() {
    // Profile 1067 (YCbCr420) declares 691200 bytes/frame, but real 720×480 4:2:0
    // payloads are 518400 bytes (1.5 bytes/pixel). Both commands must treat the
    // file as a single frame.
    let dir = tmp_dir("short_payload");
    let input = dir.join("ycbcr420.ithmb");
    let mut buf = vec![0u8; 518_404];
    buf[..4].copy_from_slice(&1067_u32.to_be_bytes());
    std::fs::write(&input, &buf).unwrap();

    let out = run_ithmb(&["--frame-count", &input.to_string_lossy()]);
    assert_ok(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");

    let ex = run_ithmb(&["--extract-all", &input.to_string_lossy()]);
    assert_ok(&ex);
    let path = dir.join("ycbcr420").join("ycbcr420_0001.ithmb");
    assert!(path.exists(), "missing extracted file: {path:?}");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        buf,
        "short-payload extraction must round-trip"
    );
}
