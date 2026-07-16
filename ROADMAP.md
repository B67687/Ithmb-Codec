# Ithmb-Codec Roadmap

## Recently shipped

These items are already done but worth calling out so contributors don't propose them again:

- **WASM decoder page**: Browser-based .ithmb decoder at [Ithmb-Codec-Dev](https://github.com/B67687/Ithmb-Codec-Dev) with drag-drop PNG rendering (v1.10.0-enterprise).
- **C ABI plugin split**: The ImageGlass plugin lives in its own repo ([Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin)) with independent versioning and CI (ADR-0002).
- **8 MB file size guard**: Systematic research-driven limit with 10x margin on the largest known frame (ADR-0005).
- **Cross-platform SIMD dispatch**: SSE2/AVX2/NEON compiled with runtime `is_x86_feature_detected!` selection; scalar fallback always available (ADR-0001).
- **Quarterly audit protocol**: Layered adversarial reviews every quarter with 4 parallel research agents (ADR-0004).
- **JPEG carving fallback**: Byte-level SOI scan for files with unknown prefixes, enabling decode when no profile matches.
- **CL decoder SSE4.1 + AVX2**: SSE4.1 `pshufb` and AVX2 paths added for the CL per-pixel chroma decoder (2-3x gain).
- **Benchmark CI**: Automated regression detection with 25% threshold against saved baseline artifacts.

---

## v1.10 (Next release)

### Hardware-accelerated NEON row functions for Apple Silicon

macOS ARM NEON is currently gated behind `#[cfg(not(target_os = "macos"))]` because a CI runner edge case causes the NEON path to fail. This means macOS ARM users fall back to scalar, losing 2-3x throughput on YUV decoders. The fix requires either a CI runner resolution (tracked in iOpenPod#140) or an alternative validation path. Once unblocked, NEON dispatch on macOS ARM will match Linux ARM behavior.

### DecodeConfig runtime API

Currently, decode parameters (rotation, crop, channel swap, chroma ordering) are driven entirely by built-in profile fields. A `DecodeConfig` struct would let library callers override any profile field at runtime: force a different encoding, toggle post-processing flags, or supply custom dimensions. This is a non-breaking additive change to the `ithmb-core` public API.

### Full 54-profile benchmark coverage with CI regression detection

The benchmark suite covers all 8 decoders but at a single resolution per format. Expand coverage to all 54 profiles at their native dimensions, and wire the results into the CI regression detector (currently at a 25% threshold). This catches performance regressions caused by changes to shared YUV conversion or SIMD dispatch.

### ARM64 CI tests on native hardware

Linux ARM64 (aarch64) CI runs on GitHub Actions `ubuntu-24-arm` runners. Ensure all 17 test suites pass with full NEON acceleration, and add the `--features simd` matrix to the ARM64 CI job (currently only runs on x64). This closes the gap where ARM64 SIMD is tested only in ad-hoc local runs.

---

## v2.0

### Encoding SIMD (SSE2/AVX2/NEON)

Decoding has SIMD for YUV conversion; encoding does not. The 7 synthetic encoders (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CLCL, CL) use scalar BGRA-to-YUV conversion. Adding SSE2/AVX2/NEON paths to the encoder YUV conversion would give 2-5x encoder throughput, matching the decoder-side architecture. Same dispatch strategy as ADR-0001: SSE2, AVX2 (compiled but conditionally dispatched), NEON, scalar fallback.

### WASM decoder page fully functional

The current WASM page at ithmb-codec-dev supports drag-drop decode to PNG. Ship the remaining features: frame index selection for multi-frame files, PhotoDB container browsing (expand chunk tree, select thumbnail entries), profile info display, and download-as-PNG for individual frames. Hosted on GitHub Pages with a simple deploy workflow.

### Python bindings published to PyPI

The PyO3 bindings in `pymod/` work but are only buildable from source. Publish `ithmb-python` to PyPI with abi3-py312 stable ABI wheels for Linux x64/ARM64, macOS x64/ARM64, and Windows x64. Build via `maturin build --release` in CI and publish on version tags. Includes a README on PyPI with basic usage examples.

### Automated crates.io / PyPI publishing on version tags

Currently, publishing is manual. Add a CI workflow triggered by `v*` tags that runs `cargo publish -p ithmb-core`, then `cargo publish -p ithmb-cli`, then builds and publishes the Python wheels to PyPI. Use trusted publishing (OIDC) for both registries to avoid token management.

### `no_std` support for embedded / IoT targets

The core library depends on `std` for file I/O, heap allocation, and synchronization. Refactor the format parsing, pixel decoding, and profile lookup to work in `no_std` + `alloc` environments. This enables `.ithmb` decoding on embedded Linux, microcontrollers with a Rust target, and IoT devices that need thumbnail extraction without an OS. The `std`-dependent parts (file I/O, cache, metrics) stay behind feature gates.

### Property-based test suite with proptest

The existing test suite has exhaustive tests (65,536 RGB565 values, 32,768 RGB555 values) but no property-based fuzzing. Add a `proptest` suite that generates random width/height/encoding combinations, random profile overrides, random frame counts, and random corruption patterns. Target 256+ cases per format, covering edge widths, padded profiles, and multi-frame boundaries.

---

## v2.1

### HEIC/AVIF thumbnail extraction (beyond .ithmb)

iPhones and iPads store thumbnails in `.ithmb` files, but newer iOS versions use HEIC/AVIF for the main image. A companion decoder for embedded HEIC/AVIF thumbnails (not full-image decode, just the embedded thumbnail stream) would expand the tool's value for iOS device forensics and photo recovery. This is a separate crate or optional feature -- no dependency on full HEIC/AVIF decode libraries.

### Streaming / batched decode API for bulk processing

The current API loads the entire file, decodes one frame, and returns a `Vec<u8>`. For PhotoDB containers with hundreds of entries, or for batch CLI decode of a directory, a streaming API that accepts an iterator of inputs and returns decoded frames via a channel (or a callback) would reduce peak memory and improve throughput. Design should match the existing `open_ithmb` / `decode_bytes` pipeline structure.

### Runtime metrics hooks for production monitoring

The `metrics` feature (behind feature gate) exposes atomic counters for decode counts, timings, and errors. Add a callback hook so production users can plug in their own metrics backend (Prometheus, OpenTelemetry, or a simple log sink) without modifying the library. The hook fires on decode start/complete/fail and receives a `MetricsEvent` struct with duration, format, dimensions, and error type.

### Dynamic profile discovery (deep scan of unknown prefixes)

Currently, unknown prefixes trigger JPEG carving fallback. A deeper scan would attempt dynamic profile discovery: try each decoder in turn with plausible dimension guesses (from file size and aspect ratio heuristics), validate the output with statistical tests (entropy, histogram, decorrelation from ADR-0004), and cache successful matches for the session. This would help users decode `.ithmb` files from obscure firmware versions without waiting for a profile update.

### Benchmark dashboard with historical tracking

The benchmark CI saves artifacts but there is no dashboard. Set up a simple static site (GitHub Pages) that displays benchmark history across releases: per-format throughput, SIMD vs scalar comparison, encode vs decode ratios. Data fed from CI artifact JSON. This makes regressions visible across releases without manually downloading and comparing artifacts.

---

## Long-term

### Formal verification of unsafe SIMD code (KLEE or similar)

All `unsafe` blocks are confined to `simd/*.rs`. Miri covers 21 SSE2 tests with zero UB, but Miri cannot prove the absence of UB in all paths. Explore KLEE (or a Rust-compatible symbolic execution tool) to exhaustively verify the SIMD kernels: correct masking, no out-of-bounds access for any width, correct tail handling for every non-SIMD-divisible dimension. This is a long-term investment that depends on toolchain maturity.

### Plug-in system for custom format handlers

The 54 built-in profiles cover known iPod/iPhone formats, but new formats are discovered every few months. A plug-in system (trait-based, loadable at runtime via `dyn FormatHandler`) would let third parties write decoders for new formats without forking the core library. This is deferred because it requires stabilizing the internal decoder trait, which is still evolving.

### GPU-accelerated decode via compute shaders

YUV-to-BGRA conversion is ALU-bound, making it a candidate for GPU offload. A compute-shader path (WGSL for WebGPU, or SPIR-V for Vulkan) would batch-convert large batches of frames. This is relevant for the WASM decoder (WebGPU shader) and for bulk CLI decode on systems with a GPU. Low priority -- the CPU SIMD paths already saturate memory bandwidth for simple formats and achieve 3-10 us per frame.

### FFmpeg / libav integration for video thumbnail extraction

Video thumbnail caches (`.ithmb` files from video files on iPods) exist but are not well documented. Integration with FFmpeg or libav would allow extracting the video frame that corresponds to a given thumbnail, enabling use cases like "find the video and jump to the scene this thumbnail represents." This is speculative -- no video-to-thumbnail mapping format has been confirmed in the wild.
