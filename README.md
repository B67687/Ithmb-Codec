<div align="center">

# ITHMB Codec for ImageGlass v10

<a href="./docs/badges/ithmb-codec-logo.svg"><img src="docs/badges/ithmb-codec-logo.svg" alt="ithmb-codec" width="400"></a>

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![.NET](https://img.shields.io/badge/.NET-10.0-512BD4)](https://dotnet.microsoft.com/download)
[![Platform](https://img.shields.io/badge/platform-win--x64%20%7C%20win--arm64%20%7C%20linux--x64%20%7C%20osx--arm64-lightgrey)](README.md#cross-platform)

<a href="./docs/badges/showcase.svg"><img src="docs/badges/showcase.svg" alt="Decode showcase" width="100%" max-width="720"></a>

<sub>Built with AI assistance — see <a href="./CREDITS.md">CREDITS.md</a></sub>
<br>
<a href="./CREDITS.md"><img src="docs/badges/deepseek.svg" alt="DeepSeek V4 Flash"></a>
<a href="./CREDITS.md"><img src="docs/badges/opencode.svg" alt="OpenCode TUI"></a>
</div>

**Goal:** The best open-source decoder for iPod Classic/Nano `.ithmb` thumbnail cache files (2005–2010), packaged as a Native AOT plugin for ImageGlass v10. 49 known profiles, 7 decoders with SIMD acceleration (SSE2 + ARM64 NEON), multi-frame support for F-prefix raw files, BGR;15 channel-swap for iPhone formats, PhotoDB/ArtworkDB parser and builder, device-specific format tables, and full roundtrip-proven correctness. Not an iOS 13+ thumbnail decoder — those are handled natively by Apple's software.

A C# Native AOT codec plugin for [ImageGlass v10](https://imageglass.org) that opens Apple `.ithmb` thumbnail-cache files — the format used by iOS devices (iPhones, iPod Touches) and iPods to store photo thumbnails for syncing with iTunes. Two format categories exist:

**T-prefix** — contains an embedded JPEG. ✅ Fully supported.

**F-prefix** (e.g. `F1019_1.ithmb`) — raw uncompressed thumbnails (RGB565, RGB555, UYVY, YCbCr420, CLCL nibble-chroma). ⚠️ Best-effort.

Tested with **956 T-prefix files** from an iPhone 5 (iOS 7) — **100% extraction rate**.<br>
Additionally validated against **227 publicly available T-prefix files** from an iPod Photo Cache (100% JPEG detection rate).

---

## Table of Contents

- [How it works](#how-it-works)
- [Install](#install)
- [Build from source](#build-from-source)
- [Testing & validation](#testing--validation)
- [Development](#development)
- [Architecture](#architecture)
- [Profile Reference](#profile-reference)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)
- [Acknowledgments](#acknowledgments)
- [Changelog](#changelog)
- [License](#license)

---

## How it works

1. **Peek read** — reads the first 4 MB of the file for JPEG scanning, then seeks the exact JPEG byte range from the FileStream (peak memory dominated by the decoded bitmap, typically a few MB for iPhone photos).
2. **JPEG scan** — SIMD-accelerated `Span.IndexOf` (SSE2 on x64, NEON on ARM64) locates a SOI marker (`FF D8`) followed by JFIF or Exif within 512 bytes. On match, the JPEG payload is extracted (SOI→EOI), decoded via StbImageSharp, and its EXIF orientation tag (0x0112) is parsed for auto-rotation in ImageGlass.
3. **Raw fallback** — if no JPEG is found, the decoder matches the first 4 bytes (big-endian prefix) against 49 known profiles and runs the appropriate raw decoder (RGB565, RGB555, UYVY, YCbCr420, YUV422 interlaced, CLCL nibble-chroma, or CL per-pixel chroma) to produce BGRA output. If the prefix doesn't match any known profile, the file is rejected as unrecognized. Additional decoder variants can be activated via `profiles.json`: swapped chroma planes for YCbCr 4:2:0, per-pixel vs shared nibble chroma, endianness toggles, interlaced field ordering, and padded frame handling.

### File size guard

> [!NOTE]
> Files larger than **50 MB** are rejected before reading to prevent out-of-memory (OOM) from pathological input. All known real .ithmb files are under 2 MB; the most extreme theoretical case (48 MP iPhone JPEG) is ~30 MB.

---

## Install

### Requirements

- [ImageGlass v10](https://imageglass.org) or later (Windows 10/11 64-bit)
- An `.ithmb` file from an iOS device photo cache

### Steps

1. Download `IthmbCodec_win-x64.zip` from the [latest release](https://github.com/B67687/ithmb-codec/releases) — it contains `IthmbCodec.dll`, `igplugin.json`, and `profiles.json`.
2. Create the `IthmbCodec` folder if it doesn't exist, then extract to `%LocalAppData%\ImageGlass_10\_plugins\IthmbCodec\`.
3. Verify the folder contains: `IthmbCodec.dll` (1.4 MB Native AOT), `igplugin.json`, `profiles.json`.
4. Restart ImageGlass.

> [!TIP]
> To register `.ithmb` in the Open File dialog, edit `%LocalAppData%\ImageGlass_10\igconfig.json` and add `ithmb` to the `FileFormats` list. Relaunch ImageGlass. Thanks to @d2phap for the fix ([issue #1](https://github.com/B67687/ithmb-codec/issues/1)).

---

## Build from source

### Windows (release binary)

Requires .NET 10 SDK and Visual Studio 2022 with the "Desktop development with C++" workload.

```powershell
dotnet publish src/IthmbCodec/IthmbCodec.csproj -c Release -r win-x64 -p:IlcInstructionSet=base
```

> [!CAUTION]
> `-p:IlcInstructionSet=base` works around a known ILC (Intermediate Language Compiler, the Native AOT compiler) stack buffer overrun in SDK 10.0.301. Builds may crash without this flag.

Output lands in `src/IthmbCodec/bin/Release/net10.0/win-x64/native/`. The publish output already includes `igplugin.json` and `profiles.json` — archive these together for distribution.

### Cross-platform

ImageGlass runs on **Windows only** (10/11 64-bit). Cross-platform builds target other runtimes for testing or integration into other projects. Native AOT cross-compilation requires platform-specific toolchains on the build machine.

| Runtime     | Command                                  | Output   |
| ----------- | ---------------------------------------- | -------- |
| Windows x64 | `dotnet publish -c Release -r win-x64`   | `.dll`   |
| Windows ARM | `dotnet publish -c Release -r win-arm64` | `.dll`   |
| Linux x64   | `dotnet publish -c Release -r linux-x64` | `.so`    |
| macOS ARM   | `dotnet publish -c Release -r osx-arm64` | `.dylib` |

---

## Testing & validation

```bash
dotnet test src/IthmbCodec/test/IthmbCodec.Tests.csproj -c Release
```

**517 tests** across roundtrip (RGB565: 65,536 values, RGB555: 32,768), fuzz (350+ inputs across all 7 decoders), SIMD identity (10 tests), YUV tolerance, parsers, speculative decoder paths (CL, CLCL, rotation, swapped chroma), buffer-too-small guards, trailing-padding tolerance, JPEG carving fallback, multi-frame raw decode, per-decoder determinism + statistical verification, SIMD tail path fuzz (5 tests), and rotation roundtrip.

**Real-device validation:**

- **iPhone 5 (iOS 7):** 956 T-prefix files — 100% extraction
- **Jakarade.com F00-F08:** 227 public T-prefix files — 100% JPEG+EXIF detection
- **FAU.edu F00-F50:** ~500 T-prefix files — unavailable (directory only, downloads return 404)

---

## Development

The plugin was developed through iterative research, implementation, review, and release cycles:

1. **Format survey** — 25 open-source .ithmb implementations found and analyzed
2. **Format table extraction** — iOpenPod (50+ entries), libgpod, iLounge threads, and Keith's iPod Photo Reader provided dimension/encoding tables for 49 profiles
3. **Implementation** — C# Native AOT plugin with 7 decoders and SIMD acceleration (SSE2 + ARM64 NEON)
4. **Testing** — 517 unit tests across roundtrip, fuzz, SIMD identity, YUV tolerance, parsers, speculative paths, buffer-too-small guards, trailing-padding tolerance, JPEG carving fallback, multi-frame raw decode, SIMD tail path fuzz, rotation roundtrip, BGR;15 channel-swap, PhotoDB roundtrip write, PhotoDB integrity, and device-specific format tables
5. **Review cycles** — 5 rounds of multi-agent review: ~47 findings fixed covering memory safety, threading, ABI compatibility, SIMD correctness, rotation buffer overflow, crop integer overflow, and defense-in-depth
6. **Release** — Windows Native AOT binary published via GitHub Releases

<div align="center"><img src="docs/diagrams/pipeline.svg" alt="Development pipeline diagram" width="100%"></div>

See [CHANGELOG.md](CHANGELOG.md) for the full version history.

### Quality pipeline

Quality checks run locally before release: linting, secret scanning, tests, static analysis, dependency audit, and link checking.

## Architecture

**Plugin ABI** — the only C export is `ig_plugin_get_api()`, which returns an `IGPluginApi` → `IGCodecApi` chain following the ImageGlass v10 native codec plugin ABI (v1.0.0.0).

**Source layout** — 12 partial class files organized by domain:

| File | Purpose | Size |
|------|---------|------|
| `IthmbCodecPlugin.cs` | ABI entry point, init, API surface | ~471 lines |
| `IthmbCodecPlugin.DecodePipeline.cs` | Decode dispatch, live buffer mgmt, crop/rotate | ~320 lines |
| `IthmbCodecPlugin.JpegDecode.cs` | JPEG scan, EXIF parsing, StbImageSharp | ~117 lines |
| `IthmbCodecPlugin.ProfileSystem.cs` | Profile lookup, profiles.json parser | ~309 lines |
| `IthmbCodecPlugin.PhotoDb.cs` | PhotoDB/ArtworkDB chunk parser, writer, integrity checker | ~540 lines |
| `IthmbCodecPlugin.DeviceProfiles.cs` | Per-generation iPod device format tables (18 devices) | ~180 lines |
| `IthmbCodecPlugin.EncoderHelpers.cs` | Shared encoder helpers (InterlaceFields, BT.601) | ~92 lines |
| `IthmbCodecPlugin.Rgb565Rgb555.cs` | RGB565/RGB555 decoders + SSE2/NEON SIMD | ~370 lines |
| `IthmbCodecPlugin.UyvyYuv.cs` | UYVY, YCbCr420, YUV422 decoders + SIMD | ~316 lines |
| `IthmbCodecPlugin.ClclCl.cs` | CLCL nibble-chroma, CL per-pixel chroma decoders | ~251 lines |
| `IthmbCodecPlugin.Encoding.cs` | Synthetic encoder for all raw formats | ~310 lines |

**Data flow:**

```
.ithmb file → Peek (4 MB) → JPEG scan → seek JPEG slice → StbImageSharp → BGRA
                                        └→ No JPEG → prefix lookup → raw decoder → BGRA
```

**SIMD acceleration:** RGB565/RGB555 → SSE2 or NEON (x64/ARM64, 4-6× gain), UYVY → SSSE3 or NEON (x64/ARM64, 2-3× gain), YCbCr420 → cross-platform Vector128 (x64 + ARM64 NEON, 3-5× gain). CLCL nibble-chroma is scalar-only. **BGR;15 channel-swap** (`SwapRgbChannels` flag) supported in all decoder variants — SIMD path uses a conditional swap outside the pixel loop (zero per-pixel overhead when inactive).

**Multi-frame support** — F-prefix `.ithmb` files may contain multiple concatenated raw frames (confirmed by Keith's iPod Photo Reader, ithmbrdr, libgpod, and iOpenPod). The codec detects frame count from file size and caches the file for read-once decode-many access. Callers can access individual frames via `frameIndex` (0-based); out-of-range indices return `IGStatus.InvalidArg`. JPEG-embedded T-prefix files are always single-frame.

<div align="center"><img src="docs/diagrams/architecture.svg" alt="Architecture diagram" width="100%"></div>

---

## Tooling

The repository includes several CLI tools under [`tools/`](tools/):

| Tool | Description |
|------|-------------|
| [`IthmbDecoder`](tools/IthmbDecoder/) | Decode .ithmb files or PhotoDB/ArtworkDB entries to BMP; `--list-pd` enumerate PhotoDB entries, `--pd-index N` extract+decode entry N, `--check-pd` validate PhotoDB integrity |
| [`IthmbBenchmark`](tools/IthmbBenchmark/) | [Performance benchmark](***REMOVED***) suite for all 7 decoders |
| [`IthmbSampleGenerator`](tools/IthmbSampleGenerator/) | Generate synthetic .ithmb files for testing |
| [`fetch_jakarade.sh`](tools/fetch_jakarade.sh) | Download T-prefix .ithmb files from jakarade.com |
| [`ithmb2img.sh`](tools/ithmb2img.sh) | Batch convert .ithmb files to images via ImageMagick |
| [`extract_hfsplus.py`](tools/extract_hfsplus.py) | Extract files from iPhone OS 1.x-3.x IPSW root filesystem DMGs |

See [***REMOVED***](***REMOVED***) for full decoder benchmark results.

---

## Profile Reference

**49 known profiles** (22 photo + 26 cover art + 1 new 640×480 cover art) covering iPod Photo 4G through iPhone 2G and iPod Nano 7G. Max frame size: 480×864 (RGB565, 830 KB). See [PROFILES.md](PROFILES.md) for the full table with dimensions, encoding, and device mapping. External profiles can be added at runtime via `profiles.json`.

---

## Limitations

> [!WARNING]
> **Only T-prefix (JPEG-embedded) is validated on real hardware.** Raw decoders exist for 49 known profiles and pass roundtrip tests, multi-frame raw decode is synthetically tested, but no real F-prefix files have been obtained for hardware validation. See [HARDWARE_GUIDE.md](HARDWARE_GUIDE.md) for a hardware validation plan.

- **F-prefix (raw) decoders are best-effort** — roundtrip-tested via synthetic encoder and multi-frame decode tested, but unverified against real iPod/iPhone hardware.
- **JPEG SOI must be within the first 4 MB** of the file (covers all known real files).

---

## Troubleshooting

| Symptom                      | Likely cause / What to do                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Plugin silently doesn't load | Missing `igplugin.json` or filename mismatch. Verify the `_plugins\IthmbCodec\` folder structure.                       |
| File won't open              | May use an unknown format variant. [Open a codec issue](https://github.com/B67687/ithmb-codec/issues) with a sample.    |
| Garbled image / wrong colors | JPEG false positive or raw decoder mismatch (rare). [Open a codec issue](https://github.com/B67687/ithmb-codec/issues). |
| "File too large" error       | File exceeds the **50 MB** guard — should never happen for normal iPhone photos. Open an issue if it does.              |
| Not in Open File dialog      | Add `.ithmb` to `FileFormats` in `igconfig.json`.                                                                       |

> [!TIP]
> If a file doesn't decode correctly, [open an issue](https://github.com/B67687/ithmb-codec/issues) with a sample link. You can also try [ithmb.org](https://ithmb.org) — a browser-based .ithmb decoder (offline, no upload) — to compare results.

---

## Acknowledgments

25 open-source .ithmb implementations were surveyed during development. See [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for the full list of credited projects, sample file sources, academic references, and color conversion standards.

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

---

## License

MIT — see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code.
