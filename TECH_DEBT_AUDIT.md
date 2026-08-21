# TECH_DEBT_AUDIT.md — Ithmb-Codec

**Generated:** 2026-08-21 | **HEAD:** f5e39f5 | **Method:** AI-generated code quality audit (read-only, no build/test)

## Executive Summary

Ithmb-Codec is a well-structured Rust workspace (5 crates, 84 .rs files, ~16.6k LOC source) with strong foundational patterns: `thiserror` error types, SIMD runtime dispatch, 328+ tests, and 53 format profiles. The code is overall clean — no TODO/FIXME markers, no commented-out code blocks, no `println!` in library code.

**Worst areas:** (1) Two god modules that should be split (`pipeline/mod.rs`, `simd/mod.rs`), (2) 10+ sites of duplicated helper functions across modules, (3) 11 production `.unwrap()` calls in SIMD hot paths that are mathematically safe but structurally poor, (4) an encoder module entirely suppressed behind `allow(dead_code)`. **Quick wins** could eliminate ~60% of the debt in under 2 hours.

## Mental Model

Ithmb-Codec decodes proprietary Apple `.ithmb` thumbnail cache files. The core pipeline reads a 4-byte big-endian prefix, looks up one of 53 device-specific profiles, dispatches to one of 6 pixel-format decoders (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CL/CLCL), then applies post-processing (dimension swap, crop, rotation). SIMD acceleration covers x86_64 (SSE2/SSE4.1/AVX2) and aarch64 (NEON) with scalar fallbacks. An encoder module mirrors the decoders for roundtrip but is not yet wired into the public pipeline.

---

## Findings Table

