# ITHMB Codec for ImageGlass v10

A Native AOT C# codec plugin for [ImageGlass v10](https://imageglass.org) that opens Apple `.ithmb` thumbnail-cache files. Primarily works by locating embedded JPEG payloads inside `.ithmb` files and decoding them via SkiaSharp. Also includes best-effort decoders for legacy raw thumbnail profiles (untested).

Tested with **956 T####.ithmb files** from an iPhone 5 (iOS 7) iPod Photo Cache --- **100% extraction rate**.

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

| Type                                | Description                                                                                                                                                     | Our support                                                                                                                    |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **T-prefix** (e.g. `T####.ithmb`)   | Contains a single full-resolution photo as an embedded JPEG (JFIF or Exif). These are found in newer iOS device caches (iPhone 5 and later).                    | ✅ **Fully supported** --- the primary path. 956/956 verified.                                                                   |
| **F-prefix** (e.g. `F1019_1.ithmb`) | Older format used by iPods and early iPhones. Contains multiple raw-format thumbnails concatenated together (RGB565, YUV422, YCbCr420). These are uncompressed. | ⚠️ Best-effort decoders exist for 6 known profiles (1007, 1009, 1015, 1019, 1020, 1023). Untested due to lack of sample files. |

### Decode pipeline

1. **Read the file** --- the entire `.ithmb` file is read into memory (typical size: 1-2 MB).
2. **JPEG scan** --- the file is scanned for a JPEG SOI marker (`FF D8`) followed within 128 bytes by either a JFIF or Exif header. If found, the JPEG payload is extracted (SOI to EOI) and decoded via SkiaSharp.
3. **Raw fallback** --- if no embedded JPEG is found, the first 4 bytes are read as a big-endian integer prefix and checked against known profiles. On match, the appropriate raw decoder (RGB565, YUV422, or YCbCr420) is used.
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
       IthmbCodec.dll        (1.6 MB --- the native plugin)
       libSkiaSharp.dll      (11 MB --- SkiaSharp dependency)
       igplugin.json         (plugin manifest)
   ```

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
dotnet publish src/IthmbCodec/IthmbCodec.csproj -c Release -r win-x64
```

Output lands in `src/IthmbCodec/bin/Release/net10.0/win-x64/native/`. To package for distribution:

```powershell
cd src/IthmbCodec
Copy-Item igplugin.json bin/Release/net10.0/win-x64/native/
Compress-Archive -Path bin/Release/net10.0/win-x64/native/* -DestinationPath IthmbCodec_win-x64.zip
```

### Linux (verification only)

Native AOT cross-compilation for Windows is not supported from Linux. However, you can verify the compilation by publishing for `linux-x64`:

```bash
git clone https://github.com/ImageGlass/SDK.git imageglass-sdk --depth 1
dotnet publish src/IthmbCodec/IthmbCodec.csproj -c Release -r linux-x64
```

This produces `IthmbCodec.so` instead of `IthmbCodec.dll`. It confirms the code compiles correctly through the Native AOT pipeline.

---

## Architecture

### Plugin ABI

The plugin follows the ImageGlass v10 native codec plugin ABI (v1.0.0.0):

```
ig_plugin_get_api() -> IGPluginApi -> GetCodec() -> IGCodecApi
```

- **Single entry point** (`ig_plugin_get_api`) --- the only C export.
- **Double-checked locking** in `GetApi` for thread-safe initialization.
- **Single codec**, single-frame static raster decoder.
- **Memory ownership**: the plugin allocates pixel buffers; the host calls back into `FreePixelBuffer` to release them (thread-safe).

### Key source files

| File                                 | Description                                                                                                                   |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| `src/IthmbCodec/IthmbCodecPlugin.cs` | Main plugin implementation (612 lines) --- entry point, codec API, JPEG extraction, raw profile decoders, EXIF parsing, helpers |
| `src/IthmbCodec/IthmbCodec.csproj`   | .NET 10 Native AOT project targeting `win-x64` and `ARM64`                                                                    |
| `src/IthmbCodec/igplugin.json`       | Plugin manifest consumed by ImageGlass on startup                                                                             |

### Raw profile definitions

Six legacy profiles are defined based on known iPod/iPhone thumbnail formats:

| Profile | Resolution | Encoding | Notes               |
| ------- | ---------- | -------- | ------------------- |
| 1007    | 480x864    | RGB565   | Swapped dimensions  |
| 1009    | 42x30      | RGB565   | Smallest thumbnail  |
| 1015    | 130x88     | RGB565   | Slideshow browser   |
| 1019    | 720x480    | YUV422   | TV-out resolution   |
| 1020    | 176x220    | RGB565   | Portrait thumbnail  |
| 1023    | 176x132    | RGB565   | Landscape thumbnail |

### EXIF orientation parsing

The codec parses TIFF IFD0 tag 0x0112 from the JPEG APP1 segment and sets `outInfo->Orientation` (1-8). ImageGlass uses this to auto-rotate the display. Additional EXIF metadata (camera model, GPS, etc.) is preserved in the JPEG bytes and may be extracted independently by the host.

---

## Verified devices and formats

| Device   | iOS version | Files tested    | Result                            |
| -------- | ----------- | --------------- | --------------------------------- |
| iPhone 5 | iOS 7       | 956 T####.ithmb | 100% --- all files yield valid JPEG |

If you test this plugin with a different device or iOS version, please open an issue with sample files (or a link to them).

---

## Limitations

1. **Only T-prefix `.ithmb` files with embedded JPEG** --- this is the primary tested path. Other `.ithmb` variants may not work.
2. **Legacy raw profiles are untested** --- the decoders exist (RGB565, YUV422, YCbCr420) but no sample files were available for verification.
3. **No open-file dialog** --- ImageGlass v10 Beta 2 doesn't register `.ithmb` for the file-open dialog. Use drag-and-drop.
4. **No folder browsing** --- third-party extensions can't be registered for folder navigation in Beta 2.
5. **Windows only** --- Native AOT is platform-specific. Only `win-x64` is published.
6. **Single-frame only** --- `.ithmb` files contain a single image per file. No animation/multi-frame support.

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

## License

MIT --- see [LICENSE](LICENSE).

The original IthmbDecoder reference implementation (PR [#2316](https://github.com/d2phap/ImageGlass/pull/2316)) was GPL-3.0. This plugin is a clean-room implementation for the v10 SDK ABI, informed by format behavior described in that PR but using no GPL code.
