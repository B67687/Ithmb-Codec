# ITHMB Codec for ImageGlass v10

A Native AOT C# codec plugin for [ImageGlass v10](https://imageglass.org) that opens Apple `.ithmb` thumbnail-cache files. Primarily works by locating embedded JPEG payloads inside `.ithmb` files and decoding them via StbImageSharp. Also includes SIMD-accelerated decoders (SSE2/SSSE3/Vector128) for 47 profiles (22 photo + 25 cover art) covering iPod Photo through iPhone 2G, aggregated from iOpenPod, libgpod, Keith's iPod Photo Reader, and the original iLounge format specification thread.

Tested with **956 T####.ithmb files** from an iPhone 5 (iOS 7) iPod Photo Cache --- **100% extraction rate**. Additionally validated against **228 publicly available T-prefix files** from an iPod Photo Cache (100% JPEG detection rate).

---

## Table of Contents

- [How it works](#how-it-works)
- [Install](#install)
- [Build from source](#build-from-source)
- [Architecture](#architecture)
- [Raw profile definitions](#raw-profile-definitions)
- [Verified devices and formats](#verified-devices-and-formats)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)
- [References and Acknowledgments](#references-and-acknowledgments)
- [License](#license)

---

## How it works

`.ithmb` files (iThumbnail cache) are a proprietary format used by Apple iOS devices to store photo thumbnails. Two broad categories exist:

| Type                                | Description                                                                                                                                                     | Our support                                                                                                                                                                           |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **T-prefix** (e.g. `T####.ithmb`)   | Contains a single full-resolution photo as an embedded JPEG (JFIF or Exif). These are found in newer iOS device caches (iPhone 5 and later).                    | ✅ **Fully supported** --- the primary path. 956/956 verified.                                                                                                                        |
| **F-prefix** (e.g. `F1019_1.ithmb`) | Older format used by iPods and early iPhones. Contains multiple raw-format thumbnails concatenated together (RGB565, YUV422, YCbCr420). These are uncompressed. | ⚠️ Best-effort decoders exist for 47 known profiles (22 photo + 25 cover art). Roundtrip-tested via synthetic encoder (317 tests). See [raw profile table](#raw-profile-definitions). |

<img src="docs/pipeline.svg" alt="Decode pipeline diagram" width="800">

### Decode pipeline

1. **Read the file** --- a 4 MB header is read for JPEG scan, then the exact JPEG slice is seeked and read from the FileStream. Peak memory: ~5 MB for typical files.
2. **JPEG scan** --- the file is scanned (SIMD-accelerated via `Span.IndexOf`) for a JPEG SOI marker (`FF D8`) followed within 512 bytes by either a JFIF or Exif header. If found, the JPEG payload is extracted (SOI to EOI) and decoded via StbImageSharp (MIT, ~200 KB).
3. **Raw fallback** --- if no embedded JPEG is found, the first 4 bytes are read as a big-endian integer prefix and checked against known profiles. On match, the appropriate raw decoder (RGB565, RGB555, UYVY, or YCbCr420) is used. The YUV422 decoder handles both linear (UYVY) and interlaced (F1019: even/odd rows in separate fields) layouts. A **CLCL nibble-chroma** decoder is also included for iPod 4G/5G files (chroma packed as 4-bit values, scaling confirmed by andrewmalta/ithmb).
4. **EXIF orientation** --- if the JPEG contains an EXIF APP1 segment with an orientation tag (0x0112), it is parsed and reported to the host. ImageGlass rotates the image accordingly.

### File size guard

Files larger than **100 MB** are rejected before reading to prevent OOM from pathological input.

---

## Install

### Requirements

- [ImageGlass v10](https://imageglass.org) Beta 2 or later (Windows 10/11 64-bit)
- An `.ithmb` file from an iOS device photo cache

### Steps

1. Download `IthmbCodec_win-x64.zip` from the [latest release](https://github.com/B67687/ithmb-codec/releases).
2. Extract the contents to `%LocalAppData%\ImageGlass_10\_plugins\IthmbCodec\`.

   The folder should contain:

   ```
    %LocalAppData%\ImageGlass_10\_plugins\IthmbCodec\
        IthmbCodec.dll        (1.4 MB --- native plugin with embedded JPEG decoder)
        igplugin.json         (plugin manifest)
        profiles.json         (optional --- external profile definitions)
   ```

   > StbImageSharp (JPEG decoder) is compiled directly into `IthmbCodec.dll` by Native AOT. No separate DLL needed. This replaced the previous 11 MB `libSkiaSharp.dll` dependency (85% size reduction).

3. Restart ImageGlass v10.
4. Drag any `.ithmb` file into the ImageGlass window.

> **Note:** ImageGlass v10 Beta 2 does not register `.ithmb` in the file-open dialog. Drag-and-drop works. This is a known limitation of the beta.

---

## Build from source

### Windows (release binary)

Requires .NET 10 SDK and Visual Studio 2022 with the "Desktop development with C++" workload (for Native AOT).

```powershell
# Clone SDK dependency (once)
git clone https://github.com/ImageGlass/SDK.git imageglass-sdk --depth 1

# Publish as Native AOT shared library
# Note: -p:IlcInstructionSet=base works around an ILC stack buffer overrun in SDK 10.0.301
dotnet publish src/IthmbCodec/IthmbCodec.csproj -c Release -r win-x64 -p:IlcInstructionSet=base
```

Output lands in `src/IthmbCodec/bin/Release/net10.0/win-x64/native/`. To package for distribution:

```powershell
cd src/IthmbCodec
Copy-Item igplugin.json,profiles.json bin/Release/net10.0/win-x64/native/
Compress-Archive -Path bin/Release/net10.0/win-x64/native/* -DestinationPath IthmbCodec_win-x64.zip
```

### Cross-platform

Native AOT cross-compilation is not supported. You must build on each target platform:

| Target      | Command                                  | Output             |
| ----------- | ---------------------------------------- | ------------------ |
| Windows x64 | `dotnet publish -c Release -r win-x64`   | `IthmbCodec.dll`   |
| Windows ARM | `dotnet publish -c Release -r win-arm64` | `IthmbCodec.dll`   |
| Linux x64   | `dotnet publish -c Release -r linux-x64` | `IthmbCodec.so`    |
| macOS ARM   | `dotnet publish -c Release -r osx-arm64` | `IthmbCodec.dylib` |

### Running tests

```bash
dotnet test src/IthmbCodec/test/IthmbCodec.Tests.csproj -c Release
```

Tests cover: exhaustive RGB565 (65,536 values, **pixel-perfect roundtrip**) + RGB555 (32,768), 250 fuzz tests across 5 decoders, YUV422/YCbCr420 roundtrip (gradient, ±5 tolerance), JPEG slice detection, EXIF orientation, SIMD correctness, memory safety, property invariants, JSON parser, real-file validation (**317 tests total**).

---

## Architecture

### Plugin ABI

The plugin follows the ImageGlass v10 native codec plugin ABI (v1.0.0.0):

```
ig_plugin_get_api() -> IGPluginApi -> GetCodec() -> IGCodecApi
```

- **Source tree** — two partial class files:
  - `IthmbCodecPlugin.cs` — ABI, init, JPEG pipeline, EXIF parsing, JSON profile loader
  - `IthmbCodecPlugin.Decoding.cs` — all decode algorithms + SIMD acceleration
- **Profile-driven dispatch** — format ID → (width, height, encoding, flags) → appropriate decoder
- **JPEG path** (primary) — TryFindJpegSlice → StbImageSharp decode → BGRA output
- **Raw path** (fallback) — prefix lookup → one of 4 decoders (RGB565, RGB555, YUV422, YCbCr420)

### Data flow

```
.ithmb file
  → Peek read (4 MB header for JPEG scan)
  → TryFindJpegSlice (SIMD Span.IndexOf)
  ├── JPEG found → seek + read slice → StbImageSharp → BGRA
  └── No JPEG    → read full file → prefix lookup → raw decoder → BGRA
```

## Pipeline

<img src="docs/pipeline.svg" alt="Pipeline diagram" width="800">

The plugin was developed through an iterative research-and-review pipeline:

1. **Format survey** — 25 open-source .ithmb implementations found across GitHub, GitLab, Codeberg, SourceHut, Bitbucket, and Gitee. Complete source analysis of each.
2. **Format table extraction** — iOpenPod (50+ entries), libgpod, iLounge threads, and Keith's iPod Photo Reader provided dimension/encoding tables for 47 profiles.
3. **Implementation** — C# Native AOT plugin with 5 decoders + 3 SIMD engines (SSE2, SSSE3, Vector128). ~931 lines ABI + ~613 lines decoding. Standalone CLI decoder at `tools/IthmbDecoder/`.
4. **Testing** — 317 unit tests (exhaustive 65K-pattern, 250 fuzz, SIMD-vs-scalar, roundtrip, property invariants, real-file validation with 228 public samples). Includes lossless roundtrip for RGB565 (65,536 values, pixel-perfect) and RGB555 (32,768 values, pixel-perfect), and lossy YUV roundtrip for UYVY and YCbCr420 (gradient, ±5 tolerance).
5. **Review cycles** — 4 rounds of 5-agent adversarial review. ~42 findings fixed covering memory safety, threading, ABI compatibility, SIMD correctness, and defense-in-depth hardening.
6. **Release** — Windows binary (1.4 MB native AOT), published to GitHub Releases.
7. **Documentation** — All 25 references credited in README.

<img src="docs/architecture.svg" alt="Architecture diagram" width="800">

### Key design decisions

- **Single entry point** (`ig_plugin_get_api`) --- the only C export.
- **Double-checked locking with `volatile`** in `GetApi` for thread-safe initialization (ARM64-safe).
- **Single codec**, single-frame static raster decoder.
- **Memory ownership**: the plugin allocates pixel buffers; the host calls back into `FreePixelBuffer` to release them (thread-safe).
- **SIMD acceleration**: RGB565 uses SSE2 (4-6× gain), YCbCr420 uses cross-platform Vector128 (3-5× gain, x64 + ARM64 NEON), UYVY uses SSSE3+SSE2 (2-3× gain). UYVY interlaced and non-interlaced share a common SIMD inner loop via `ProcessUyvyRow`. The **CLCL nibble-chroma** decoder is scalar-only (nibble scaling confirmed by andrewmalta/ithmb).

### Key source files

| File                                          | Description                                                                                                                                        |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/IthmbCodec/IthmbCodecPlugin.cs`          | Plugin ABI, init, JPEG pipeline, EXIF parsing, JSON profile loader (~931 lines)                                                                    |
| `src/IthmbCodec/IthmbCodecPlugin.Decoding.cs` | Decode algorithms + SIMD (SSE2/SSSE3/Vector128) for RGB565, RGB555, UYVY, YCbCr420 (~613 lines)                                                    |
| `src/IthmbCodec/IthmbCodecPlugin.Encoding.cs` | Synthetic file encoder for all 5 raw formats (RGB565, RGB555, UYVY, YCbCr420, CLCL) — used by roundtrip tests                                      |
| `src/IthmbCodec/IthmbCodec.csproj`            | .NET 10 Native AOT project targeting `win-x64`, `win-arm64`, `linux-x64`, `osx-arm64`                                                              |
| `tools/IthmbDecoder/`                         | Standalone CLI decoder — decodes .ithmb files through the full plugin pipeline, writes BMP output                                                  |
| `src/IthmbCodec/igplugin.json`                | Plugin manifest consumed by ImageGlass on startup                                                                                                  |
| `src/IthmbCodec/profiles.json`                | External profile definitions (sidecar, merged on first decode, overridable without recompile)                                                      |
| `src/IthmbCodec/test/`                        | xUnit test project (317 tests) --- exhaustive RGB565+RGB555 roundtrip (65K+32K values), SIMD-vs-scalar, 250 fuzz, YUV roundtrip, EXIF, JSON parser |

### Raw profile definitions

47 profiles are defined based on known iPod/iPhone thumbnail and album art formats, aggregated from iOpenPod (50+ entries), libgpod, Keith's iPod Photo Reader, and the original iLounge format specification thread. Additional profiles can be added at runtime via an external `profiles.json` sidecar file (shipped with the plugin, no recompile needed).

> **Note:** iOS 1.x firmwares used slightly different dimensions for some iPhone format IDs (e.g., 3004=55×55, 3009=120×160, 3011=75×75 per [Steee29/ithmb_converter](https://github.com/Steee29/ithmb_converter)). Our dimensions target iPhone 2G+ (per libgpod). If your iOS 1.x files fail to decode, try adjusting the dimensions via `profiles.json`.

| Profile | Resolution | Encoding    | Device(s)                               |
| ------- | ---------- | ----------- | --------------------------------------- |
| 1007    | 480×864    | RGB565      | iPod nano 7G (swapped dimensions)       |
| 1005    | 80×80      | RGB565      | iPod Nano 7G (photo thumbnail)          |
| 1009    | 42×30      | RGB565      | iPod Photo 4G (smallest thumbnail)      |
| 1010    | 240×240    | RGB565      | Nano 7G (cover art large)               |
| 1013    | 220×176    | RGB565 BE   | iPod Photo 4G (full-screen, big-endian) |
| 1015    | 130×88     | RGB565      | iPod Photo 4G (slideshow browser)       |
| 1016    | 140×140    | RGB565      | iPod Photo 4G (cover art)               |
| 1017    | 56×56      | RGB565      | iPod Photo 4G (cover art)               |
| 1019    | 720×480    | YUV422      | iPod Photo/Video (TV-out, interlaced)   |
| 1020    | 176×220    | RGB565 BE   | iPod (portrait thumb, BE, rotated)      |
| 1023    | 176×132    | RGB565 BE   | iPod Nano 1G/2G (landscape, big-endian) |
| 1024    | 320×240    | RGB565      | iPod Classic 5G/6G (full-screen)        |
| 1027    | 100×100    | RGB565      | Nano/Classic (cover art)                |
| 1028    | 100×100    | RGB565      | iPod Video 5G (cover art)               |
| 1029    | 200×200    | RGB565      | iPod Video 5G (cover art)               |
| 1031    | 42×42      | RGB565      | iPod Nano (album art small)             |
| 1032    | 42×37      | RGB565      | iPod Nano 1G/2G (photo list thumb)      |
| 1036    | 50×41      | RGB565      | iPod Classic (smallest thumbnail)       |
| 1055    | 128×128    | RGB565      | Classic/Nano3G/Nano4G (cover art)       |
| 1056    | 128×128    | RGB565      | Nano 5G (cover art)                     |
| 1060    | 320×320    | RGB565      | Classic/Nano3G (cover art)              |
| 1061    | 56×56      | RGB565      | Classic (cover art small)               |
| 1066    | 64×64      | RGB565      | iPod Classic 6G (square photo)          |
| 1067    | 720×480    | YCbCr 4:2:0 | iPod Classic 6G / Nano 3G (padded)      |
| 1068    | 128×128    | RGB565      | Classic/Nano (cover art variant)        |
| 1071    | 240×240    | RGB565      | Nano 4G (cover art large)               |
| 1073    | 240×240    | RGB565      | Nano 5G/6G (cover art large)            |
| 1074    | 50×50      | RGB565      | Nano 4G/5G/6G (cover art xsmall)        |
| 1078    | 80×80      | RGB565      | Nano 4G/5G (cover art small)            |
| 1079    | 80×80      | RGB565      | iPod Nano 4G (photo)                    |
| 1083    | 240×320    | RGB565      | iPod Nano 4G (photo)                    |
| 1084    | 240×240    | RGB565      | Nano 4G (cover art alt)                 |
| 1085    | 88×88      | RGB565      | Nano 6G (cover art medium)              |
| 1087    | 384×384    | RGB565      | iPod Nano 5G (photo)                    |
| 1089    | 58×58      | RGB565      | Nano 6G (cover art small)               |
| 1092    | 80×80      | RGB565      | iPod Nano 6G (photo thumbnail)          |
| 1093    | 512×512    | RGB565      | iPod Nano 6G (full-screen photo)        |
| 2002    | 50×50      | RGB565 BE   | iPod Mobile / Motorola ROKR (cover art) |
| 2003    | 150×150    | RGB565 BE   | iPod Mobile / Motorola ROKR (cover art) |
| 3001    | 256×256    | RGB555      | iPod touch (cover art large)            |
| 3002    | 128×128    | RGB555      | iPod touch (cover art medium)           |
| 3003    | 64×64      | RGB555      | iPod touch (cover art small)            |
| 3004    | 56×55      | RGB555      | iPhone 1G/2G, iPod Touch (photo thumb)  |
| 3005    | 320×320    | RGB555      | iPod touch (cover art xlarge)           |
| 3008    | 640×480    | RGB555      | iPhone 1G/2G, iPod Touch (full-screen)  |
| 3009    | 160×120    | RGB555      | iPhone 1G/2G, iPod Touch (photo prev)   |
| 3011    | 80×79      | RGB555      | iPhone 1G/2G, iPod Touch (photo thumb)  |

### EXIF orientation parsing

The codec parses TIFF IFD0 tag 0x0112 from the JPEG APP1 segment and sets `outInfo->Orientation` (1-8). ImageGlass uses this to auto-rotate the display. Additional EXIF metadata (camera model, GPS, etc.) is preserved in the JPEG bytes and may be extracted independently by the host.

---

## Verified devices and formats

| Device / Source      | Files tested              | Result                                    |
| -------------------- | ------------------------- | ----------------------------------------- |
| iPhone 5 (iOS 7)     | 956 T####.ithmb           | 100% --- all yield valid JPEG             |
| Jakarade.com F00-F08 | 227 T-prefix .ithmb       | 100% --- all contain embedded JPEG + EXIF |
| FAU.edu F00-F50      | ~500 T-prefix (directory) | Listed but files return 404 on download   |

If you test this plugin with a different device or iOS version, please open an issue with sample files (or a link to them).

---

## Limitations

1. **Only T-prefix `.ithmb` files with embedded JPEG** --- this is the primary tested path. Other `.ithmb` variants may not work.
2. **Legacy raw profiles are unverified on real hardware** --- the decoders exist (RGB565, RGB555, UYVY, YCbCr420, CLCL nibble-chroma) and are roundtrip-tested via synthetic encoder (317 tests), but no real-device F-prefix sample files were available for hardware validation. Community contributions of unknown profiles can be added via `profiles.json` without recompiling.
3. **No open-file dialog** --- ImageGlass v10 Beta 2 doesn't register `.ithmb` for the file-open dialog. Use drag-and-drop.
4. **No folder browsing** --- third-party extensions can't be registered for folder navigation in Beta 2.
5. **Single-frame only** --- `.ithmb` files contain a single image per file. No animation/multi-frame support.

---

## Troubleshooting

| Symptom                      | Likely cause                                                                                                     |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Plugin silently doesn't load | Missing `igplugin.json` in the plugin folder, or `executable` in the manifest doesn't match the `.dll` filename. |
| File won't open              | The `.ithmb` file may not contain an embedded JPEG. Try [ithmb.org](https://ithmb.org) to verify.                |
| Garbled image                | The JPEG extraction found a false positive SOI marker. This is rare but possible with unusual files.             |
| "File too large" error       | The file exceeds the 100 MB size guard. This should never happen for normal iPhone photos.                       |
| Crash on close               | Report as an issue. The `FreePixelBuffer` is thread-safe, but other edge cases may exist.                        |

If the plugin doesn't work for your files, try [ithmb.org](https://ithmb.org) --- a free browser-based `.ithmb` decoder with broader device support. No upload required.

---

---

## References and Acknowledgments

Every known open-source `.ithmb` implementation (25 total) was surveyed across GitHub, GitLab, Codeberg, SourceHut, Bitbucket, Gitee, and SourceForge.

### Directly incorporated (MIT-licensed)

| Project                                                                        | Author(s) | What it contributed                                                                              |
| ------------------------------------------------------------------------------ | --------- | ------------------------------------------------------------------------------------------------ |
| [**iOpenPod**](https://github.com/TheRealSavi/iOpenPod)                        | Savi      | Most complete modern codec (2026). 50+ format entries, encode + decode for all iPod generations. |
| [**ithmbrdr**](https://github.com/cyianor/ithmbrdr)                            | cyianor   | F1067 YCbCr 4:2:0 with correct BT.601 coefficients; padded frame structure.                      |
| [**B67687/ithmb-codec**](https://github.com/B67687/ithmb-codec) (this project) | B67687    | C# Native AOT ImageGlass plugin. JPEG-embedded path (956/956).                                   |

### Clean-room format reference (no code copied; format specs are factual discoveries)

| Project                                                                                                                          | Author(s)          | What it contributed                                                                                                                                                                                                      |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [**Keith's iPod Photo Reader**](https://github.com/kebwi/Keiths_iPod_Photo_Reader)                                               | Keith Wiley        | Original 2005 RE. 13 decode methods. [iLounge thread](https://web.archive.org/web/20191225184817/https://forums.ilounge.com/threads/hacking-ithmb-file-format.110066/) documents YUV 4:2:2 interlaced with working code. |
| [**iLounge "Gory Details" thread**](https://web.archive.org/web/20090120040252/http://forums.ilounge.com/showthread.php?t=66435) | jhollington        | Complete per-device format ID table (2005).                                                                                                                                                                              |
| [**andrewmalta/ithmb**](https://github.com/andrewmalta/ithmb)                                                                    | Andrew Malta       | Python decoder confirming F1019 CLCL packed-chroma layout.                                                                                                                                                               |
| [**Gaurav-Phogat/F1007**](https://github.com/Gaurav-Phogat/ithmb-extractor-F1007)                                                | Gaurav Phogat      | F1007 RGB565 at 480×864 with MSB-replication scaling.                                                                                                                                                                    |
| [**keyj.emphy.de blog**](https://web.archive.org/web/2024*/https://keyj.emphy.de/an-ipod-hackers-diary/)                         | Jeff Luyten (KeyJ) | ArtworkDB RE: F1027/F1031 mandatory filenames, RGB565 byte-swapped artwork.                                                                                                                                              |
| [**worstje/repear**](https://github.com/worstje/repear)                                                                          | worstje            | Python ArtworkDB writer with complete format→dimension encoder table.                                                                                                                                                    |
| [**tbutter/podsyncr**](https://github.com/tbutter/podsyncr)                                                                      | tbutter            | iPod Nano 2G photo syncer (2006). Writes F1023/F1032 with configurable endianness.                                                                                                                                       |
| [**libgpod/gtkpod**](https://github.com/gtkpod/libgpod)                                                                          | gtkpod team        | C library, 22 format variants, complete ArtworkDB/PhotoDB parser. 22 years of Linux distribution.                                                                                                                        |
| [**shinyquagsire23 gist**](https://gist.github.com/shinyquagsire23/5ac38487b4c8f9252e78e0275814c90b)                             | shinyquagsire23    | C code for iPod Nano 6G Photo DB reading confirming F1093 = 512×512 RGB565 decode.                                                                                                                                       |
| [**Steee29/ithmb_converter**](https://github.com/Steee29/ithmb_converter)                                                        | Steee29            | Python iOS 1.x converter. Format table: 3004=55×55, 3009=120×160, 3011=75×75 — differs from our libgpod-sourced dimensions, suggesting firmware version variation.                                                       |
| [**wrinklykong/pyithmb**](https://github.com/wrinklykong/pyithmb)                                                                | wrinklykong        | iPod nano CLCL nibble-chroma decoder confirming the Keith Wiley packing method.                                                                                                                                          |
| [**thomas-alrek/iPod-photo-database**](https://github.com/thomas-alrek/iPod-photo-database)                                      | thomas-alrek       | Node.js Photo Database parser with ithmb → JPEG conversion.                                                                                                                                                              |
| [**epireyn/ithmb-rs**](https://gitlab.com/epireyn/ithmb-rs)                                                                      | epireyn            | Rust implementation supporting profiles 1024, 1066, 1067 (iPod 6G). GPLv3.                                                                                                                                               |
| [**Keipydesu/ipod-convert**](https://github.com/Keipydesu/ipod-convert)                                                          | Keipydesu          | Python converter, profiles 1066/1067 with F1067 padded YCbCr support. MIT.                                                                                                                                               |
| [**devm18426/mhfd_extractor**](https://github.com/devm18426/mhfd_extractor)                                                      | devm18426          | Python MHFD chunk parser confirming UYVY interlaced storage format for F1019.                                                                                                                                            |
| [**moerdowo/Minpod**](https://github.com/moerdowo/Minpod)                                                                        | moerdowo           | Swift iPod sync tool. Creates .ithmb files for album art via ArtworkDB. MIT.                                                                                                                                             |
| [**atimevil/Ithmb-Converter**](https://github.com/atimevil/Ithmb-Converter)                                                      | atimevil           | Korean Python converter with AI upscaling (Real-ESRGAN). Profiles 1015/1019/1024/1036. MIT.                                                                                                                              |
| [**yosoyemi/ithmb-converter-a-jpg**](https://github.com/yosoyemi/ithmb-converter-a-jpg)                                          | yosoyemi           | Simple Python extractor, F1019 UYVY big-endian decode with BT.601-like coefficients. MIT.                                                                                                                                |
| [**Bionded/pygpod**](https://github.com/Bionded/pygpod)                                                                          | Bionded            | Pure-Python libgpod port. Complete iPod database library with ithmb encode/decode.                                                                                                                                       |

### Color conversion references

- YCbCr → RGB uses **ITU-R BT.601** matrix (JPEG full-range variant), per [Recommendation ITU-R BT.601-7](https://www.itu.int/rec/R-REC-BT.601).
- RGB565 → RGB888 uses **MSB replication** (standard in ffmpeg, libpng, Skia).

---

## License

MIT --- see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316), closed without merging) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code.