| ID | Category | File:Line | Severity | Effort (h) | Description | Recommendation |
|----|----------|-----------|----------|------------|-------------|----------------|
| D-1 | Duplication | `simd/mod.rs:65-73` ↔ `pixel_utils.rs:14-24` | **High** | 0.5 | `msb_replicate_5()` and `msb_replicate_6()` defined identically in two modules. `simd/mod.rs` copies the exact same bit-manipulation formula. | Remove from `simd/mod.rs`, import from `pixel_utils`. |
| D-2 | Duplication | `rgb565.rs:76-78` | **High** | 0.5 | Scalar fallback in `rgb565::decode()` inlines the same `r5/g6/b5` bit extraction that `simd::unpack_rgb565()` at `simd/mod.rs:56-61` already implements. | Call `simd::unpack_rgb565()` from the scalar fallback path. |
| D-3 | Duplication | `rgb555.rs:74-86`, `reordered_rgb555.rs:98-110` | **High** | 1.0 | RGB555 pixel-unpack logic (swap vs. no-swap bit extraction + MSB replication) is copy-pasted in 3 places: `rgb555.rs`, `reordered_rgb555.rs`, and `simd/mod.rs:78-84`. | Extract `unpack_rgb555_with_swap(raw, swap) -> [u8; 4]` in `pixel_utils.rs`, use everywhere. |
| D-4 | Duplication | `simd/scalar.rs:78-104` ↔ `simd/reordered.rs:232-257` | **Medium** | 0.5 | `rgb555_pack_to_bgra` scalar body is duplicated between `scalar.rs` and `reordered.rs`. | Have `reordered.rs` delegate to `scalar.rs` on non-SIMD platforms. |
| D-5 | Duplication | `enc/cl.rs:20-33` ↔ `enc/clcl.rs:26-67` | **Medium** | 1.0 | BT.601 forward-transform + nibble-quantize pattern is copy-pasted across 4 loops in `enc/clcl.rs` and 1 loop in `enc/cl.rs`. | Extract `fn bgra_to_ycc_nibbles(bgra, offset) -> (u8, u8, u8)` shared helper. |
| D-6 | Duplication | `pipeline/mod.rs:326-337` ↔ `pipeline/mod.rs:345-364` | **Medium** | 0.5 | `apply_post_process()` and `apply_post_process_with_transform()` are near-identical 3-step pipelines (swap → crop → rotate). | Rewrite `apply_post_process` as one-liner delegating to `apply_post_process_with_transform` with default config. |
| D-7 | Duplication | `yuv.rs:40` ↔ `pixel_utils.rs:30` | **Low** | 0.25 | `yuv::clamp()` is a trivial one-line wrapper over `pixel_utils::clamp_u8()`. | Remove `yuv::clamp`, use `pixel_utils::clamp_u8` directly in `yuv_to_bgra`. |
| B-1 | Bloat | `pipeline/mod.rs` (1286 LOC) | **High** | 2.0 | God module combining decode dispatch (6 encodings), 6 public API wrappers, post-processing (crop/rotation), embedded JPEG scanning, and 766 lines of tests. Non-test code: ~520 lines — over 2× the 250 LOC guideline. | Extract: `post_process.rs` (crop/rotation), `jpeg_scan.rs` (embedded JPEG scanner), `dispatch.rs` (decode dispatch). |
| B-2 | Bloat | `simd/mod.rs` (1243 LOC) | **High** | 2.0 | God module with SIMD dispatch for 8 pixel formats, helper functions, and 651 lines of tests. Non-test: ~591 lines. | Each format dispatch (`uyvy_row_to_bgra`, `rgb565_apply_row_to_bgra`, etc.) could move to its respective `simd/*.rs` submodule; `mod.rs` becomes thin re-exports. |
| B-3 | Bloat | `enc/mod.rs:9` | **Medium** | 0.5 | `#![cfg_attr(not(test), allow(dead_code))]` — entire encoder module suppresses dead_code warning because it's not wired into the public pipeline yet. 550+ lines of untested-in-production code. | Wire encoder into a public API (even behind a feature gate) or mark as `pub(crate)` and remove the blanket suppression. |
| B-4 | Bloat | Repeated `#[allow(clippy::cast_possible_truncation)]` | **Low** | 0.5 | 11+ annotations scattered across 6+ files for `w as u32` / `h as u32` casts in every decoder's return value. | Add a module-level `#![allow(clippy::cast_possible_truncation)]` in `pixel_utils.rs` or create a `to_u32()` helper. |
| DC-1 | Dead Code | `simd/mod.rs:402-407`, `simd/mod.rs:451-456` | **Low** | 0.25 | `rgb565_row_to_bgra()` and `rgb555_row_to_bgra()` are allocate-then-call wrappers over the `*_apply_row_to_bgra` variants. Never called by any decoder (they use the in-place `*_apply` variant). | Remove if no external consumers, or make them `pub(crate)` + `#[cfg(test)]`. |
| DC-2 | Dead Code | `simd/mod.rs:47` | **Low** | 0.1 | `CL_NIBBLE_TABLE` constant has `#[allow(dead_code)]` — used only by SIMD code paths that may not be compiled. | Acceptable as platform-conditional code; verify it's referenced when SSE4.1/AVX2 is enabled. |
| DC-3 | Dead Code | `simd/scalar.rs:10,24` | **Low** | 0.1 | `uyvy_quad_to_bgra` and `uyvy_double_quad_to_bgra` in scalar.rs have `#[allow(dead_code)]` — used only on non-x86/aarch64 platforms. | Acceptable for platform-conditional code. No action needed. |
| DC-4 | Dead Code | `clcl.rs:34`, `ycbcr420.rs:24`, `enc/clcl.rs:4`, `enc/ycbcr420.rs:4` | **Low** | 0.25 | `#[allow(unused_imports)]` on `use crate::yuv` — used only by SIMD dispatch, not in scalar-only builds. | Acceptable for conditional compilation. No action needed. |
| EH-1 | Error Handling | `simd/mod.rs:218-267` (11 sites) | **High** | 1.0 | 11 `.try_into().unwrap()` calls in production SIMD dispatch code. Mathematically safe (slices are bounded by loop invariants), but structurally poor — panics on malformed data in a library crate. | Replace with `TryFrom` + error propagation, or use `unsafe { &*(slice.as_ptr() as *const [u8; N]) }` with SAFETY comment, or accept `&[u8; N]` directly. |
| EH-2 | Error Handling | `cache.rs:141,153` | **Medium** | 0.5 | `self.cache.write().expect("cache lock poisoned")` / `.read().expect(...)` in production — RwLock poisoning panics escalate a prior crash. | Use `parking_lot::RwLock` (no poisoning) or return `Result` from `clear()` and `len()`. |
| EH-3 | Error Handling | `simd/mod.rs:8-9` | **Medium** | 0.25 | Module-level `#![allow(unsafe_code, unreachable_code, dead_code)]` — blanket suppression hides real issues. | Narrow each `allow` to specific items that justify it. Remove blanket `dead_code` and `unreachable_code` allows. |
| EH-4 | Error Handling | `pymod/src/lib.rs:118-121` | **Low** | 0.25 | `list_profiles()` uses `let _ = dict.set_item(...)` — silently discards Python errors from 4 consecutive dict insertions. | Add `?` operator to propagate errors. |
| EH-5 | Error Handling | `pipeline/mod.rs:41` | **Low** | 0.25 | `encoding_name_for_prefix()` returns `String` via `to_display_string().to_string()` — forces heap allocation for `&'static str`. | Return `Cow<'static, str>` or `&'static str` + static fallback. |
| IC-1 | Inconsistent | `uyvy.rs` test assertions vs `cl.rs` test assertions | **Low** | 0.5 | Error tests use different assertion styles: `uyvy.rs` and `ycbcr420.rs` use `match ... { Err(...) => {} other => panic!(...) }` while `cl.rs`, `clcl.rs`, `rgb555.rs` use `assert!(matches!(...))`. | Standardize on `assert!(matches!(result, Err(DecodeError::...)))`. |
| IC-2 | Inconsistent | `ithmb-wasm` error model vs other crates | **Low** | 0.5 | WASM `decode_ithmb` returns `Option<Vec<u8>>` — all errors collapsed into `None`. Other crates surface structured `DecodeError` variants. | Consider returning a result struct with error info for WASM consumers. |
| IC-3 | Inconsistent | `pipeline/mod.rs:1192-1193` | **Low** | 0.1 | Test `test_decode_ithmb_prefix_2002_big_endian_rgb565` has duplicate buffer initialization (lines 1192-1193 repeat lines 1190-1191). | Remove duplicate `buf[0..4].copy_from_slice` and `buf[4..].fill(0xFF)`. |
| DC-5 | Dead Code | `pipeline/mod.rs:146` | **Medium** | 0.25 | `decode_with_profile_with_transform()` is `pub` but has zero callers anywhere in the codebase — not called by any test, CLI, WASM, or Python binding. (`decode_ithmb_with_transform` at line 98 IS used by `fuzz/fuzz_targets/fuzz_decode_pipeline.rs`.) | Remove the dead function, or add `#[doc(hidden)]` if it's planned for future use. |

