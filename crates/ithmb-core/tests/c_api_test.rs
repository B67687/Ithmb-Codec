//! Integration test for the C API — compiles and runs `test_ithmb.c`.
//!
//! This test is only compiled and run when `--features c` is enabled.
//! It verifies that the cdylib exposes the correct C symbols and that the
//! C integration test program links against it correctly.
#![allow(unused_crate_dependencies, clippy::uninlined_format_args)]

#[cfg(feature = "c")]
#[test]
fn c_api_integration() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");

    // Locate the cdylib. It lives in <workspace>/target/debug/
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap(); // workspace root

    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let lib_dir = workspace_root.join("target").join(profile);

    // Paths for the C source and header
    let c_src = std::path::Path::new(&manifest_dir)
        .join("tests")
        .join("c_api_test")
        .join("test_ithmb.c");
    let include_dir = std::path::Path::new(&manifest_dir).join("include");

    // Output binary directory
    let out_dir = std::path::Path::new(&manifest_dir)
        .join("..")
        .join("..")
        .join("target")
        .join("c_api_test_bin");
    std::fs::create_dir_all(&out_dir).expect("create out_dir");
    let output_bin = out_dir.join("test_ithmb");

    // Compile the C test program, linking against libithmb_core
    let status = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&output_bin)
        .arg(&c_src)
        .args(["-I", &include_dir.to_string_lossy()])
        .args(["-L", &lib_dir.to_string_lossy()])
        .args(["-l", "ithmb_core"])
        .status()
        .expect("failed to run cc");

    assert!(status.success(), "C compilation failed");

    // Run the C test with LD_LIBRARY_PATH set
    let output = std::process::Command::new(&output_bin)
        .env("LD_LIBRARY_PATH", &*lib_dir.to_string_lossy())
        .output()
        .expect("failed to run test_ithmb");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        eprintln!("C test stderr: {stderr}");
    }

    assert!(
        output.status.success(),
        "C test failed (exit code: {status}):\n{stdout}",
        status = output.status,
        stdout = stdout,
    );

    assert!(
        stdout.contains("All C API tests PASSED"),
        "C test did not report all passing:\n{stdout}",
        stdout = stdout,
    );
}
