# Ithmb-Codec Standards

This file documents the Rust engineering standards applied in this workspace.

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
| 2 | Benchmarks | ✅ `ithmb-core/benches/` — 3 Divan benchmarks (decoders, encoders, pipeline) |
| 2 | Fuzz testing | ✅ `fuzz/` — 2 libfuzzer targets, CI fuzz build check, 1.2M+ iterations, 0 crashes |
| 2 | Golden test vectors | ✅ 14+ reference files across 7 encoding formats |
| 2 | `--features simd` CI | ✅ CI tests with `--features simd` for SIMD code paths |
| 2 | Python bindings CI | ✅ `pymod/` built via maturin/abi3-py312 |
| 2 | File size gate (250 LOC) | 🟡 Script at `tools/check-file-sizes.sh`, not yet wired into CI |
| 2 | C ABI release integrity | ✅ `cabi/` built in CI; `nm` verifies `ig_plugin_get_api` symbol export |
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
| **Layered Dependencies** | `ithmb-core` → `ithmb-cli`, `ithmb-core-cabi`, `pymod`. No cycles. |

## Code Rules

- **Unsafe**: Only in `simd/` (behind `#![allow(unsafe_code)]`) and `cabi/` (FFI). Denied at workspace level (`unsafe_code = "warn"`).
- **Errors**: `DecodeError` enum with typed variants. Never `Box<dyn Error>`. `?` operator throughout.
- **Unwrap/Expect**: None in production code. Only in tests and `fn main()`.
- **Dead code**: Zero. Every function is used or behind `#[cfg(test)]`. Every fallback is `#[cfg(not(...))]`-gated — no `#[allow(unreachable_code)]`.
- **Warnings**: Zero across all build configurations (`default`, `--features simd`, `--all-features`), clippy, doc, and fmt.

## Workflow (Token Efficiency)

1. **Scaffold then fill** — write signatures first, `cargo check`, then fill bodies one at a time.
2. **Pre-verify API sigs** — `grep "pub fn decode_with_profile" src/pipeline.rs` before calling.
3. **Prefer `cargo check` over `cargo test`** — 5s vs 60s. Use test only for running tests.
4. **Use `cargo clippy --fix`** first — handles 60% of pedantic lints automatically.