---

## Top 5 Priorities (Impact/Effort Ratio)

| Rank | ID | Finding | Impact | Effort | Rationale |
|------|----|---------|--------|--------|-----------|
| 1 | D-1 | Consolidate `msb_replicate_5/6` | High | 0.5h | Single canonical location eliminates a class of drift bugs; trivial fix. |
| 2 | EH-1 | Eliminate 11 `.try_into().unwrap()` in SIMD paths | High | 1.0h | Prevents panics in library crate; structurally cleaner; affects hot path. |
| 3 | D-2+D-3 | Fix scalar fallback duplication (RGB555/RGB565) | High | 1.5h | Three copy-pasted pixel-unpack implementations → one shared helper. |
| 4 | D-6 | Collapse `apply_post_process` into transform variant | Medium | 0.5h | Eliminates 12 lines of near-duplicate post-processing code. |
| 5 | B-3 | Wire encoder or remove dead_code suppression | Medium | 0.5h | 550+ lines of code with suppressed dead_code warning is a maintenance hazard. |

**Total for top 5: ~4.0 hours**

---

## Quick Wins Checklist (< 30 min each)

- [ ] **D-1**: Remove `msb_replicate_5/6` from `simd/mod.rs:65-74`, add `use crate::pixel_utils::{msb_replicate_5, msb_replicate_6};` in `simd/mod.rs`. (10 min)
- [ ] **D-7**: Remove `yuv::clamp()` wrapper at `yuv.rs:40-42`, replace `clamp(...)` with `crate::pixel_utils::clamp_u8(...)` in `yuv_to_bgra`. (5 min)
- [ ] **D-6**: Rewrite `apply_post_process` as `apply_post_process_with_transform(img, profile, &TransformConfig::default())`. (10 min)
- [ ] **IC-3**: Delete duplicate lines 1192-1193 in `pipeline/mod.rs` test. (2 min)
- [ ] **EH-5**: Change `encoding_name_for_prefix` return type to `Cow<'static, str>`. (15 min)
- [ ] **EH-4**: Add `?` to `dict.set_item()` calls in `pymod/src/lib.rs:118-121`. (5 min)
- [ ] **DC-1**: Remove or `#[cfg(test)]`-gate `rgb565_row_to_bgra` and `rgb555_row_to_bgra` wrappers in `simd/mod.rs`. (10 min)

