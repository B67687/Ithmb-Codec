<div align="center">

<img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/ithmb-icon.svg" alt="Ithmb Image Codec" width="96" height="96">

# Ithmb Image Codec

[![License: MIT](https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/license.svg)](LICENSE)
[![Rust](https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/rust.svg)](https://rust-lang.org)
[![Python](https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/python.svg)](pymod/)

[![Platform](https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/platform.svg)](README.md#build-from-source)

<a href="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/showcase.svg"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/showcase.svg" alt="Concept render" width="100%" max-width="720"></a>
<i>Concept render — not an actual screenshot.</i>
<hr style="max-width: 360px;">
<sub>Built with AI assistance — see <a href="./docs/CREDITS.md">CREDITS.md</a></sub>
<br>
<a href="./docs/CREDITS.md"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/deepseek.svg" alt="DeepSeek"></a>
<a href="./docs/CREDITS.md"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/opencode.svg" alt="OpenCode"></a>
<a href="./docs/CREDITS.md"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/badges/omo.svg" alt="Oh My OpenAgent"></a>






<br>


</div>
<br>

A pure Rust codec library, CLI tool, and C ABI shared library for decoding and encoding Apple `.ithmb` thumbnail-cache files — the format used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo and album art thumbnails.

**Key features**

- 54 built-in profiles (+ 1 speculative disabled) covering known iPod/iPhone formats
- 8 decoders (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CLCL, CL, JPEG)
- 7 synthetic encoders for all raw formats
- Roundtrip-proven tests — see [STATS.md](docs/STATS.md) for current count and suite breakdown
- PhotoDB/ArtworkDB read, write, and integrity checking
- Multi-frame F-prefix raw file support
- BGR15 channel-swap for iPhone compatibility
- JPEG-embedded T-prefix decoding via pure Rust JPEG decoder
- Cross-platform (Linux x64/ARM64, macOS x64/ARM64, Windows x64)
- C ABI shared library for FFI integration — see [separate plugin repo](https://github.com/B67687/Imageglass-Ithmb-Plugin)
- CLI tool for decoding, inspection, and frame extraction
- Python bindings (PyO3) for scripting and ML pipelines
- Full SIMD acceleration (SSE2+AVX2+NEON runtime dispatch) for YUV conversion paths

> Not an iOS 13+ thumbnail decoder — those use a different proprietary format.

**T-prefix** — contains an embedded JPEG. ✅ Fully supported (validated on 1,183 real files).

**F-prefix** (e.g. `F1019_1.ithmb`) — raw uncompressed thumbnails (RGB565, RGB555, UYVY, YCbCr420, CLCL nibble-chroma). ✅ Cross-referenced against iOpenPod's empirically validated set (50+ profiles across multiple iPod models) and confirmed on real iPod Classic 6G samples (F1061/F1055/F1060).

<table><tr><td>
🎖️ <strong>Special thanks to <a href="https://github.com/TheRealSavi">Savi</a> and the <a href="https://github.com/TheRealSavi/iOpenPod">iOpenPod</a> community</strong><br>
<em>For hardware validation — purchasing multiple iPod models and testing profiles across firmware generations. Profile tuning incorporates feedback from the iOpenPod community.</em>
</td></tr></table>

> [!TIP]
> New to `.ithmb` files? See [docs/what-is-this.md](docs/what-is-this.md) for a plain-english explainer.
> Confused by technical terms? See [docs/GLOSSARY.md](docs/GLOSSARY.md) for simple definitions.
> Want to extract photos from your iPod? See [docs/guides/GUIDE.md](docs/guides/GUIDE.md) for a walkthrough.

---

## Quick start

```bash
# Build from source
cargo build --release

# Decode a single .ithmb file to PNG
./target/release/ithmb my_photo.ithmb output.png

# Open a PhotoDB container and extract all thumbnails
./target/release/ithmb --open PhotoDB

# Or use from Python
pip install ithmb-python  # (not yet published — build from pymod/ instead)
```

For detailed build instructions see [Build from source](#build-from-source).

## Table of Contents

- [How it works](#how-it-works)
- [Acknowledgments](#acknowledgments)
- [Install](#install)
- [Build from source](#build-from-source)
- [Testing & validation](#testing--validation)
- [Architecture](#architecture)
- [CLI tool](#cli-tool)
- [C ABI shared library](#c-abi-shared-library)
- [Profile Reference](#profile-reference)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Contributions to the ecosystem](#contributions-to-the-ecosystem)
- [Changelog](#changelog)
- [License](#license)

---

## How it works

1. **Peek read** — reads the entire file into memory (peak memory dominated by the decoded bitmap, typically a few MB for iPhone photos). An 8 MB size guard prevents OOM from pathological input (see [ADR-0005](docs/adr/0005-file-size-guard-limit.md)).

2. **JPEG scan** — checks for the JPEG SOI marker (`FF D8`) followed by JFIF or Exif within 512 bytes. On match, the JPEG payload is extracted (SOI→EOI), decoded via `jpeg-decoder`, and its EXIF orientation tag (0x0112) is exposed through the profile system.

3. **Raw fallback** — if no JPEG is found, the decoder matches the first 4 bytes (big-endian prefix) against 54 known profiles and runs the appropriate raw decoder (RGB565, RGB555, Reordered RGB555, UYVY, YCbCr420, YUV422 interlaced, CLCL nibble-chroma, or CL per-pixel chroma) to produce BGRA output. If the prefix doesn't match any known profile, the file is scanned for embedded JPEG markers (byte-level carving) before being rejected. Additional decoder variants can be activated via `profiles.json`: swapped chroma planes for YCbCr 4:2:0, per-pixel vs shared nibble chroma, endianness toggles, interlaced field ordering, padded frame handling, and channel-swap for BGR15 formats.

4. **PhotoDB/ArtworkDB** — Apple's iPod thumbnail databases (PhotoDB, ArtworkDB) use a binary chunk-based format (MHFD→MHSD→MHNI entries). When a file starts with `mhfd`, the codec parses the chunk tree, extracts individual thumbnails (inline pixel data or external `.ithmb` file references), and decodes each via the raw decoder matching its format ID. Read, write, and integrity checking are all supported.

### File size guard

> [!NOTE]
> Files larger than **8 MB** are rejected before decoding to prevent OOM/DoS from pathological input. All known real .ithmb files are under 1 MB (max observed: 852 KB), and the largest single frame is 810 KB — so 8 MB provides ~10× margin on the largest frame. The actual iPod firmware caps individual .ithmb files at ~500 MB. See [ADR-0005](docs/adr/0005-file-size-guard-limit.md) for the full research.

## Acknowledgments

This project builds on the work of the iPod reverse-engineering community. Key references:

| Project | Author | Role |
|---------|--------|------|
| [iOpenPod](https://github.com/TheRealSavi/iOpenPod) | Savi | Primary format profile reference (50+ entries, empirically validated across multiple iPod models) |
| [libgpod](https://sourceforge.net/p/gtkpod/libgpod/ci/master/tree/) | community | PhotoDB/ArtworkDB chunk parser, format ID tables |
| [Keith's iPod Photo Reader](https://github.com/kebwi/Keiths_iPod_Photo_Reader) | kebwi | Original RE (2005), multi-frame confirmation, 13 decode methods |
| [clickwheel](https://github.com/dstaley/clickwheel) | dstaley | C# ArtworkDB read/write, 40+ format IDs |
| [OrgZ](https://github.com/FoxCouncil/OrgZ) | Fox | C# ArtworkDB+ithmb read/write |
| [pyithmb](https://github.com/wrinklykong/pyithmb) | wrinklykong | Python YUV reference decoder |

See [ACKNOWLEDGMENTS.md](docs/ACKNOWLEDGMENTS.md) for the full list (33 projects, sample file sources, academic references, and color conversion standards).

### Contribution breakdown

| Area | What was done | Who |
|------|--------------|-----|
| **Community foundations** | iOpenPod, libgpod, clickwheel, Keith's RE, pyithmb, ithmb-rs, 15+ more | Community |
| **Hardware-validated profiles** | 50+ profiles empirically validated across multiple iPod models | Savi (iOpenPod) |
| **Sample contribution** | First public F-prefix .ithmb test vectors + 30 reference PNG files (CC0) | Reuhno |
| **AI execution** | Code, testing, documentation, CI, format cross-referencing | AI (Sisyphus + OMO) |
| **Project lead** | Vision, architecture, quality control, community engagement, verification, hardware coordination | B67687 |

---

## Install

### Library

Add the `ithmb-core` crate to your `Cargo.toml`:

```toml
[dependencies]
ithmb-core = { git = "https://github.com/B67687/Ithmb-Codec", branch = "main" }
```

Or use the CLI binary directly (see [releases](https://github.com/B67687/Ithmb-Codec/releases)).

### CLI binary

Install the CLI binary from source (see build instructions below).
Full usage: see the [CLI tool](#cli-tool) section.

## Build from source

### Requirements

- [Rust](https://rust-lang.org) 1.88 or later (edition 2024)
- A C compiler toolchain (for native code linking)

### Build

```bash
# Clone the repository
git clone https://github.com/B67687/Ithmb-Codec.git
cd Ithmb-Codec

# Build everything in release mode
cargo build --release
```

The workspace produces four artifacts:

| Crate | Artifact | Install |
|-------|----------|---------|
| `ithmb-core`   | `libithmb_core.rlib` (static library) | `cargo add ithmb-core` (crates.io) |
| `ithmb-cli`    | `ithmb` CLI binary                   | `cargo install --path crates/ithmb-cli` |
| `ithmb-python` | `libithmb_python.{so,dylib,pyd}`     | `pip install pymod/` (or crates.io when published) |
| `ithmb-gen`    | `ithmb-gen` sample generator binary | `cargo install --path crates/ithmb-gen` |

A separate C ABI shared library for ImageGlass integration is maintained at [Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin).

### Cross-compilation

Cross-compiling for other targets requires the appropriate target toolchain installed via `rustup target add`:

```bash
# Linux x86-64 (default on x64 hosts)
cargo build --release

# Linux ARM64 (aarch64)
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu

# macOS ARM64
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Windows x64 (cross-compile from Linux)
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

Enable SIMD acceleration:
```bash
cargo build --release --features simd
```

Available features: `simd` (SSE2/AVX2/NEON YUV conversion), `cache` (LRU raw file cache), `metrics` (decode timing counters).

> [!NOTE]
> RGB565/RGB555 pixel-unpack formats use auto-vectorized scalar loops instead of hand-written SIMD — the hand-written SSE2/AVX2 was 34× slower on Intel due to AVX frequency downclock and port-5 bottleneck.

---

## Testing & validation

```bash
cargo test
```

See **[STATS.md](docs/STATS.md)** for current test counts and suite breakdown. Coverage includes:

- **Roundtrip** — RGB565 (65,536 values), RGB555 (32,768), CL (15,625 nibble combos)
- **Fuzz** — 350+ mutated inputs across all 8 decoders + 10,000 random byte mutations + 2 libfuzzer targets (1.2M+ iterations, 0 crashes)
- **Golden vectors** — 14 reference files verified against expected output
- **Concurrency** — 11 stress tests (Barrier sync, cancellation, repeated decode)
- **Statistical** — decorrelation, entropy, histogram analysis of decoded output
- **Edge cases** — zero dimension, corruption, partial data, truncation, oversized inputs
- **Speculative** — CL/CLCL, rotation, swapped chroma, trailing padding, JPEG carving fallback
- **PhotoDB** — roundtrip write, integrity, JPEG blob decode, device-specific format tables
- **Encoder** — interlace fields, BT.601 color conversion, all format generators

**12 test suites** across the workspace, all passing.

**Real-device validation:**

- **iPod Classic 6G (Reuhno):** Real F1061/F1055/F1060 .ithmb files decoded successfully (BGR15 channel-swap, MSB replication — both confirmed correct). 30 reference PNG files match decoder output.
- **iOpenPod (TheRealSavi):** Empirically validated 50+ profiles across multiple iPod models purchased and tested. Confirmed "no known issues for iPod Nano and iPod Classic models." Our 54 profiles derive from the same format ID sources — hardware validation covered by iOpenPod's testing. See [iOpenPod#140](https://github.com/TheRealSavi/iOpenPod/issues/140).
- **iPhone 5 (iOS 7):** 956 T-prefix files — 100% extraction
- **Jakarade.com F00-F08:** 227 public T-prefix files — 100% JPEG+EXIF detection
- **MVS CTF 2026 (iOS 18):** iPhone 14 Plus full filesystem image scanned — 3 `.ithmb` files found but use a different proprietary format (iOS Photos framework, not decodable by this codec)

> Other known sources (FAU.edu, ~500 files) have live directory listings but downloads return 404. Not available for testing.

---

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for a full diagram showing how all crates, bindings, and the ImageGlass plugin relate.

## Architecture

The project is organized as a Rust workspace with four crates:

### ithmb-core (library)

The core decoding library. All decoder logic lives here; wrappers for FFI, CLI use, or other languages call into this crate.

**21 modules** organized by domain:

| Module | Purpose |
|--------|---------|
| `pipeline/` | Central dispatch — reads format prefix, dispatches to the correct decoder, applies crop/rotation post-processing; accepts `&AtomicBool` for cancellation |
| `jpeg.rs` | JPEG decoder wrapper (`jpeg-decoder` crate), EXIF orientation parsing |
| `rgb565.rs` | RGB565 decoder (16-bit RGB 5/6/5) |
| `rgb555.rs` | RGB555 decoder (15-bit RGB 5/5/5) |
| `reordered_rgb555.rs` | Reordered RGB555 decoder (byte-swapped variant) |
| `uyvy.rs` | UYVY 4:2:2 + Interlaced UYVY decoders (YUV→BGRA uses SSE2/AVX2/NEON with `--features simd`) |
| `ycbcr420.rs` | YCbCr 4:2:0 planar decoder (YUV→BGRA uses SSE2/AVX2/NEON with `--features simd`) |
| `clcl.rs` | CLCL nibble-chroma decoder (YUV→BGRA uses SSE2/AVX2/NEON with `--features simd`) |
| `cl.rs` | CL per-pixel chroma decoder (YUV→BGRA uses SSE2/AVX2/NEON with `--features simd`) |
| `yuv.rs` | Shared YUV conversion helpers, SSE2/AVX2/NEON runtime dispatch |
| `simd/` | SSE2/AVX2/NEON YUV conversion dispatch (feature-gated), per-format SIMD sub-modules |
| `device_profiles.rs` | 18-device iPod/iPhone format lookup table |
| `enc.rs` | Synthetic encoders for all raw formats |
| `enc/helpers.rs` | Shared encoder helpers (InterlaceFields, BT.601) |
| `profile.rs` | Profile struct (IthmbVariantProfile, IthmbEncoding) |
| `profile_db.rs` | Built-in profile database (54 entries) |
| `profile_parser.rs` | JSON parser for external `profiles.json` |
| `cache.rs` | LRU raw file cache (feature-gated) |
| `metrics.rs` | Decode timing counters (feature-gated) |
| `error.rs` | Typed error enum (`DecodeError`) + decoded image type (`DecodedImage`) |
| `photodb/` | PhotoDB/ArtworkDB chunk parser, writer, integrity checker, and type definitions |

**Data flow:**

```
.ithmb file → JPEG/EXIF scan ──→ JPEG slice → jpeg-decoder crate → BGRA
                ├─ No JPEG → prefix lookup → raw decoder → BGRA
                │              └→ no prefix + JPEG scan → byte-level SOI carving → jpeg-decoder → BGRA
                │              └→ no prefix + mhfd magic → PhotoDB parser → entries → raw decoder → BGRA
                └─ external .ithmb reference → read file → raw decoder → BGRA
```

**Multi-frame support** — F-prefix `.ithmb` files may contain multiple concatenated raw frames (confirmed by Keith's iPod Photo Reader, ithmbrdr, libgpod, and iOpenPod). The codec detects frame count from file size. Callers can access individual frames via frame index (0-based); out-of-range indices return an error. JPEG-embedded T-prefix files are always single-frame.

### ithmb-cli (CLI binary)

A command-line tool for decoding, inspecting, and analyzing `.ithmb` files. Built with `clap` for argument parsing and `png` crate for PNG output.

```bash
# Decode a file to PNG
ithmb input.ithmb output.png

# Print file metadata only
ithmb --info input.ithmb

# List all known profiles
ithmb --list-profiles

# Extract a specific frame from a multi-frame file
ithmb --frame 2 input.ithmb output.png

# Raw BGRA output (no PNG encoding)
ithmb --raw input.ithmb output.bin
```

### C ABI shared library

The C ABI library (`ithmb-core-cabi`) — a `cdylib` implementing the ImageGlass v10 native plugin ABI — is now maintained in its [own repository](https://github.com/B67687/Imageglass-Ithmb-Plugin) to keep the plugin scope separate from the format codec. It enables integration into the ImageGlass image viewer **(Windows-only)** and provides FFI from any language with C FFI support.

### ithmb-python (PyO3 bindings)

A `cdylib` exposing ithmb-core to Python 3.12+ via PyO3 (abi3-py312). Built with `maturin build`.

### ithmb-gen (sample generator)

A CLI tool for generating synthetic `.ithmb` test vectors and reference PNG files used during development.

<div align="center"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/diagrams/architecture.svg" alt="Architecture diagram" width="100%"></div>

---

## CLI tool

The `ithmb` CLI tool supports four modes:

| Command | Description |
|---------|-------------|
| `ithmb input.ithmb [output.png]` | Decode to PNG (auto-detects format from extension) |
| `ithmb --info input.ithmb` | Print metadata (size, prefix, profile, frame count) |
| `ithmb --list-profiles` | Print the 54-profile database as a formatted table |
| `ithmb --frame N input.ithmb [output.png]` | Extract frame N from a multi-frame file |

Output options: `--raw` for raw BGRA binary, `--format bin` for explicit binary, `.png` extension auto-selects PNG.

## Benchmarks

Measured on AMD Ryzen AI 9 HX 370 with `--features simd`. Same order across both Rust and C#.

| Decoder | 64×64 | 720×480 |
|---------|-------|--------|
| RGB565 | 0.52 µs | 45 µs |
| RGB555 | 0.55 µs | 47 µs |
| UYVY | 0.97 µs | 82 µs |
| UYVY (interlaced) | 2.05 µs | 82 µs |
| YCbCr 4:2:0 | 3.6 µs | 199 µs |
| CL | 1.3 µs | 112 µs |
| CLCL | 0.22 µs | 19 µs |
| Reordered RGB555 | — | 632 µs (512×512) |

See [`BENCHMARKS.md`](docs/benchmarks/BENCHMARKS.md) for full results (all 4 sizes, encoder throughput, methodology).
With `--features simd`, Rust is faster than the C# original on 5 of 8 decoders (see BENCHMARKS.md for full comparison).
### Performance Limits

Theoretical maximum throughput per format at 256×256 (L2 cache ~1 MB, ~100 GB/s bandwidth):

| Format | Current | Limit | Bottleneck | Room to improve |
|--------|---------|-------|------------|-----------------|
| RGB565 | **7.5 µs** | **7.5 µs** | L2 bandwidth saturated (35 GB/s) | **0%** — at hardware limit |
| RGB555 | **7.7 µs** | ~7.5 µs | L2 bandwidth | ~3% |
| CLCL | **2.9 µs** | ~2 µs | Nibble table lookup (4→8 bit expansion) | ~25% |
| CL | **49 µs** | ~15 µs | Per-pixel BT.601 YUV→BGRA (ALU throughput) | ~70% |
| UYVY | **17 µs** | ~8 µs | AVX2 vpshufb + vpmaddwd pipeline latency | ~50% |
| YCbCr 4:2:0 | **38 µs** | ~15 µs | Per-pixel YUV clamp+pack (same UYVY bottleneck) | ~60% |
| ReorderedRGB555 | **106 µs** | ~80 µs | Z-order Morton non-linear address pattern | ~25% |

> **Note on bottlenecks** — Simple pixel unpack (RGB565, RGB555) is memory-bandwidth limited.
> YUV formats (UYVY, YCbCr420, CL, CLCL) are ALU-limited by per-pixel BT.601 arithmetic.
> CLCL achieves near-memory-bandwidth despite the ALU bottleneck because its plane-separated
> layout allows 8-pixel SSE2 row processing without address interleave overhead.

---

## Project History

This codec was first implemented in **C#** as a Native AOT plugin for ImageGlass v10. The **Rust** workspace evolved from that reference to enable broader distribution (crates.io, CLI, Python bindings). See [EVOLUTION.md](docs/EVOLUTION.md) for the full migration story, architecture decisions, and acknowledgments.

The C# reference repository ([B67687/Ithmb-Codec-CSharp](https://github.com/B67687/Ithmb-Codec-CSharp)) is archived but remains the authoritative source for algorithm verification.
## Profile Reference

**54 known profiles** (+ 1 speculative disabled — see note in codebase) covering iPod Photo 4G through iPhone 2G and iPod Nano 7G. Max frame size: 480×864 (RGB565, 830 KB). See [docs/PROFILES.md](docs/PROFILES.md) for the full table with dimensions, encoding, and device mapping. External profiles can be added at runtime via `profiles.json`.

Each profile defines the pixel encoding, dimensions, byte length per frame, and post-processing flags (crop, rotation, channel swap, dimension swap, interlacing, padding).

---

## Limitations

> [!WARNING]
> **T-prefix (JPEG-embedded) validated on 1,183 real files (956 iPhone 5 + 227 Jakarade); F-prefix raw decoders validated on iPod Classic 6G samples (F1061/F1055/F1060).** Raw decoders exist for 54 known profiles and pass roundtrip tests (see [STATS.md](docs/STATS.md) for current count).
>
> **F-prefix decoder coverage is broad but not exhaustive.** 54 profiles cover known iPod/iPhone formats through iPod Nano 7G and iPhone 2G. Unknown formats from obscure firmware versions may still exist. [Open an issue](https://github.com/B67687/Ithmb-Codec/issues) if you encounter one.
>
> **JPEG SOI must be within the first 4 MB** of the file (covers all known real files). For unknown raw files, the codec falls back to byte-level JPEG carving.
>
> **Hardware validation details** — see [HARDWARE_GUIDE.md](docs/guides/HARDWARE_GUIDE.md) for the full device testing matrix and methodology.
>
> **SIMD acceleration** — SSE2 for YUV conversion paths (UYVY, YCbCr420, CL, CLCL). AVX2 and ARM NEON runtime dispatch via `--features simd`. On macOS ARM, NEON is gated (known runner edge case) and falls back to scalar — see [STANDARDS.md](docs/standards/STANDARDS.md) for details.

---

## Troubleshooting

| Symptom                      | Likely cause / What to do                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| File won't open              | May use an unknown format variant. [Open a codec issue](https://github.com/B67687/Ithmb-Codec/issues) with a sample.    |
| Garbled image / wrong colors | JPEG false positive or raw decoder mismatch (rare). [Open a codec issue](https://github.com/B67687/Ithmb-Codec/issues). |
| "File too large" error       | File exceeds the **8 MB** guard — should never happen for normal iPhone photos. Open an issue if it does.               |

> [!TIP]
> If a file doesn't decode correctly, [open an issue](https://github.com/B67687/Ithmb-Codec/issues) with a sample link. You can also try [ithmb.org](https://ithmb.org) — a browser-based .ithmb decoder (offline, no upload) — to compare results.

---

## Development

The library was developed through iterative research, implementation, review, and release cycles:

1. **Format survey** — 33 open-source .ithmb implementations found and analyzed
2. **Format table extraction** — iOpenPod (50+ entries), libgpod, iLounge threads, and Keith's iPod Photo Reader provided dimension/encoding tables for 54 profiles
3. **Implementation** — Pure Rust workspace with 8 decoders, JPEG decode, PhotoDB/ArtworkDB support, CLI tooling, PyO3 Python bindings, and sample generator
4. **Testing** — Current test count in [STATS.md](docs/STATS.md); unit tests across roundtrip, fuzz, libfuzzer, parsers, speculative paths, buffer-too-small guards, trailing-padding tolerance, JPEG carving fallback, multi-frame raw decode, rotation roundtrip, byte-level corruption fuzz, BGR15 channel-swap, PhotoDB roundtrip write, PhotoDB integrity, PhotoDB JPEG blob decode, device-specific format tables, concurrency stress tests, and format ID profile tests
5. **Review cycles** — 5 rounds of multi-agent review: ~47 findings fixed covering memory safety, threading, ABI compatibility, buffer overflow, integer overflow, and defense-in-depth
6. **Release** — Published via GitHub Releases

<div align="center"><img src="https://cdn.jsdelivr.net/gh/B67687/Ithmb-Codec@main/docs/diagrams/pipeline.svg" alt="Development pipeline diagram" width="100%"></div>

See [CHANGELOG.md](CHANGELOG.md) for the full version history.

### Quality pipeline

Quality checks run locally before release:

```bash
cargo clippy --all-targets -- -D warnings  # Lint
cargo test                                 # All tests (see [STATS.md](docs/STATS.md) for current count)
cargo audit                                # Advisory check
cargo fuzz build                           # Fuzz targets compile check
```

## Contributions to the ecosystem

This project made several original contributions to the .ithmb reverse-engineering space.
See [`docs/ECOSYSTEM.md`](docs/ECOSYSTEM.md) for the full write-up.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

---

## Support

If this project helps you recover iPod photos, decode thumbnails, or save old memories:

<div align="center">

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/ThumbNami)

</div>

Every bit helps keep the research going. Thank you 🙏

---

## Quality Assurance

This codec has been systematically reviewed across multiple rounds:

| Round | Scope | Date |
|-------|-------|------|
| 1 | Initial code audit — format profiles, pipeline, all 7 decoders | 2026-06 |
| 2 | Documentation review — all 19 markdown files, link checking (lychee) | 2026-06 |
| 3 | C# reference parity — golden vectors, exhaustive roundtrip, SIMD constants, zero-alloc audit | 2026-07 |
| 4 | Coverage expansion — SIMD tail boundaries, cancellation, profile validation, statistical completeness | 2026-07 |
| 5 | Polish — benchmark regression baseline, ADR docs, review documentation | 2026-07 |

**Current test count:** See [STATS.md](docs/STATS.md) for the latest count and suite breakdown.

**Known gaps:**
- macOS ARM NEON is gated (known CI runner edge case) — falls back to scalar. See [STANDARDS.md](docs/standards/STANDARDS.md).
- No dedicated NEON CI runner (deferred — no reliable ARM64 CI available at this time).
- Cache concurrency stress tests (deferred — feature-gated LRU cache, low user impact).

The C# reference implementation ([B67687/Ithmb-Codec-CSharp](https://github.com/B67687/Ithmb-Codec-CSharp)) underwent 5 review rounds of its own before being archived. The Rust port builds on that foundation with cross-platform distribution as its primary goal.

## License

MIT — see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This library is a clean-room implementation, informed by format behavior described in that PR but using no GPL code.
