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

A C# Native AOT codec plugin for [ImageGlass v10](https://imageglass.org) that opens Apple `.ithmb` thumbnail-cache files — the format used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo and album art thumbnails. Two format categories exist:

**T-prefix** — contains an embedded JPEG. ✅ Fully supported (validated on 1,183 real files).

**F-prefix** (e.g. `F1019_1.ithmb`) — raw uncompressed thumbnails (RGB565, RGB555, UYVY, YCbCr420, CLCL nibble-chroma). ✅ Profiles cross-referenced against iOpenPod's empirically validated set (50+ profiles tested across multiple iPod models) and real iPod Classic 6G samples (F1061/F1055/F1060).

---

## Table of Contents

- [How it works](#how-it-works)
- [Install](#install)
- [Build from source](#build-from-source)
- [Testing & validation](#testing--validation)
- [Performance](#performance)
- [Architecture](#architecture)
- [Tooling](#tooling)
- [Profile Reference](#profile-reference)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)
- [Development](#development)
- [Acknowledgments](#acknowledgments)
- [Changelog](#changelog)
- [License](#license)

---

## How it works

1. **Peek read** — reads the first 4 MB of the file for JPEG scanning, then seeks the exact JPEG byte range from the FileStream (peak memory dominated by the decoded bitmap, typically a few MB for iPhone photos).
2. **JPEG scan** — SIMD-accelerated `Span.IndexOf` (SSE2 on x64, NEON on ARM64) locates a SOI marker (`FF D8`) followed by JFIF or Exif within 512 bytes. On match, the JPEG payload is extracted (SOI→EOI), decoded via StbImageSharp, and its EXIF orientation tag (0x0112) is parsed for auto-rotation in ImageGlass.
3. **Raw fallback** — if no JPEG is found, the decoder matches the first 4 bytes (big-endian prefix) against 49 known profiles and runs the appropriate raw decoder (RGB565, RGB555, UYVY, YCbCr420, YUV422 interlaced, CLCL nibble-chroma, or CL per-pixel chroma) to produce BGRA output. If the prefix doesn't match any known profile, the file is scanned for embedded JPEG markers (byte-level carving) before being rejected. Additional decoder variants can be activated via `profiles.json`: swapped chroma planes for YCbCr 4:2:0, per-pixel vs shared nibble chroma, endianness toggles, interlaced field ordering, padded frame handling, and channel-swap for BGR;15 formats.

4. **PhotoDB/ArtworkDB** — Apple's iPod thumbnail databases (PhotoDB, ArtworkDB) use a binary chunk-based format (MHFD→MHSD→MHNI entries). When a file starts with `mhfd`, the codec parses the chunk tree, extracts individual thumbnails (inline pixel data or external `.ithmb` file references), and decodes each via the raw decoder matching its format ID. Read, write, and integrity checking are all supported.

### File size guard

> [!NOTE]
> Files larger than **32 MB** are rejected before reading to prevent OOM/DoS from pathological input. All known real .ithmb files are under 1 MB (max observed: 852 KB). The 32 MB limit covers ~40 max-size raw frames — far beyond any realistic thumbnail cache. Researched from scratch: no evidence that libgpod's commonly-cited 256 MB limit is a real firmware constant.

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

**530 tests** across roundtrip (RGB565: 65,536 values, RGB555: 32,768), fuzz (350+ inputs across all 7 decoders + 1,000 random byte mutations), SIMD identity (10 tests), YUV tolerance, parsers, speculative decoder paths (CL, CLCL, rotation, swapped chroma), buffer-too-small guards, trailing-padding tolerance, JPEG carving fallback, multi-frame raw decode, per-decoder determinism + statistical verification, SIMD tail path fuzz (5 tests), rotation roundtrip, BGR;15 channel-swap, PhotoDB roundtrip write/integrity/JPEG blob decode, and device-specific format tables, corruption fuzz, and format ID profile tests.

**Real-device validation:**

- **iPod Classic 6G (Reuhno):** Real F1061/F1055/F1060 .ithmb files decoded successfully (BGR;15 channel-swap, MSB replication — both confirmed correct). 30 reference PNGs match decoder output.
- **iOpenPod (TheRealSavi):** Empirically validated 50+ profiles across multiple iPod models purchased and tested. Confirmed "no known issues for iPod Nano and iPod Classic models." Our 49 profiles derive from the same format ID sources — hardware validation covered by iOpenPod's testing. See [iOpenPod#140](https://github.com/TheRealSavi/iOpenPod/issues/140).
- **iPhone 5 (iOS 7):** 956 T-prefix files — 100% extraction
- **Jakarade.com F00-F08:** 227 public T-prefix files — 100% JPEG+EXIF detection
- **MVS CTF 2026 (iOS 18):** iPhone 14 Plus full filesystem image scanned — 3 `.ithmb` files found but use a different proprietary format (iOS Photos framework, not decodable by this codec)

> Other known sources (FAU.edu, ~500 files) have live directory listings but downloads return 404. Not available for testing.

---

## Development

The plugin was developed through iterative research, implementation, review, and release cycles:

1. **Format survey** — 25 open-source .ithmb implementations found and analyzed
2. **Format table extraction** — iOpenPod (50+ entries), libgpod, iLounge threads, and Keith's iPod Photo Reader provided dimension/encoding tables for 49 profiles
3. **Implementation** — C# Native AOT plugin with 7 decoders and SIMD acceleration (SSE2 + ARM64 NEON)
4. **Testing** — 530 unit tests across roundtrip, fuzz, SIMD identity, YUV tolerance, parsers, speculative paths, buffer-too-small guards, trailing-padding tolerance, JPEG carving fallback, multi-frame raw decode, SIMD tail path fuzz, rotation roundtrip, byte-level corruption fuzz, BGR;15 channel-swap, PhotoDB roundtrip write, PhotoDB integrity, PhotoDB JPEG blob decode, device-specific format tables, and format ID profile tests
5. **Review cycles** — 5 rounds of multi-agent review: ~47 findings fixed covering memory safety, threading, ABI compatibility, SIMD correctness, rotation buffer overflow, crop integer overflow, and defense-in-depth
6. **Release** — Windows Native AOT binary published via GitHub Releases

<div align="center"><img src="docs/diagrams/pipeline.svg" alt="Development pipeline diagram" width="100%"></div>

See [CHANGELOG.md](CHANGELOG.md) for the full version history.

### Quality pipeline

Quality checks run locally before release: linting, secret scanning, tests, static analysis, dependency audit, and link checking.

## Architecture

**Plugin ABI** — the only C export is `ig_plugin_get_api()`, which returns an `IGPluginApi` → `IGCodecApi` chain following the ImageGlass v10 native codec plugin ABI (v1.0.0.0).

**Source layout** — 16 files organized by domain (previously 11, split for single-responsibility):

| File | Purpose | Size |
|------|---------|------|
| `IthmbCodecPlugin.cs` | ABI entry point, init, API surface | ~471 lines |
| `IthmbCodecPlugin.DecodePipeline.cs` | Decode dispatch, live buffer mgmt, crop/rotate | ~320 lines |
| `IthmbCodecPlugin.JpegDecode.cs` | JPEG scan, EXIF parsing, StbImageSharp | ~117 lines |
| `IthmbCodecPlugin.ProfileSystem.cs` | Profile lookup, profiles.json parser | ~309 lines |
| `IthmbCodecPlugin.VariantProfile.cs` | IthmbVariantProfile + IthmbEncoding types | ~38 lines |
| `PhotoDb/Core.cs` | PhotoDB/ArtworkDB chunk parser, walker, data model | ~294 lines |
| `PhotoDb/Serialization.cs` | PhotoDB writer (TryBuildPhotoDb) + integrity checker | ~235 lines |
| `IthmbCodecPlugin.DeviceProfiles.cs` | Per-generation iPod device format tables (18 devices) | ~173 lines |
| `IthmbCodecPlugin.EncoderHelpers.cs` | Shared encoder helpers (InterlaceFields, BT.601) | ~77 lines |
| `IthmbCodecPlugin.Rgb565Rgb555.cs` | RGB565/RGB555 decoders + SSE2/NEON SIMD (shared params) | ~303 lines |
| `IthmbCodecPlugin.UyvyYuv.cs` | UYVY, YCbCr420, YUV422 decoders + SIMD | ~265 lines |
| `IthmbCodecPlugin.DecodeFormatClcl.cs` | CLCL nibble-chroma decoder | ~80 lines |
| `IthmbCodecPlugin.DecodeFormatCl.cs` | CL per-pixel chroma decoder | ~60 lines |
| `IthmbCodecPlugin.DecodeFormatYcbcr420.cs` | YCbCr420 decoder | ~60 lines |
| `IthmbCodecPlugin.YuvUtils.cs` | Shared YUV pixel helpers (WriteYuvPixel) | ~20 lines |
| `IthmbCodecPlugin.Encoding.cs` | Synthetic encoder for all raw formats | ~310 lines |

**Data flow:**

```
.ithmb file → Peek (4 MB) → JPEG scan → seek JPEG slice → StbImageSharp → BGRA
                                         └→ No JPEG → prefix lookup → raw decoder → BGRA
                                                        └→ JPEG carving → StbImageSharp → BGRA
                                                         (byte-level SOI scan before rejection)

PhotoDB/ArtworkDB → mhfd detect → chunk parser → MHNI entries → raw decoder per format_id → BGRA
                                                              → external .ithmb file → raw decoder → BGRA
```

**SIMD acceleration:** RGB565/RGB555 → SSE2 or NEON (x64/ARM64, 4-6× gain), UYVY → SSSE3 or NEON (x64/ARM64, 2-3× gain), YCbCr420 → cross-platform Vector128 (SSE2 on x64 + NEON on ARM64, 3-5× gain). CLCL nibble-chroma is scalar-only. **BGR;15 channel-swap** (`SwapRgbChannels` flag) supported in all decoder variants — SIMD uses a conditional branch outside the pixel loop (zero per-pixel overhead when inactive). NEON CI validated on native ARM64 runners.

**Multi-frame support** — F-prefix `.ithmb` files may contain multiple concatenated raw frames (confirmed by Keith's iPod Photo Reader, ithmbrdr, libgpod, and iOpenPod). The codec detects frame count from file size and caches the file for read-once decode-many access. Callers can access individual frames via `frameIndex` (0-based); out-of-range indices return `IGStatus.InvalidArg`. JPEG-embedded T-prefix files are always single-frame.

<div align="center"><img src="docs/diagrams/architecture.svg" alt="Architecture diagram" width="100%"></div>

---

## Tooling

The repository includes several CLI tools under [`tools/`](tools/):

| Tool | Description |
|------|-------------|
| [`IthmbDecoder`](tools/IthmbDecoder/) | Decode .ithmb files or PhotoDB/ArtworkDB entries to BMP. Flags: `--list-pd` enumerate entries, `--pd-index N` extract+decode entry N, `--extract-all-pd` extract all entries, `--check-pd` validate PhotoDB integrity, `--list-devices` show device format tables |
| [`IthmbSampleGenerator`](tools/IthmbSampleGenerator/) | Generate synthetic .ithmb files for testing |
| [`fetch_jakarade.sh`](tools/fetch_jakarade.sh) | Download T-prefix .ithmb files from jakarade.com |
| [`ithmb2img.sh`](tools/ithmb2img.sh) | Batch convert .ithmb files to images via ImageMagick |
| [`extract_hfsplus.py`](tools/extract_hfsplus.py) | Extract files from iPhone OS 1.x-3.x IPSW root filesystem DMGs |

## Performance

| Decoder | Encoding | Width | Height | Mean time | Allocations |
|---------|----------|-------|--------|-----------|-------------|
| RGB565 | 16-bit RGB | 720 | 480 | 64.19 µs | 0 B |
| RGB555 | 15-bit RGB | 720 | 480 | 65.90 µs | 0 B |
| UYVY | YUV 4:2:2 | 720 | 480 | 189.61 µs | 0 B |
| YCbCr 4:2:0 | YUV 4:2:0 | 720 | 480 | 221.36 µs | 0 B |
| YUV422 Interlaced | YUV 4:2:2 (fields) | 720 | 480 | 190.22 µs | 0 B |
| CLCL | Nibble-chroma YUV | 720 | 480 | 456.80 µs | 0 B |
| CL | Per-pixel chroma YUV | 720 | 480 | 590.66 µs | 0 B |

All decoders produce BGRA 8-bit output. Zero heap allocations — output is written directly to caller-provided native buffers.

---

## Profile Reference

**49 known profiles** (22 photo + 26 cover art + 1 new 640×480 cover art) covering iPod Photo 4G through iPhone 2G and iPod Nano 7G. Max frame size: 480×864 (RGB565, 830 KB). See [PROFILES.md](PROFILES.md) for the full table with dimensions, encoding, and device mapping. External profiles can be added at runtime via `profiles.json`.

---

## Limitations

> [!WARNING]
> **T-prefix (JPEG-embedded) validated on 227 real files; F-prefix raw decoders validated on iPod Classic 6G samples (F1061/F1055/F1060).** Raw decoders exist for 49 known profiles and pass roundtrip tests (530 total). Multi-frame raw decode is synthetically tested. Profiles are cross-referenced against iOpenPod's empirically validated set (50+ profiles, tested across multiple iPod models). BGR;15 format identified from Steee29/ithmb_converter analysis and confirmed via Reuhno's real samples. See [HARDWARE_GUIDE.md](HARDWARE_GUIDE.md) for details.

- **F-prefix decoder coverage is broad but not exhaustive** — 49 profiles cover known iPod/iPhone formats through iPod Nano 7G and iPhone 2G. Profiles are cross-referenced against iOpenPod's empirically validated set. Unknown formats from obscure firmware versions may still exist — [open an issue](https://github.com/B67687/ithmb-codec/issues) if you encounter one.
- **JPEG SOI must be within the first 4 MB** of the file (covers all known real files). For unknown raw files, the codec falls back to byte-level JPEG carving.

---

## Troubleshooting

| Symptom                      | Likely cause / What to do                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Plugin silently doesn't load | Missing `igplugin.json` or filename mismatch. Verify the `_plugins\IthmbCodec\` folder structure.                       |
| File won't open              | May use an unknown format variant. [Open a codec issue](https://github.com/B67687/ithmb-codec/issues) with a sample.    |
| Garbled image / wrong colors | JPEG false positive or raw decoder mismatch (rare). [Open a codec issue](https://github.com/B67687/ithmb-codec/issues). |
| "File too large" error       | File exceeds the **32 MB** guard — should never happen for normal iPhone photos. Open an issue if it does.              |
| Not in Open File dialog      | Add `.ithmb` to `FileFormats` in `igconfig.json`.                                                                       |

> [!TIP]
> If a file doesn't decode correctly, [open an issue](https://github.com/B67687/ithmb-codec/issues) with a sample link. You can also try [ithmb.org](https://ithmb.org) — a browser-based .ithmb decoder (offline, no upload) — to compare results.

---

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

See [ACKNOWLEDGMENTS.md](ACKNOWLEDGMENTS.md) for the full list (35+ projects, sample file sources, academic references, and color conversion standards).

---

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for version history.

---

## License

MIT — see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code.