---

## "Looks Bad But Is Fine"

1. **`enc/mod.rs:9` — `#![cfg_attr(not(test), allow(dead_code))]`**: The entire encoder module suppresses dead_code because it mirrors the decoder API but isn't wired into the public pipeline yet. This is intentional — the encoders are tested via roundtrip fuzz tests but not exposed as a public build API. The suppression is correct but should be temporary.

2. **`simd/mod.rs:8-9` — blanket `#![allow(unsafe_code, unreachable_code, dead_code)]`**: This module is the SIMD dispatch hub. `unsafe_code` is required for every platform-specific intrinsics block. `unreachable_code` is needed for `#[cfg]` conditional compilation where some platforms' code is unreachable on others. `dead_code` is needed because platform-specific functions appear dead when compiled for a different platform. The blanket allow is overly permissive but justified by the cross-platform SIMD pattern.

3. **`simd/scalar.rs:10,24` — `#[allow(dead_code)]` on scalar fallbacks**: These functions are only called when neither SSE2 nor NEON is available (e.g., WASM or non-SIMD targets). They appear dead on x86_64/aarch64 builds but are the sole implementations on other platforms. This is a correct pattern for conditional compilation.

4. **`profile_db.rs` — 233 lines of profile definitions**: The file contains 53 `Profile` structs with identical shape but different field values. This looks like bloat but is by design — each profile maps to a specific Apple device/format combination. The data could live in an external JSON (and does in `profiles.json`), but the compiled-in version avoids runtime parsing.

5. **`pipeline/mod.rs:310` — `..profile.clone()` for fallback encoding**: When the primary decoder fails and a fallback encoding is tried, the code clones the entire `Profile` to create a copy with `fallback_encodings: None` (preventing infinite recursion). This is a single allocation per fallback attempt and is structurally necessary.

6. **`cache.rs:141,153` — `.expect("cache lock poisoned")`**: RwLock poisoning only occurs if a thread panicked while holding the lock. At that point, the application is already in an undefined state. Panicking again during cache access is arguably correct — the alternative (returning a stale or empty cache) could mask the original crash.

---

## Open Questions

1. **Encoder pipeline integration**: The encoder module (`enc/`) has 754 lines of code + 550 lines of tests but is suppressed behind `allow(dead_code)`. Is there a plan to expose `build_ithmb_file` as a public API? If not, should the encoder be feature-gated or moved to a separate crate?

2. **WASM error model**: Should `ithmb-wasm` return structured errors (like `ithmb-core`'s `DecodeError`) instead of `Option<Vec<u8>>`? Current behavior collapses all errors into `None`, making debugging impossible for WASM consumers.

3. **SIMD unwrap elimination strategy**: The 11 `.try_into().unwrap()` calls in `simd/mod.rs` are mathematically safe but violate the "no panics in library code" principle. Should these use `unsafe` transmute (with SAFETY comments), propagate errors via `Result`, or is the current approach acceptable given the invariant guarantees?

4. **Module split priority**: `pipeline/mod.rs` (520 LOC non-test) and `simd/mod.rs` (591 LOC non-test) both exceed the 250 LOC guideline by 2×. Should these be split as a dedicated refactoring pass, or is the current structure acceptable given the code is working and well-tested?

5. **`ithmb-gen` tool quality**: The generator tool (`crates/ithmb-gen/src/main.rs`, ~200 LOC) uses `println!` for all output and `.expect()` for argument validation. Should it follow the same patterns as `ithmb-cli` (proper `clap` validation, structured output)?
