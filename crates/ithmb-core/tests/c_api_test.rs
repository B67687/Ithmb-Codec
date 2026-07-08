//! Integration test for the C API — compiles and runs `test_ithmb.c`.
//!
//! This test is only compiled and run when `--features c` is enabled.
//! It verifies that the cdylib exposes the correct C symbols and that the
//! C integration test program links against it correctly.
#![allow(
    unused_crate_dependencies,
    clippy::uninlined_format_args,
    clippy::unnecessary_debug_formatting,
    clippy::pedantic,
    clippy::unwrap_used
)]

#[cfg(feature = "c")]
#[test]
fn c_api_integration() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");

    // Locate the cdylib. During `cargo test` it may be in target/debug/ (simple)
    // or target/debug/deps/ (hash-suffixed).
    let manifest_path = std::path::Path::new(&manifest_dir);
    let workspace_root = manifest_path
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap(); // workspace root

    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let target_dir = workspace_root.join("target").join(profile);
    let deps_dir = target_dir.join("deps");
    let lib_path = find_cdylib(&target_dir, &deps_dir);
    let lib_dir = lib_path.parent().unwrap().to_path_buf();

    // Paths for the C source and header
    let c_src = manifest_path.join("tests").join("c_api_test").join("test_ithmb.c");
    let include_dir = manifest_path.join("include");

    // Output binary dir (under workspace target to avoid cluttering source tree)
    let out_dir = workspace_root.join("target").join("c_api_test_bin");
    std::fs::create_dir_all(&out_dir).expect("create out_dir");
    let output_bin = out_dir.join("test_ithmb");

    // Compile the C test program, linking against the cdylib
    let cc_output = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&output_bin)
        .arg(&c_src)
        .args(["-I", &include_dir.to_string_lossy()])
        .args(["-L", &lib_dir.to_string_lossy()])
        .args(["-l", "ithmb_core"])
        .output()
        .expect("failed to run cc");

    let cc_stderr = String::from_utf8_lossy(&cc_output.stderr);
    assert!(cc_output.status.success(), "C compilation failed:\n{}", cc_stderr,);

    // Run the C test with LD_LIBRARY_PATH set to find the cdylib at runtime
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
        "C test failed (exit code: {}):\n{}",
        output.status,
        stdout,
    );

    assert!(
        stdout.contains("All C API tests PASSED"),
        "C test did not report all passing:\n{}",
        stdout,
    );
}

/// Find the cdylib in either `target/debug/` or `target/debug/deps/`.
fn find_cdylib(target_dir: &std::path::Path, deps_dir: &std::path::Path) -> std::path::PathBuf {
    let lib_name = if cfg!(target_os = "macos") {
        "libithmb_core.dylib"
    } else if cfg!(target_os = "windows") {
        "ithmb_core.dll"
    } else {
        "libithmb_core.so"
    };

    let simple = target_dir.join(lib_name);
    if simple.exists() {
        return simple;
    }

    // Search deps/ for the hash-named file
    if let Ok(entries) = std::fs::read_dir(deps_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(lib_name) || name_str.starts_with("libithmb_core-") {
                return entry.path();
            }
        }
    }

    panic!("could not find {lib_name} cdylib in {target_dir:?} or {deps_dir:?}")
}
