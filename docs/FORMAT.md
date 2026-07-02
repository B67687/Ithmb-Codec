# iPod Photo Cache (.ithmb) Format Specification

## Overview

The `.ithmb` file format is Apple's proprietary thumbnail cache for iPod devices. It stores
downscaled copies of photos synchronised to the device, enabling the iPod's UI to display
photo thumbnails and full-screen previews without decoding the original JPEG each time.

Files have no global header or magic number. The first 4 bytes are a **big-endian format ID**
that identifies the encoding, dimensions, and byte layout of the raw pixel data that follows.

### Two container types

| Type | Detection | Data layout |
|------|-----------|-------------|
| **JPEG-embedded** | First bytes contain JPEG SOI marker (`FF D8 FF`) — format ID is a **small integer** (0–65535) | JPEG data starts at some offset within the file, not necessarily at byte 0 |
| **Raw profile** | First 4 bytes = big-endian format ID; bytes 4+ are raw pixel frames | `[4-byte prefix] [frame_0] [frame_1] ...` |

A file is first probed for embedded JPEG (via SOI marker search in the first 512 KB).
If no JPEG is found, it falls back to raw profile decoding using the 4-byte prefix.

---

## 4-Byte Prefix (Format ID)

The first 4 bytes are a **big-endian signed 32-bit integer** (`Int32BigEndian`).

- Values `0–65535` → **raw profile** — looked up in the device profile database
- Values `65536+` → no profile known; file is treated as JPEG-carve candidate
- The byte `0x00` in position 0 is valid (format IDs < 65536 have first byte `0x00`)

### Notable format ID ranges

| Range | Device generation | Typical encoding |
|-------|-------------------|-----------------|
| 1005–1093 | iPod Classic 5G/6G/7G, iPod Nano 1G–7G | RGB565, YCbCr420 |
| 2002–2003 | Motorola ROKR/SLVR/RAZR (iTunes Phone) | RGB565 big-endian |
| 3001–3011 | iPod Touch 1G/2G, iPhone 1G/2G | RGB555, reordered RGB555 |

