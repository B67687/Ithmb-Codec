# Ithmb-Codec Standards

This file documents project-specific engineering standards.
See [`RUST_STANDARDS.md`](RUST_STANDARDS.md) for general Rust engineering practices,
lint configuration, unsafe code policy, error handling, and SIMD architecture.

---

## Automation

| Tier | Item | Status |
|------|------|--------|
| 0 | CI build + test | ✅ `cargo build --workspace`, `cargo test --workspace` on push/PR to main |
| 0 | Static analysis (lints-as-errors) | ✅ `[workspace.lints.clippy]` — `all = "deny"`, `pedantic = "deny"` in root Cargo.toml; `cargo clippy -- -D warnings` in CI |
| 0 | Signed commits | ✅ All commits signed with SSH (`commit.gpgsign=true`). Verified via `git log --show-signature`. |
| 0 | Reproducible builds | ✅ Cargo.lock committed; workspace version `0.3.0` |
| 0 | CHANGELOG | ✅ Keep a Changelog format, `[Unreleased]` header present |
| 1 | Conventional commits | ✅ Manually enforced (not CI-gated) |
| 1 | Formatter enforcement | ✅ `cargo fmt --check` in CI |
| 1 | EditorConfig | ✅ `.editorconfig` with LF, UTF-8, 4-space indent |
| 1 | Toolchain pinning | ✅ `rust-toolchain.toml` with stable channel, clippy + rustfmt components |
| 1 | Concurrency-safe state | ✅ `RwLock<LruCache>` for cache, `AtomicBool` for cancellation |
| 2 | Benchmarks | ✅ `ithmb-core/benches/` — 4 Divan benchmarks (decoders, encoders, pipeline) |
| 2 | Fuzz testing | ✅ `fuzz/` — 2 libfuzzer targets, CI fuzz build check, 1.2M+ iterations, 0 crashes |
| 2 | Golden test vectors | ✅ 14+ reference files across 7 encoding formats |
| 2 | `--features simd` CI | ✅ CI tests with `--features simd` for SIMD code paths |
| 2 | Python bindings CI | ✅ `pymod/` built via maturin/abi3-py312 |
| 2 | File size gate (250 LOC) | 🟡 Script at `tools/check-file-sizes.sh`, not yet wired into CI |
| 2 | C ABI release integrity | ✅ Built in the [plugin repo](https://github.com/B67687/Imageglass-Ithmb-Plugin); `nm` verifies `ig_plugin_get_api` symbol export |
| 2 | Cancellation polling | ✅ `AtomicBool` parameter in all decoder functions |
| 2 | C# cross-verification | ✅ All 7 formats verified pixel-for-pixel against C# oracle during development |
| 2 | Miri unsafety check | ✅ `cargo +nightly miri test --features simd` — 21 SSE2 tests verified, 0 UB |

## Design

| Axiom | Application |
|-------|-------------|
| **Modularity** | 21 modules in ithmb-core. Each decoder in its own file. PhotoDB in its own submodule. Pipeline owns dispatch only. SIMD split into 7 per-format files. |
| **Data Flow** | Unidirectional: pipeline detect → prefix match → per-format decoder → DecodedImage. No back-edges. |
| **Fail-Fast** | Buffer-too-small guards in every decoder. Cancellation polled at macroblock boundaries. |
| **Parse-Don't-Validate** | 54 built-in profiles parsed at compile-time into `ProfileDb`. |
| **Layered Dependencies** | `ithmb-core` → `ithmb-cli`, `pymod`. No cycles. |

## Code Rules

- **Dead code**: Zero. Every function is used or behind `#[cfg(test)]`. Every fallback is `#[cfg(not(...))]`-gated — no `#[allow(unreachable_code)]`.
- **Warnings**: Zero across all build configurations (`default`, `--features simd`, `--all-features`), clippy, doc, and fmt.
- **See `RUST_STANDARDS.md`** for unsafe code policy, error handling, unwrap rules, lint configuration, and SIMD architecture.

## Workflow (Token Efficiency)

1. **Scaffold then fill** — write signatures first, `cargo check`, then fill bodies one at a time.
2. **Pre-verify API sigs** — `grep "pub fn decode_with_profile" src/pipeline.rs` before calling.
3. **Prefer `cargo check` over `cargo test`** — 5s vs 60s. Use test only for running tests.
4. **Use `cargo clippy --fix`** first — handles 60% of pedantic lints automatically.

## Cross-Platform SIMD

This section documents how Ithmb-Codec handles cross-platform SIMD across x86_64, aarch64, and Windows/macOS/Linux. It is based on our implementation experience and best practices from production Rust codecs (memchr, rav1e, image-rs, libyuv).

### Architecture Overview

Our SIMD stack has three layers:

1. **Dispatch functions** (`simd/mod.rs`) — platform-agnostic entry points that detect CPU features at runtime
2. **Platform-specific modules** (`simd/uyvy.rs`, `simd/rgb565.rs`, etc.) — per-format SIMD kernels
3. **Scalar fallbacks** (`simd/scalar.rs`) — portable implementations used when SIMD is unavailable

```
pub fn uyvy_quad_to_bgra(quad: &[u8; 4]) -> [u8; 16] {
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    unsafe { return uyvy::sse2(quad); }

    #[cfg(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))]
    unsafe { return neon::neon_impl(quad); }

    #[cfg(not(all(feature = "simd",
        any(target_arch = "x86_64", target_arch = "x86", all(target_arch = "aarch64", not(target_os = "macos")))
    )))]
    scalar::uyvy_quad_to_bgra(quad)
```


### Platform Coverage

| Platform | Default SIMD | Our Dispatch |
|----------|-------------|--------------|
| x86_64 (Linux, macOS Intel, Windows) | SSE2 guaranteed | AVX2 -> SSE4.1 -> SSE2 -> scalar |
| aarch64 (Linux ARM) | NEON guaranteed | NEON -> scalar |
| aarch64 (macOS ARM) | NEON guaranteed | Scalar (NEON gated — known edge case, see below) |
| x86 (32-bit) | SSE2 | SSE2 -> scalar |
| Other (RISC-V, etc.) | None | scalar only |

### Key Patterns

#### Pattern 1: Always-gated scalar fallbacks

Each dispatch function has a scalar fallback behind a cfg guard:
```rust
#[cfg(not(all(
    feature = "simd",
    any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")
)))]
// Scalar fallback (not needed when SIMD covers all platforms)
scalar::uyvy_quad_to_bgra(quad)
```
This ensures no dead_code or unreachable_code errors across platforms.

#### Pattern 2: Test-only functions for uncalled code

Functions with no production caller on any platform must be gated:
```rust
#[cfg(test)]
pub(crate) unsafe fn rgb565_row_to_bgra_neon(...)
```

Scalar functions unused on aarch64+simd in production:
```rust
#[cfg(any(test, not(all(feature = "simd", target_arch = "aarch64"))))]
use crate::yuv;
```

#### Pattern 3: Module-level unsafe_code

SIMD modules use `#![allow(unsafe_code)]` at the module level rather than per-function.
Every unsafe fn must have a // SAFETY: comment documenting the invariant.

#### Pattern 4: Compile-time over runtime dispatch

- On x86_64: `#[cfg(target_arch = "x86_64")]` + runtime `is_x86_feature_detected!` for AVX2 vs SSE4.1
- On aarch64: `#[cfg(target_arch = "aarch64")]` — no runtime detection needed (NEON guaranteed by ARMv8)

### What We Fixed (Lessons Learned)

Over the course of development, we discovered and fixed these cross-platform issues:

| Issue | Symptom | Fix |
|-------|---------|-----|
| `core::arch::x86_64` import at module level | Failed to compile on macOS ARM | Gated behind `#[cfg(any(x86_64, x86))]` |
| Scalar functions dead on aarch64+simd | macOS simd CI dead_code error | Added `#[cfg(any(test, not(all(feature = "simd", aarch64))))]` on 6 scalar functions |
| NEON functions never called | Dead code on aarch64+simd | Added `#[cfg(test)]` on 3 uncalled NEON functions |
| unreachable_code after NEON return | macOS simd CI failed | Added cfg guard to scalar fallbacks |
| `use crate::yuv` unused on aarch64+simd | Unused import error | Gated behind same cfg as scalar definitions |
| rust-toolchain.toml not tracked in git | CI used wrong toolchain | Added `!/rust-toolchain.toml` to .gitignore |
| Windows uses PowerShell, not bash | Build script syntax error | Added `shell: bash` to CI steps |
| Cache key with commas in features | Windows CI key validation error | Removed features from cache key |
| pymod links Python on macOS ARM | Linker fails | Added `--exclude ithmb-python` on macOS in CI |
| macOS ARM NEON edge case | `macOS+simd` CI fails on test or runtime | Gated NEON dispatch behind `not(target_os = "macos")`; macOS+simd uses scalar fallback. Known issue tracked in [iOpenPod#140](https://github.com/TheRealSavi/iOpenPod/issues/140) — may be a QEMU or runner quirk rather than codec bug. |

### CI Matrix

We test 6 platform/feature combinations plus clippy verification:

| Job | OS | Features | Why |
|-----|-----|----------|-----|
| build | ubuntu-latest | — | Core compatibility |
| build | ubuntu-latest | simd | x86_64 SIMD coverage |
| build | macos-latest | — | aarch64 ARM coverage |
| build | macos-latest | simd | ARM coverage (scalar fallback — NEON gated, see below) |
| build | windows-latest | — | Windows/ImageGlass support |
| build | windows-latest | simd | Windows SIMD |
| verify_clippy | ubuntu-latest | — | Clippy + audit |

### References

- [Rust core::arch docs](https://doc.rust-lang.org/core/arch/index.html)
- [ARM Rust SIMD Learning Path](https://learn.arm.com/learning-paths/cross-platform/simd-on-rust/)
- [memchr crate](https://github.com/BurntSushi/memchr) — canonical Rust SIMD dispatch
- [Distributing Rust SIMD binaries](https://curiouscoding.nl/posts/distributing-rust-simd-binaries/)
- [State of SIMD in Rust 2025](https://shnatsel.medium.com/the-state-of-simd-in-rust-in-2025-32c263e5f53d)
- [image-png performance](https://blog.image-rs.org/2026/06/18/png-adoption.html)
- [Google Highway](https://github.com/google/highway) — cross-platform SIMD (C++)
- [Safe SIMD in Rust](https://shnatsel.medium.com/safe-simd-in-rust-even-on-the-inside-c6f1ff381828)
