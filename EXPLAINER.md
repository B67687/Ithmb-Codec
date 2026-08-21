# EXPLAINER.md: Code Explanation

> Generated at REVIEW start, before the fixed checklist. Bridges the gap between "the AI built it" and "you understand what it does and why."
> One explanation per project. Updated as the project evolves. This version describes the AS-BUILT codebase at HEAD aae56f4.

## For the Reader

You built a project with help from AI agents. You can't read the code directly, but you should still understand what your project does, how its parts fit together, and why certain decisions were made. This document gives you that understanding.

Think of it as a guided tour. After reading it, you should be able to describe this repo to someone else with confidence.

---

## 1. Macro Architecture

Ithmb-Codec is a pure Rust codec for Apple `.ithmb` thumbnail-cache files, the format used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo and album-art thumbnails. The workspace is a five-crate Rust project: `ithmb-core` is the codec library that owns all decoding and encoding logic, and four thin wrappers sit on top of it (`ithmb-cli` for the command line, `ithmb-wasm` for the browser, `pymod` for Python, and `ithmb-gen` for generating synthetic test files). A separate `fuzz/` package (excluded from the workspace) holds six libfuzzer targets that hammer the decoders with arbitrary bytes. The core is deliberately dependency-light: it ships with an empty default feature set, and optional features (`cache`, `metrics`, `c`, `logging`) add capabilities only when a consumer asks for them.

## 2. Data Flow Walk

Trigger: a user runs `ithmb photo.ithmb out.png` on a real iPod Classic 6G sample.