Format IDs encode **both dimensions and encoding** — they are looked up in a static profile
dictionary, not computed from the data. See [Profiles](#profiles) below.

---

## Encoding Formats

### 1. RGB565 (2 bytes per pixel)

The most common encoding. Each pixel is 2 bytes in **little-endian** by default. The pixel
packing is the standard 16-bit RGB565:

```
Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
       R4 R3 R2 R1 R0 G5 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
```

- Some profiles (e.g. iPod Photo 4G, format ID 1013) use **big-endian** byte order
  (`littleEndian: false` in the profile)
- 2 bytes × width × height = total frame bytes (no padding, unless `isPadded` is set)

### 2. RGB555 (2 bytes per pixel)

Used by iPod Touch and iPhone 1G/2G. Each pixel is 16 bits with 1 unused high bit:

```
Bits   15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
       X  R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
```

### 3. Reordered RGB555 (2 bytes per pixel)

A variant of RGB555 used by iPod Touch cover art (format IDs 3001–3003). The byte
pair is byte-swapped relative to standard RGB555. Each 2-byte pixel word is stored
**big-endian** before the byte order is applied.

### 4. YUV 4:2:2 (2 bytes per pixel, 2 sub-variants)

Standard YCbCr 4:2:2 with 8-bit luma per pixel and shared chroma per horizontal pair.

**Byte layout per 2-pixel macropixel (UYVY order):**

```
[Cb] [Y0] [Cr] [Y1]   — 4 bytes, 2 pixels
```

Cb and Cr are shared between the two pixels. Conversion uses BT.601 coefficients.

#### 4a. CL — Per-pixel nibble chroma (Keith's "CL" format)

Used in later iPod generations. Each pixel has independent chroma compressed to
nibbles (4-bit).

**Byte layout per pixel:**

```
[Cb:Cr_nibble] [Y]   — 2 bytes, 1 pixel
```

- High nibble = Cb (4-bit, range 0–15, scaled to 0–240 by ×16)
- Low nibble = Cr (4-bit, range 0–15, scaled to 0–240 by ×16)

SIMD accelerated: SSSE3 (x86) and NEON (ARM64) implementations, each decoding
8 pixels per iteration. See `DecodeFormatCl.cs`.

#### 4b. CLCL — Shared macropixel nibble chroma

Two pixels share one nibble-pair, housed in a 4-byte macropixel:

```
[CbCr_nibbles] [Y0] [CbCr_nibbles] [Y1]   — 4 bytes, 2 pixels
```

Same nibble-to-byte scaling as CL. The Cb/Cr values are the same for both pixels
in the macropixel (true 4:2:2). See `DecodeFormatClcl.cs`.

#### 4c. Interlaced YUV422

Used by format ID 1019 (iPod Classic 5G/6G full-screen). The two fields (even/odd
scanlines) are interleaved. Decoding deinterlaces by reading even and odd rows
separately before interleaving them into the final frame.

### 5. YCbCr 4:2:0 packed (2 bytes per pixel, ~12-bit effective)

Used by iPod Classic 6G and Nano 3G (format ID 1067). A 12-bit YCbCr 4:2:0 signal
packed into 2 bytes per pixel (no wasted bits — like YUV422 but with subsampled
chroma).

```
Frame layout:
  [Y plane]         w × h bytes
  [Cb interleaved]  ((w+1)/2) × ((h+1)/2) × 2 bytes
```

The Y plane is full resolution. Chroma is stored as interleaved Cb/Cr pairs at
half resolution (4:2:0 subsampling). Conversion follows BT.601 coefficients.

This format is **always slot-padded** (`isPadded: true`). See [Padding](#padding) below.

---

## Padding & Slot System

Some profiles declare `isPadded: true` with a `slotSize` larger than `frameByteLength`.

| Profile | Frame bytes | Slot size | Padding |
|---------|-------------|-----------|---------|
| 3006 (Touch cover art 56×56 RGB555) | 6,272 | 8,192 | 1,920 bytes |
| 3007 (Touch cover art 88×88 RGB555) | 15,488 | 16,384 | 896 bytes |
| 3009 (Touch 120×160 RGB555) | 38,400 | 40,960 | 2,560 bytes |
| 3004 (iPhone thumbnail 56×55 RGB555) | 6,160 | 8,192 | 2,032 bytes |

The padding exists because the iPod's NAND flash has a minimum erase block size
(typically 4 KB or 8 KB), and the firmware allocates fixed-size slots.

**When `isPadded` is true:**
- `slotSize` determines the frame boundary for multi-frame slicing
- `frameByteLength` is the actual pixel data within the slot
- The decoder reads `frameByteLength` bytes, discarding padding
- A `TrailingPaddingTolerance` of 16 bytes is applied for device alignment quirks

---

## Multi-Frame Concatenation

A single `.ithmb` file can contain multiple frames concatenated end-to-end:

```
[4-byte prefix] [frame_0] [frame_1] ... [frame_N]
```

Each frame is exactly `frameSize` bytes (either `slotSize` for padded profiles or
`frameByteLength` for non-padded). The frame count is `(fileSize - 4) / frameSize`.

This is used by the iPod to store multiple thumbnail resolutions in one file.

---

## Rotation

Some profiles declare a `rotation` field (values: 0, 90, 180, 270). Rotation is
applied **after decoding** and **before cropping**. The BGRA pixel buffer is rotated
in-place by the specified degrees.

Profiles with rotation:
| Format ID | Rotation | Device |
|-----------|----------|--------|
| 1013 | 90° | iPod Photo 4G (portrait) |
| 1020 | swapped dimensions | iPod Classic 3G/4G (portrait) |

---

## Crop

Some photo formats use centered padding — the visible image is smaller than the
decoded frame, with padding borders around it. The `cropX`, `cropY`, `cropWidth`,
`cropHeight` fields in the profile define the visible region.

Crop is applied **after rotation** so the crop coordinates reference the final
orientation. Based on iOpenPod's `_crop_visible_region` approach.

---

## Profiles

The format ID is resolved through a multi-layer profile system:

1. **KnownProfiles** (53 built-in profiles) — embedded as JSON in `ProfilesJson.cs`.
   Keyed by format ID. Each entry specifies: width, height, encoding, frameBytes,
   and optional fields (littleEndian, rotation, isPadded, slotSize, crop fields,
   isInterlaced, swapsDimensions, useMhniDimensions, swapChromaPlanes, etc.)

2. **DeviceProfiles** — maps device names (e.g. "iPod Classic 6G (Thin)") to a list
   of format IDs they use. A device may override the global profile dimensions
   (e.g. Nano 7G overrides format 1013 from 220×176 to 50×50).

3. **External profiles.json** — placed next to the plugin DLL, overrides the built-in
   profiles at runtime without recompilation.

4. **Profile resolution by data size**: When a device profile declares alternates for
   a format ID, the decoder picks the variant whose `frameByteLength` matches the
   actual data size.

---

## JPEG-Embedded .ithmb

Many `.ithmb` files (especially from newer devices and the iPod Touch) simply embed
a JPEG image with a thin wrapper. Detection:

1. Scan the first 512 KB for the JPEG SOI marker `FF D8 FF`
2. If found, extract the JPEG slice from SOI to EOI (`FF D9`)
3. Decode via standard JPEG decoder (stb_image in this implementation)
4. If no SOI found in 512 KB and file is larger, extend scan to 4 MB

JPEG-embedded files usually have small format IDs (0–65535) but the JPEG data may
start at any offset — the format ID prefix may or may not precede the JPEG data.

**JPEG carving fallback**: If the format ID is unknown and no JPEG SOI is found in
the initial scan, the decoder performs a full-file byte-level JPEG carving scan
(similar to File Juicer's approach). This handles .ithmb variants from newer or
unreleased devices.

---

## Photo Database Container

iPod Touch and iPhone devices store thumbnails in a **PhotoDB** or **ArtworkDB**
container (SQLite or custom binary format) that wraps individual raw `.ithmb` blobs.
The decoder detects these containers by their first 4 bytes and parses them to
extract individual frame entries.

Each entry in the database contains:
- Format ID (same profile system as standalone .ithmb)
- Raw pixel data blob
- Dimensions (width, height)

The decoder iterates through entries and decodes each as if it were a standalone
.ithmb raw profile, with fallback to JPEG decoding for entries starting with
`FF D8`.

---

## Byte Order Summary

| Encoding | Default byte order | Notable exceptions |
|----------|-------------------|-------------------|
| RGB565 | Little-endian | 1013 (big-endian), 1020 (big-endian), 1023 (big-endian), 2002/2003 (big-endian, Motorola) |
| RGB555 | Little-endian | — |
| Reordered RGB555 | Byte-swapped big-endian | — |
| YUV422 (all variants) | Little-endian | — |
| YCbCr420 | Little-endian | — |
| Format ID prefix | Big-endian | Always |

---

## References

The format knowledge in this document was synthesized from:

- **IthmbCodec** — this implementation: clean-room analysis of `.ithmb` files against
  Apple's iPod firmware output (ImageGlass PR #2316)
- **libgpod** — `itdb_device.c` profile definitions and padding fields
- **iOpenPod** — `_crop_visible_region`, device format discovery
- **Keith's iPod Photo Reader** — Methods 3 and 4 (CL chroma decoding)
- **ithmbrdr** — Reuhno's reader: slot padding calculation
- **iPodLinux** — kernel-level .ithmb file handling
- **Steee29** — iPhone 2G iOS 1.1.4 photo database structure (mhXX format)
- **dstaley** — Nano 5G SysInfoExtended format identification (1062)
- **File Juicer** — JPEG carving approach for unknown .ithmb variants
- **cyianor/ithmbrdr** — iPod Nano 3G F1067 multi-frame format
- **andrewmalta/ithmb** — CLCL format reference
- **wrinklykong/pyithmb** — Python decoder reference

| Source | URL |
|--------|-----|
| IthmbCodec | https://github.com/B67687/ithmb-codec |
| iOpenPod | https://github.com/TheRealSavi/iOpenPod |
| libgpod | https://github.com/libgpod/libgpod |
| Keith's iPod Photo Reader | https://code.google.com/archive/p/ipod-photo-reader/ |
| ithmbrdr (cyianor) | https://github.com/cyianor/ithmbrdr |
| ithmb_converter (Steee29) | https://github.com/Steee29/ithmb_converter |
| ithmbrdr (Reuhno) | https://github.com/reuhno/ithmbrdr |
| wrinklykong/pyithmb | https://github.com/wrinklykong/pyithmb |
| mgminformatique/ipod-photo-recovery | https://github.com/mgminformatique/ipod-photo-recovery |
| iPodLinux | https://ipodlinux.org/ |
