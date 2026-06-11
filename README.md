# ITHMB Codec for ImageGlass v10

A Native AOT C# codec plugin for [ImageGlass v10](https://imageglass.org) that opens Apple `.ithmb` thumbnail-cache files. Primarily works by locating embedded JPEG payloads inside `.ithmb` files and decoding them via StbImageSharp. Also includes SIMD-accelerated decoders (SSE2/SSSE3/Vector128) for 28 profiles (20 photo + 6 cover art) covering iPod Photo through iPhone 2G.

Tested with **956 T####.ithmb files** from an iPhone 5 (iOS 7) iPod Photo Cache --- **100% extraction rate**. Additionally validated against **227 publicly available T-prefix files** from an iPod Photo Cache (100% JPEG detection rate).

---

## Table of Contents

- [How it works](#how-it-works)
- [Install](#install)
- [Build from source](#build-from-source)
- [Architecture](#architecture)
- [Verified devices and formats](#verified-devices-and-formats)
- [Limitations](#limitations)
- [Troubleshooting](#troubleshooting)
- [SDK sample PR](#sdk-sample-pr)
- [License](#license)

---

## How it works

`.ithmb` files (iThumbnail cache) are a proprietary format used by Apple iOS devices to store photo thumbnails. Two broad categories exist:

| Type                                | Description                                                                                                                                                     | Our support                                                                                                                                                            |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **T-prefix** (e.g. `T####.ithmb`)   | Contains a single full-resolution photo as an embedded JPEG (JFIF or Exif). These are found in newer iOS device caches (iPhone 5 and later).                    | ✅ **Fully supported** --- the primary path. 956/956 verified.                                                                                                         |
| **F-prefix** (e.g. `F1019_1.ithmb`) | Older format used by iPods and early iPhones. Contains multiple raw-format thumbnails concatenated together (RGB565, YUV422, YCbCr420). These are uncompressed. | ⚠️ Best-effort decoders exist for 28 known profiles (20 photo + 6 cover art). Untested due to lack of sample files. See [raw profile table](#raw-profile-definitions). |

### Decode pipeline

1. **Read the file** --- a 4 MB header is read for JPEG scan, then the exact JPEG slice is seeked and read from the FileStream. Peak memory: ~5 MB for typical files.
2. **JPEG scan** --- the file is scanned (SIMD-accelerated via `Span.IndexOf`) for a JPEG SOI marker (`FF D8`) followed within 128 bytes by either a JFIF or Exif header. If found, the JPEG payload is extracted (SOI to EOI) and decoded via StbImageSharp (MIT, ~200 KB).
3. **Raw fallback** --- if no embedded JPEG is found, the first 4 bytes are read as a big-endian integer prefix and checked against known profiles. On match, the appropriate raw decoder (RGB565, YUV422, or YCbCr420) is used. The YUV422 decoder handles both linear (UYVY) and interlaced (F1019: even/odd rows in separate fields) layouts. A **speculative CLCL nibble-chroma** decoder is also included for iPod 4G/5G files (chroma packed as 4-bit values, no test files available).
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

Tests cover: RGB565 + RGB555 decode (both 65,536 exhaustive + SIMD-vs-scalar), 250 fuzz tests across 5 decoders, YUV422/Ycbcr420 cross-reference and roundtrip, JPEG slice detection, EXIF orientation parsing, SIMD correctness, memory safety, property invariants, JSON parser tests, real-file validation (**307 tests total**).

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

The plugin was developed through an iterative research-and-review pipeline:

1. **Format survey** — 17 open-source .ithmb implementations found across GitHub, GitLab, Codeberg, and SourceForge. Complete source analysis of each.
2. **Format table extraction** — iOpenPod, libgpod, iLounge threads, and Keith's iPod Photo Reader provided dimension/encoding tables for 28 profiles.
3. **Implementation** — C# Native AOT plugin with 4 decoders + 3 SIMD engines (SSE2, SSSE3, Vector128). ~877 lines ABI + ~554 lines decoding.
4. **Testing** — 307 unit tests (exhaustive 65K-pattern, 250 fuzz, SIMD-vs-scalar, roundtrip, property invariants, real-file validation with 227 public samples).
5. **Review cycles** — 3 rounds of 5-agent adversarial review. ~30 findings fixed covering memory safety, threading, ABI compatibility, and SIMD correctness.
6. **Release** — Windows binary (1.4 MB native AOT), published to GitHub Releases. SDK sample PR submitted to ImageGlass/SDK.
7. **Documentation** — All 17 references credited in README and RESEARCH.md. Research methodology preserved in repo.

- **Single entry point** (`ig_plugin_get_api`) --- the only C export.
- **Double-checked locking with `volatile`** in `GetApi` for thread-safe initialization (ARM64-safe).
- **Single codec**, single-frame static raster decoder.
- **Memory ownership**: the plugin allocates pixel buffers; the host calls back into `FreePixelBuffer` to release them (thread-safe).
- **SIMD acceleration**: RGB565 uses SSE2 (4-6× gain), YCbCr420 uses cross-platform Vector128 (3-5× gain, x64 + ARM64 NEON), UYVY uses SSSE3+SSE2 (2-3× gain). UYVY interlaced and non-interlaced share a common SIMD inner loop via `ProcessUyvyRow`. The speculative **CLCL nibble-chroma** decoder is scalar-only (no SIMD, no test files).

### Key source files

| File                                          | Description                                                                                                                                         |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/IthmbCodec/IthmbCodecPlugin.cs`          | Plugin ABI, init, JPEG pipeline, EXIF parsing, JSON profile loader (~877 lines)                                                                     |
| `src/IthmbCodec/IthmbCodecPlugin.Decoding.cs` | Decode algorithms + SIMD (SSE2/SSSE3/Vector128) for RGB565, YUV422, YCbCr420 (~554 lines)                                                           |
| `src/IthmbCodec/IthmbCodec.csproj`            | .NET 10 Native AOT project targeting `win-x64`, `win-arm64`, `linux-x64`, `osx-arm64`                                                               |
| `src/IthmbCodec/igplugin.json`                | Plugin manifest consumed by ImageGlass on startup                                                                                                   |
| `src/IthmbCodec/profiles.json`                | External profile definitions (sidecar, merged on first decode, overridable without recompile)                                                       |
| `src/IthmbCodec/test/`                        | xUnit test project (307 tests) --- exhaustive RGB565+RGB555, SIMD-vs-scalar, fuzz, roundtrip, EXIF, property invariants, cross-reference validation |

### Raw profile definitions

20 profiles are defined based on known iPod/iPhone thumbnail formats, aggregated from iOpenPod, Keith's iPod Photo Reader, and the original iLounge format specification thread. Additional profiles can be added at runtime via an external `profiles.json` sidecar file (shipped with the plugin, no recompile needed).

> **Note:** iOS 1.x firmwares used slightly different dimensions for some iPhone format IDs (e.g., 3004=55×55, 3009=120×160, 3011=75×75 per [Steee29/ithmb_converter](https://github.com/Steee29/ithmb_converter)). Our dimensions target iPhone 2G+ (per libgpod). If your iOS 1.x files fail to decode, try adjusting the dimensions via `profiles.json`.

| Profile | Resolution | Encoding    | Device(s)                               |
| ------- | ---------- | ----------- | --------------------------------------- |
| 1007    | 480×864    | RGB565      | iPod nano 7G (swapped dimensions)       |
| 1005    | 80×80      | RGB565      | iPod Nano 7G (photo thumbnail)          |
| 1009    | 42×30      | RGB565      | iPod Photo 4G (smallest thumbnail)      |
| 1013    | 220×176    | RGB565 BE   | iPod Photo 4G (full-screen, big-endian) |
| 1015    | 130×88     | RGB565      | iPod Photo 4G (slideshow browser)       |
| 1019    | 720×480    | YUV422      | iPod Photo/Video (TV-out, interlaced)   |
| 1020    | 176×220    | RGB565 BE   | iPod (portrait thumb, BE, rotated)      |
| 1023    | 176×132    | RGB565 BE   | iPod Nano 1G/2G (landscape, big-endian) |
| 1024    | 320×240    | RGB565      | iPod Classic 5G/6G (full-screen)        |
| 1031    | 42×42      | RGB565      | iPod Nano (album art small)             |
| 1032    | 42×37      | RGB565      | iPod Nano 1G/2G (photo list thumb)      |
| 1036    | 50×41      | RGB565      | iPod Classic (smallest thumbnail)       |
| 1066    | 64×64      | RGB565      | iPod Classic 6G (square photo)          |
| 1067    | 720×480    | YCbCr 4:2:0 | iPod Classic 6G / Nano 3G (padded)      |
| 1079    | 80×80      | RGB565      | iPod Nano 4G (photo)                    |
| 1083    | 240×320    | RGB565      | iPod Nano 4G (photo)                    |
| 1087    | 384×384    | RGB565      | iPod Nano 5G (photo)                    |
| 1092    | 80×80      | RGB565      | iPod Nano 6G (photo thumbnail)          |
| 1093    | 512×512    | RGB565      | iPod Nano 6G (full-screen photo)        |
| 1016    | 140×140    | RGB565      | iPod Photo 4G (cover art)               |
| 1017    | 56×56      | RGB565      | iPod Photo 4G (cover art)               |
| 1028    | 100×100    | RGB565      | iPod Video 5G (cover art)               |
| 1029    | 200×200    | RGB565      | iPod Video 5G (cover art)               |
| 1055    | 128×128    | RGB565      | Classic/Nano3G/Nano4G (cover art)       |
| 1060    | 320×320    | RGB565      | Classic/Nano3G (cover art)              |
| 3004    | 56×55      | RGB555      | iPhone 1G/2G, iPod Touch (photo thumb)  |
| 3008    | 640×480    | RGB555      | iPhone 1G/2G, iPod Touch (full-screen)  |
| 3009    | 160×120    | RGB555      | iPhone 1G/2G, iPod Touch (photo prev)   |
| 3011    | 80×79      | RGB555      | iPhone 1G/2G, iPod Touch (photo thumb)  |

### EXIF orientation parsing

The codec parses TIFF IFD0 tag 0x0112 from the JPEG APP1 segment and sets `outInfo->Orientation` (1-8). ImageGlass uses this to auto-rotate the display. Additional EXIF metadata (camera model, GPS, etc.) is preserved in the JPEG bytes and may be extracted independently by the host.

---

## Verified devices and formats

| Device / Source      | Files tested        | Result                                    |
| -------------------- | ------------------- | ----------------------------------------- |
| iPhone 5 (iOS 7)     | 956 T####.ithmb     | 100% --- all yield valid JPEG             |
| Jakarade.com F00-F08 | 227 T-prefix .ithmb | 100% --- all contain embedded JPEG + EXIF |

If you test this plugin with a different device or iOS version, please open an issue with sample files (or a link to them).

---

## Limitations

1. **Only T-prefix `.ithmb` files with embedded JPEG** --- this is the primary tested path. Other `.ithmb` variants may not work.
2. **Legacy raw profiles are untested** --- the decoders exist (RGB565, YUV422, YCbCr420, CLCL nibble-chroma) but no sample files were available for verification. Community contributions of unknown profiles can be added via `profiles.json` without recompiling.
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

## SDK sample PR

This plugin has been submitted as a sample to the [ImageGlass SDK](https://github.com/ImageGlass/SDK) repository:

**[PR #2 --- samples: add IthmbCodec plugin](https://github.com/ImageGlass/SDK/pull/2)**

The standalone repo (`B67687/ithmb-codec`) is the primary development home. The SDK PR is a mirror for reference.

---

## References and Acknowledgments

Every known open-source `.ithmb` implementation (17 total) was surveyed across GitHub, Codeberg, GitLab, SourceHut, Bitbucket, Gitee, and SourceForge.

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

### Color conversion references

- YCbCr → RGB uses **ITU-R BT.601** matrix (JPEG full-range variant), per [Recommendation ITU-R BT.601-7](https://www.itu.int/rec/R-REC-BT.601).
- RGB565 → RGB888 uses **MSB replication** (standard in ffmpeg, libpng, Skia).

---

## License

MIT --- see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code.