1. **ithmb-cli** (`crates/ithmb-cli/src/main.rs`) parses the arguments with clap, reads the file into memory, and calls `ithmb_core::decode_ithmb(&bytes, &canceled)`.
2. **pipeline/mod.rs** (the core's central dispatch) takes over. It first checks the file size against the 8 MB guard and rejects oversized input before any allocation. It reads the first 4 bytes as a big-endian prefix and checks whether the file starts with a JPEG SOI marker (`FF D8`) followed by JFIF or Exif within 512 bytes.
3. For this F-prefix raw file there is no JPEG, so the pipeline resolves the prefix against the **profile_db** (`profile_db.rs`), which holds 53 built-in profiles loaded from embedded JSON. The prefix maps to a `Profile` describing dimensions, pixel encoding, byte length per frame, and post-processing flags (crop, rotation, channel swap, interlacing, padding).
4. **dispatch_decode** routes the frame payload to the matching raw decoder: `rgb565.rs`, `rgb555.rs`, `reordered_rgb555.rs`, `uyvy.rs`, `ycbcr420.rs`, `cl.rs`, or `clcl.rs`. For YUV formats the decoder calls into **yuv.rs / simd/** which selects SSE2, AVX2, or NEON kernels at runtime (with a scalar fallback) to convert YUV to BGRA.
5. The decoded frame returns to the pipeline, which applies **apply_post_process** (channel swap, crop, rotation) and wraps the result in a `DecodedImage` (BGRA8 pixel data plus width and height).
6. Control returns to **ithmb-cli**, which encodes the BGRA buffer to PNG via the `png` crate and writes `out.png` to disk. End: a viewable PNG that matches the reference decoder's output pixel for pixel.

If any step fails, the pipeline returns a typed `DecodeError` (with the values that caused it, such as expected/actual sizes) and the CLI prints the message and exits non-zero. No panic escapes the library.

## 3. Module Breakdown

Modules are listed in dependency order, foundation first. Every module has a single responsibility.

| Module | Responsibility | Public API | Key Types |
|--------|---------------|------------|-----------|
| `error.rs` | Typed error enum and image container used by every module | `DecodeError` (8 variants), `DecodedImage` | `DecodeError`, `DecodedImage` |
| `profile.rs` | Profile type and encoding enum; the format-variant description | `Profile`, `Encoding`, `built_in_profiles` | `Profile`, `Encoding` |
| `profile_db.rs` | Built-in profile database (53 active entries from embedded JSON) | `ProfileDb::load_builtin`, `get`, `resolve`, `all` | `ProfileDb` |
| `profile_parser.rs` | JSON parser for external `profiles.json` overrides | `parse_profiles_json` | `Profile` |
| `device_profiles.rs` | 18-device iPod/iPhone format lookup table | `find_device` | device profile table |
| `pipeline/` | Central decode dispatch: prefix parse, profile resolution, JPEG carving fallback, post-processing | `decode_ithmb`, `decode_ithmb_with_config`, `decode_ithmb_with_transform`, `decode_with_profile`, `open_ithmb`, `open_ithmb_with_config`, `encoding_name_for_prefix` | `DecodeConfig`, `TransformConfig` |
| `rgb565.rs`, `rgb555.rs`, `reordered_rgb555.rs` | Raw RGB pixel-unpack decoders (auto-vectorized scalar loops) | `decode(data, profile, canceled)` | `DecodedImage` |
| `uyvy.rs`, `ycbcr420.rs`, `cl.rs`, `clcl.rs` | Raw YUV decoders (SIMD-accelerated YUV to BGRA) | `decode(data, profile, canceled)` | `DecodedImage` |
| `jpeg.rs` | JPEG-embedded stream decoder via zune-jpeg, EXIF orientation (0x0112) | `decode(data, profile, canceled)` | `DecodedImage` |
| `yuv.rs`, `simd/` | Shared YUV conversion helpers and SSE2/AVX2/NEON kernels with scalar fallback | per-format conversion functions | SIMD kernel functions |
| `photodb/` | PhotoDB/ArtworkDB chunk parser, writer, integrity checker | `try_parse_photodb`, `PhotoDbEntry`, `PhotoDbMetadata` | `PhotoDbEntry`, `PhotoDbMetadata` |
| `enc/` | Seven synthetic encoders for all raw formats (test-vector generation) | `encode_rgb565`, `encode_rgb555`, `encode_reordered_rgb555`, `encode_uyvy`, `encode_ycbcr420`, `encode_cl`, `encode_clcl`, `build_ithmb_file` | encoder functions |
| `cache.rs` | LRU raw file cache (feature `cache`) | cache API | `LruCache` |
| `metrics.rs` | Decode timing counters (feature `metrics`) | metrics API | counters |
| `c_api.rs` | C ABI exports (feature `c`) | `ithmb_decode`, `ithmb_prefix_to_profile` | FFI-safe marshalling |
| `ithmb-cli` | Command-line binary: decode, inspect, extract | the `ithmb` binary | clap `Cli` struct |
| `ithmb-gen` | Synthetic sample generator binary | the `ithmb-gen` binary | clap `Args` struct |
| `ithmb-wasm` | WASM bindings for browser use | `decode_ithmb`, `peek_prefix`, `get_encoding_name` | wasm-bindgen functions |
| `pymod` | Python bindings via PyO3 (abi3-py312) | `decode_ithmb`, `open_ithmb`, `list_profiles` | PyO3 functions |
| `fuzz/` | Six libfuzzer targets (decode, open, encode roundtrip, pipeline, photodb, profile) | fuzz targets run by cargo-fuzz in CI | libfuzzer harnesses |

The most complex module is `pipeline/`: it hides the resolution order (prefix lookup, then data-size heuristic within 256 bytes, then byte-level JPEG carving), the fallback-encoding chain, and the post-processing order. Everything else in the codebase treats it as a black box that turns bytes into `DecodedImage`.

## 4. Key Decisions

**Decision 1: Rust clean-room port over the C# reference.**
The codec was first written in C# as a Native AOT plugin for ImageGlass v10. We needed to distribute it as a library, CLI, Python bindings, and WASM, and the C# plugin was trapped inside a Windows-only GUI ABI. We chose a pure Rust workspace with a C ABI from day one. The tradeoff: a full rewrite and a parity gap that took three waves of quality work to close against the C# reference (documented in docs/EVOLUTION.md). The C# repo is archived but remains the authoritative algorithm reference.

**Decision 2: Static profile database over pure format detection.**
`.ithmb` files are headerless blobs keyed by a 4-byte format prefix, and one prefix can map to multiple resolutions and encodings. Guessing the format from bytes alone risks silent misdecodes. We chose a curated database of 53 built-in profiles (derived from iOpenPod's empirically validated set and cross-referenced against libgpod and Keith's iPod Photo Reader), resolved by prefix with a data-size heuristic as a tiebreaker. The tradeoff: a maintenance burden, and unknown formats from obscure firmware versions may not decode (documented as a limitation).

**Decision 3: 8 MB file size guard before any allocation.**
The codec decodes untrusted files from photo libraries. The largest known real frame is 810 KB, so we reject files over 8 MB before allocating, giving roughly a 10x safety margin against OOM/DoS from pathological input. The tradeoff: a hypothetical legitimate file over 8 MB would be rejected (the actual iPod firmware caps files at ~500 MB, so this is theoretical).

**Decision 4: Runtime SIMD dispatch with a scalar fallback.**
YUV-to-BGRA conversion is the ALU-bound hot path. We chose runtime `is_x86_feature_detected!` dispatch (SSE2/AVX2/NEON) with a scalar fallback rather than per-target compile-time builds. The tradeoff: on macOS ARM the NEON path is gated on CI runners and falls back to scalar (a known, documented edge case). Hand-written SIMD for RGB565/RGB555 was measured and rejected: it was 34x slower on Intel due to AVX frequency downclock and port-5 bottleneck, so those decoders use auto-vectorized scalar loops.

**Decision 5: Zero-default dependencies with optional features.**
As a library meant for embedding (WASM, embedded, FFI), we ship with an empty default feature set. LRU cache, metrics, C ABI, and logging are all opt-in features. The tradeoff: consumers must explicitly enable what they need, and feature-flag combinations add a small testing surface.

**Decision 6: zune-jpeg over the full image crate for JPEG decode.**
T-prefix files embed a JPEG stream. We chose zune-jpeg 0.5 for a small, auditable, dependency-light decode path instead of pulling in the full image crate. The tradeoff: a narrower feature set than the image crate offers (the image crate remains a dev-dependency for golden-vector comparison only).

## 5. Quality Guarantees

**Tests.** 570 unit tests across 17 suites (docs/STATS.md). Coverage includes: exhaustive per-format roundtrip (RGB565 over all 65,536 values, RGB555 over 32,768, CL over 15,625 nibble combos), 14 golden vectors verified against the C# reference, 11 concurrency stress tests, statistical analysis of decoded output, edge cases (zero dimension, corruption, truncation, oversized input), PhotoDB roundtrip and integrity, and 21 Miri tests over the unsafe SIMD kernels with zero UB findings.

**Fuzzing.** Six libfuzzer targets run in CI (30 s each): decode, open, encode roundtrip, pipeline with transforms, PhotoDB parsing, and profile parsing. Over 1.2M iterations with zero crashes.

**Invariants.** Decoded output length always equals width x height x 4 (BGRA). No decoder panics on malformed input; every failure is a typed `DecodeError` carrying the values that caused it. The 8 MB size guard is enforced before any allocation. `unsafe` is confined to the SIMD kernels and the C-ABI boundary (workspace lint `unsafe_code = "deny"`).

**Safety guarantees.** Rust's ownership and bounds checking prevent use-after-free and buffer overflows at compile time. Errors are explicit `Result` values, never exceptions or crashes. Cancellation is cooperative via an `AtomicBool` checked at 64 KiB intervals.

**Automated checks.** Every commit and PR runs: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, typos, lychee link check, cargo-deny (license allowlist, multiple-versions ban, crates.io-only source registry), cargo-audit, `cargo doc` with `RUSTDOCFLAGS="-D warnings"`, and a gitleaks secrets scan. Main-branch pushes additionally run a 3-OS build/test matrix, a benchmark regression gate (25% threshold), the fuzz targets, the C-ABI symbol check, and the WASM build.

**Honest limits.** macOS ARM NEON is gated on CI and falls back to scalar; there is no dedicated NEON CI runner. Cache concurrency stress tests are deferred (feature-gated LRU cache, low user impact). F-prefix decoder coverage is broad but not exhaustive: 53 profiles cover known formats through iPod Nano 7G and iPhone 2G, but unknown formats from obscure firmware may still exist. JPEG SOI must appear within the first 4 MB of a file (covers all known real files). Line coverage is measured locally via cargo llvm-cov but is not enforced in CI.

---

## Mandatory Check

1. What does this project do, and what are its 3-5 main pieces? A pure Rust codec for Apple .ithmb thumbnail files; the core library, the CLI, the WASM/Python bindings, the sample generator, and the fuzz package.
2. What happens from start to finish when you trigger the main action? Running `ithmb photo.ithmb out.png` walks through CLI parsing, the 8 MB guard, prefix and JPEG detection, profile resolution, format dispatch, SIMD YUV conversion, post-processing, and PNG output.
3. Which module has the most complexity, and what does it hide? `pipeline/` hides resolution order, fallback encodings, JPEG carving, and post-processing order.
4. What was the hardest design decision, and why was it made that way? The Rust clean-room port over the C# reference, chosen for cross-platform distribution at the cost of a full rewrite and a long parity-closing effort.
5. What would break first if something went wrong, and how would you know? A decoder regression would fail the golden-vector or exhaustive roundtrip tests in CI; a format gap would surface as an `Unsupported` error on a real file, reported via the issue tracker.