# Apple .ithmb Thumbnail Cache Format Specification

**Document version:** 1.0  
**Format domain:** iPod Classic / Nano / Photo / Video, iPhone 2G, iPod Touch  
**Container names:** `.ithmb` (bare frame), `PhotoDB` / `ArtworkDB` (chunk-container database)

---

## 1. File Format Overview

The `.ithmb` extension covers two structurally distinct container formats:

1. **Bare frame files** — a 4-byte big-endian format prefix followed by a single (or multiple concatenated) raw pixel-data frames. These are the `F`-prefixed files (e.g. `F1019_1.ithmb`) found on iPod storage.
2. **T-prefix files** — begin with a JPEG SOI marker (`0xFF 0xD8`) and contain an embedded JPEG stream (with optional JFIF or EXIF header), decoded via a standard JPEG decoder.
3. **PhotoDB / ArtworkDB containers** — begin with the ASCII magic `mhfd` (or its big-endian equivalent `dfhm`) and organise thumbnails in a hierarchical chunk tree. These are the Photo Database and Artwork Database files stored in the iPod's `Photos/` directory.

### 1.1 Endianness

All multi-byte integer fields in the bare-frame prefix and in PhotoDB chunk headers are read according to the file's declared endianness. PhotoDB endianness is detected from the first four bytes: raw bytes `mhfd` (0x6d 0x68 0x66 0x64) indicate little-endian; raw bytes `dfhm` (0x64 0x66 0x68 0x6d) indicate big-endian. Within individual pixel-encoding formats, the per-pixel byte order is controlled by the profile's `little_endian` flag.

### 1.2 Magic Constants

| Constant | ASCII | LE u32 | BE u32 | Purpose |
|---|---|---|---|---|
| MHFD | `mhfd` | 0x6466686d | 0x4d484644 | PhotoDB File Descriptor header |
| MHSD | `mhsd` | 0x6473686d | 0x4d485344 | PhotoDB Section Descriptor |
| MHL  | `mhli` | 0x696c686d | 0x4d484c49 | PhotoDB List (four-char magic uses `i` padding) |
| MHII | `mhii` | 0x6969686d | 0x4d484949 | Photo Image Item |
| MHNI | `mhni` | 0x696e686d | 0x4d484e49 | Thumbnail Info entry |
| MHBA | `mhba` | 0x6162686d | 0x4d484241 | Album container |
| MHIA | `mhia` | 0x6169686d | 0x4d484941 | Album Item container |
| MHIF | `mhif` | 0x6669686d | 0x4d484946 | File Info record |
| MHOD | `mhod` | 0x646f686d | 0x4d484f44 | Other Data record |

---

## 2. Bare Frame Layout

Every bare `.ithmb` frame begins with a 4-byte big-endian format prefix. This prefix is treated as a signed 32-bit integer (e.g. `1007`, `1019`, `3004`) and serves as the key into the built-in profile database. The layout is:

```
Offset  Size  Field
────────────────────────────────────────
0       4     Format prefix (big-endian i32)
4       N     Pixel data (encoding-specific, see §5)
```

**Multi-frame concatenation:** Some `F`-prefixed files contain multiple independent frames appended consecutively. Each frame carries its own 4-byte prefix and pixel data. The total frame count is derived from the file size divided by each frame's `frame_byte_length` (plus 4 for the prefix). Each frame can be decoded independently by index.

**T-prefix (JPEG-embedded) files** do not have a 4-byte prefix; the file begins directly with the JPEG SOI marker `0xFF 0xD8`. The decoder detects JPEG by checking the first two bytes. When no matching profile is found but JPEG is detected, a fallback JPEG profile is used.

---

## 3. PhotoDB / ArtworkDB Container

When a file starts with the 4-byte `mhfd` magic (or its big-endian equivalent), it is a chunk-based database container called PhotoDB (photo thumbnails) or ArtworkDB (album art thumbnails). The file is structured as a tree of typed chunks, each beginning with a 4-byte magic and a 4-byte `header_size` field.

### 3.1 Chunk Tree Structure

The root chunk is always **MHFD**. Its children are **MHSD** section descriptors, which in turn contain **MHL** photo lists. Each **MHL** contains **MHII** photo item containers, and the leaf nodes are **MHNI** thumbnail info entries that reference the actual pixel data. **MHBA** (album) and **MHIA** (album item) chunks form a parallel branch for album-art thumbnails. **MHIF** and **MHOD** carry metadata. The tree-walk algorithm uses recursion with a maximum depth of 64.

Parent-child relationship (indentation shows nesting):

```
MHFD (root, 12 bytes)
  └─ MHSD (section, 16 bytes)
       └─ MHL (photo list, 12 bytes)
            └─ MHII (photo item, 12+ bytes)
                 └─ MHNI (thumbnail info, 36 or 76 bytes)
  └─ MHBA (album container, 12 bytes)
       └─ MHIA (album item, 12 bytes)
            └─ MHNI (thumbnail info, 36 or 76 bytes)
```

### 3.2 Chunk Types

#### 3.2.1 MHFD — File Descriptor

The root chunk. Always 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHFD` |
| 4 | 4 | header_size | Always 12 (size of this header) |
| 8 | 4 | entry_count | Number of top-level MHSD sections |

#### 3.2.2 MHSD — Section Descriptor

Describes a section of the database. 16 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHSD` |
| 4 | 4 | header_size | Total section size including all child entries |
| 8 | 2 | index | Section index within parent |
| 10 | 2 | record_type | Type of records: 1 = Photos, 4 = Thumbnails |
| 12 | 4 | entry_count | Number of records in this section |

#### 3.2.3 MHL — Image List (magic "mhli")

Groups photo items. 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHL` |
| 4 | 4 | header_size | Always 12 |
| 8 | 4 | count | Number of child MHII items |

#### 3.2.4 MHII — Image Info

Identifies a single photo. 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHII` |
| 4 | 4 | header_size | Always 12 |
| 8 | 4 | photo_id | Unique photo identifier (also serves as total_len for children) |

#### 3.2.5 MHNI — Image Name (Thumbnail Info)

The critical record that maps a `format_id` to a byte range of pixel data. Two variants exist:

**Classic (iPod Classic 6G/7G) — 36 bytes:**

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHNI` |
| 4 | 4 | header_size | 36 |
| 8 | 8 | (padding) | Reserved, zero |
| 16 | 4 | format_id | Matches profile prefix (e.g. 1019) |
| 20 | 4 | ithmb_offset | Byte offset of pixel data within the .ithmb file |
| 24 | 4 | image_size | Byte count of the pixel data blob |
| 28 | 4 | (padding) | Reserved |
| 32 | 2 | height | Image height in pixels (u16 LE/BE) |
| 34 | 2 | width | Image width in pixels (u16 LE/BE) |

**Extended (Apple TV / Animal) — 76 bytes:**

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHNI` |
| 4 | 4 | header_size | 76 |
| 8 | 8 | (padding) | Reserved |
| 16 | 4 | format_id | Matches profile prefix |
| 20 | 4 | packed | Low 16 bits = width, high 16 bits = height |
| 24 | 4 | (additional fields) | External-file references |

In the extended variant, `ithmb_offset` is `-1` indicating the pixel data is stored in an external `.ithmb` file rather than inline.

#### 3.2.6 MHBA — Image BGRA Data (Album Container)

Album container. 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHBA` |
| 4 | 4 | header_size | Always 12 |
| 8 | 4 | album_id | Unique album identifier |

#### 3.2.7 MHIA — Image Attributes (Album Item Container)

Album item container. 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHIA` |
| 4 | 4 | header_size | Always 12 |
| 8 | 4 | artwork_id | Unique artwork identifier |

#### 3.2.8 MHIF — Image File Info

Metadata record. 12 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 4 | magic | `MHIF` |
| 4 | 4 | header_size | Always 12 |
| 8 | 4 | info_type | Type of file info |

#### 3.2.9 MHOD — Other Data

Variable-length data record. The header is 4 bytes.

| Offset | Size | Field | Description |
|---|---|---|---|
| 0 | 2 | tag | 1 = null-terminated string (MhodString) |
| 2 | 2 | size | Size of the data following this header |

The payload following the header is raw bytes (typically UTF-16 null-terminated string data).

### 3.3 JPEG Trimming

When an MHNI entry carries an unknown `format_id` but its data starts with the JPEG SOI marker, the parser performs backward search for the JPEG EOI marker (`0xFF 0xD9`) and trims any trailing garbage bytes. This handles PhotoDB entries where the `image_size` field includes both the JPEG stream and padding bytes.

---

## 4. Profile System

Each known bare-frame format is described by a **profile** — a struct containing all parameters needed to decode a frame. The profile database contains 54 built-in profiles derived from community reverse-engineering (iOpenPod, libgpod, clickwheel, Keith's iPod Photo Reader, and 18 other open-source implementations).

### 4.1 Profile Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `prefix` | i32 | 0 | Big-endian 4-byte format identifier |
| `width` | i32 | 0 | Frame width in pixels |
| `height` | i32 | 0 | Frame height in pixels |
| `encoding` | Encoding | Rgb565 | Pixel encoding format |
| `frame_byte_length` | i32 | 0 | Byte count of one complete frame (excluding prefix) |
| `swaps_dimensions` | bool | false | Swap width and height after decode |
| `little_endian` | bool | true | Per-pixel byte order (false = big-endian) |
| `is_padded` | bool | false | Frame occupies a fixed-size slot with zero padding |
| `is_interlaced` | bool | false | Even/odd scanlines stored as separate fields |
| `clcl_chroma` | bool | false | Chroma uses CLCL shared-nibble planar layout |
| `swap_chroma_planes` | bool | false | Swap Cb/Cr plane order in YCbCr 4:2:0 |
| `cl_chroma` | bool | false | Chroma uses CL per-pixel nibble layout |
| `swap_rgb_channels` | bool | false | Swap R and B channels (BGR15 for iPhone) |
| `rotation` | i32 | 0 | Post-decode clockwise rotation (0, 90, 180, 270) |
| `crop_x` | i32 | 0 | Visible-region X offset |
| `crop_y` | i32 | 0 | Visible-region Y offset |
| `crop_width` | i32 | 0 | Visible-region width (0 = full remaining) |
| `crop_height` | i32 | 0 | Visible-region height (0 = full remaining) |
| `slot_size` | i32 | 0 | Fixed slot byte size for padded profiles |
| `use_mhni_dimensions` | bool | false | Use MHNI chunk width/height instead of fixed values |
| `fallback_encodings` | Option\<Vec\<Encoding\>\> | None | Ordered list of fallback encodings to try |

### 4.2 Prefix-Based Lookup

During decoding the first 4 bytes are read as a big-endian `i32`. This value is looked up in the profile database. When a match is found, the associated profile provides all parameters for decoding. When no match exists but the data starts with `0xFF 0xD8` (JPEG SOI), a fallback JPEG profile is used with `use_mhni_dimensions = true` so dimensions come from the JPEG metadata.

### 4.3 Encoding Enum

```
pub enum Encoding {
    Rgb565,          // 16-bit RGB 5/6/5
    Rgb555,          // 15-bit RGB 5/5/5 (MSB unused)
    ReorderedRgb555, // 15-bit RGB with Morton Z-order interleave
    Yuv422,          // Packed UYVY 4:2:2
    Ycbcr420,        // Planar YCbCr 4:2:0
    Jpeg,            // JPEG passthrough (SOI/EOI delimited)
}
```

---

## 5. Pixel Encoding Formats

This section describes all eight pixel encoding formats supported by the decoder. Each entry provides the byte layout, bits-per-pixel, subsampling details, compression scheme, and alignment/padding rules.

### 5.1 RGB565 (16-bit, 5/6/5)

**BPP:** 2 bytes per pixel  
**Used by:** Most iPod formats (1007, 1024, 1027, 1029, 1055, 1060, 1061, etc.)

Bit layout within each 16-bit word:

```
Bit     15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
        R4 R3 R2 R1 R0 G5 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
```

- Red: 5 bits (bits 11-15) expanded to 8 via MSB replication: `(r5 << 3) | (r5 >> 2)`
- Green: 6 bits (bits 5-10) expanded to 8 via MSB replication: `(g6 << 2) | (g6 >> 4)`
- Blue: 5 bits (bits 0-4) expanded to 8 via MSB replication
- Byte order: little-endian by default (byte 0 = low byte), big-endian variant exists (e.g. iPod Photo 4G format 1013)
- `swap_rgb_channels` flag: when true, treats the bit layout as BGR15 (`xBBBBBGGGGGRRRRR`) for iPhone 2G compatibility

Output: BGRA 8-bit per channel (B at byte 0, G at byte 1, R at byte 2, A = 255 at byte 3).

**Frame byte calculation:** `width × height × 2`

### 5.2 RGB555 (15-bit, 5/5/5)

**BPP:** 2 bytes per pixel (MSB of each 16-bit word is unused)  
**Used by:** iPhone/iPod Touch formats (3004, 3005, 3006, 3007, 3008, 3009, 3011)

Default layout (`swap_rgb_channels = false`):

```
Bit     15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
        x  R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
```

BGR15 layout (`swap_rgb_channels = true`, iPhone 2G):

```
Bit     15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
        x  B4 B3 B2 B1 B0 G4 G3 G2 G1 G0 R4 R3 R2 R1 R0
```

- Each 5-bit channel expanded to 8 bits via: `(v << 3) | (v >> 2)`
- Byte order: little-endian by default; big-endian variant exists for certain profiles
- Some profiles (3004, 3006, 3007) use padded slot allocation with `is_padded = true`

**Frame byte calculation:** `width × height × 2`

### 5.3 Reordered RGB555 (Morton Z-Order Interleave)

**BPP:** 2 bytes per pixel  
**Used by:** iPod Touch formats (3001, 3002, 3003)

This variant of RGB555 stores each 16-bit pixel in big-endian byte order AND rearranges pixel positions using a Morton Z-order curve. The decoder reads pixels from Z-order positions and writes them in row-major order.

**Pixel bit layout** (identical to standard RGB555 big-endian):

```
Default:  x R4 R3 R2 R1 R0 G4 G3 G2 G1 G0 B4 B3 B2 B1 B0
BGR15:    x B4 B3 B2 B1 B0 G4 G3 G2 G1 G0 R4 R3 R2 R1 R0
```

**Morton Z-order:** The position of each pixel in the data stream is computed by interleaving the bits of its x and y coordinates:

```
z = morton_interleave(x, y, bits)
where bits = ceil(log2(max(width, height)))
```

The interleave function places y bits in even positions and x bits in odd positions:

```
for i in 0..bits:
    z |= ((x >> i) & 1) << (2*i + 1)
    z |= ((y >> i) & 1) << (2*i)
```

For non-power-of-2 dimensions, gaps in the Z-order address space are zero-filled (decoder reads 0x0000 for out-of-range positions). The encoder allocates a buffer sized to the largest Z-order index, which for square power-of-2 images equals `width × height`.

**Frame byte calculation:** `width × height × 2` (potentially larger for non-power-of-2 due to buffer padding).

### 5.4 UYVY (YUV 4:2:2, Packed)

**BPP:** 2 bytes per pixel  
**Used by:** iPod Classic 5G/5.5G (format 1019, 720x480 full-screen video)

Layout per 2-pixel pair (4 bytes):

```
Byte 0: U0 (Cb)     — shared chroma blue-difference for both pixels
Byte 1: Y0          — luma for pixel 0
Byte 2: V0 (Cr)     — shared chroma red-difference for both pixels
Byte 3: Y1          — luma for pixel 1
```

- Chroma is sampled at half the horizontal rate (4:2:2): each pair of pixels shares one Cb and one Cr value.
- Odd widths: the last pixel reads its Y and U from the trailing incomplete group and reuses V from the last complete group (or 128 if no groups exist).
- Color conversion uses BT.601 fixed-point coefficients:

```
R = clamp(Y + (Cr - 128) * 359 / 256)
G = clamp(Y - (Cb - 128) * 88 / 256 - (Cr - 128) * 183 / 256)
B = clamp(Y + (Cb - 128) * 454 / 256)
```

- Division is arithmetic right-shift (`>> 8`), matching C# semantics.

**Interlaced variant:** When `is_interlaced = true`, the pixel data is split into two fields. The first half of the data contains even rows (0, 2, 4, ...) and the second half contains odd rows (1, 3, 5, ...). Each field is decoded as progressive UYVY, then rows are weaved into the final frame. Requires even height.

**Frame byte calculation:** `width × height × 2`

### 5.5 YCbCr 4:2:0 (Planar)

**BPP:** 1.5 bytes per pixel (average)  
**Used by:** iPod Classic 6G (format 1067, 720x480 padded)

Three separate planes:

```
Plane 0: Y — full resolution (width × height bytes)
Plane 1: Cb or Cr — quarter resolution ((width/2) × (height/2) bytes)
Plane 2: Cr or Cb — quarter resolution ((width/2) × (height/2) bytes)
```

- Chroma subsampling: 4:2:0 — each Cb/Cr sample covers a 2x2 block of luma pixels.
- Chroma upsampling: nearest-neighbour (each chroma sample is applied to all 4 pixels in its 2x2 block).
- Cb and Cr plane order: default is Y, Cb, Cr. When `swap_chroma_planes = true`, the order becomes Y, Cr, Cb.
- Requires even width and height (odd dimensions cannot be subsampled).
- BT.601 colour conversion (same coefficients as UYVY).
- Some profiles (e.g. 1067) set `is_padded = true`, allocating the frame to a fixed slot.

**Frame byte calculation:** `w × h + (w/2 × h/2) + (w/2 × h/2)`

Example for 720x480: Y = 345,600 bytes, each chroma plane = 86,400 bytes, total = 518,400 bytes (but padded profile 1067 uses 691,200).

### 5.6 CL (Compressed Luma — Per-Pixel Nibble Chroma)

**BPP:** 2 bytes per pixel  
**Used by:** Certain Yuv422 profiles with `cl_chroma = true`

Planar layout:

```
[Y0, Y1, ..., Y(N-1), CbCr0, CbCr1, ..., CbCr(N-1)]
```

- **Y plane:** `width × height` bytes, each a full 8-bit luma value.
- **CbCr plane:** `width × height` bytes, each byte packs two 4-bit chroma nibbles:
  - High nibble = Cr (red-difference, 4-bit)
  - Low nibble = Cb (blue-difference, 4-bit)
- Each 4-bit chroma nibble is upscaled to 8-bit by left-shifting 4 positions (`nibble << 4`).
- Color conversion is BT.601 YUV-to-BGRA.

Example for 2 pixels:
```
Byte 0: Y0
Byte 1: Y1
Byte 2: (Cr0 << 4) | Cb0
Byte 3: (Cr1 << 4) | Cb1
```

**Frame byte calculation:** `width × height × 2`

### 5.7 CLCL (Compressed Luma+Chroma — Separate Nibble Planes)

**BPP:** 2 bytes per pixel  
**Used by:** Certain Yuv422 profiles with `clcl_chroma = true`

Planar layout with three planes:

```
[Y0, Y1, ..., Y(N-1), Cb0_Cb1, ..., Cb_{N-2}_Cb_{N-1}, Cr0_Cr1, ..., Cr_{N-2}_Cr_{N-1}]
```

- **Y plane:** `width × height` bytes (full 8-bit luma).
- **Cb plane:** `N/2` bytes, each packing two 4-bit Cb nibbles. Even pixels use the low nibble; odd pixels use the high nibble. Byte packing: `byte[k] = (Cb_{2k+1} << 4) | Cb_{2k}`.
- **Cr plane:** Same packing scheme as Cb (`N/2` bytes).
- Each 4-bit nibble upscaled to 8-bit by `<< 4`.
- Color conversion: BT.601.

Example for 4 pixels:
```
Byte 0: Y0
Byte 1: Y1
Byte 2: Y2
Byte 3: Y3
Byte 4: (Cb1 << 4) | Cb0   // Cb nibbles for pixels 0, 1
Byte 5: (Cb3 << 4) | Cb2   // Cb nibbles for pixels 2, 3
Byte 6: (Cr1 << 4) | Cr0   // Cr nibbles for pixels 0, 1
Byte 7: (Cr3 << 4) | Cr2   // Cr nibbles for pixels 2, 3
```

**Frame byte calculation:** `width × height + (width × height) / 2 + (width × height) / 2 = width × height × 2`

### 5.8 JPEG Passthrough

**BPP:** Variable (compressed)  
**Used by:** T-prefix files (e.g. iPhone 5, iPod Touch JPEG-embedded thumbnails)

The file (or inline data blob) contains a standard JPEG stream delimited by SOI (`0xFF 0xD8`) and EOI (`0xFF 0xD9`) markers. Decoding is performed by a standard JPEG decoder (the `jpeg-decoder` crate). EXIF orientation tag (0x0112) is parsed and exposed.

- T-prefix files are always single-frame.
- JPEG SOI must be within the first 4 MB of the file.
- No 4-byte format prefix precedes the JPEG stream in T-prefix files.
- In PhotoDB entries with unknown format IDs, the embedded JPEG data is trimmed at the EOI marker to remove trailing padding.

---

## 6. Built-in Profiles

The following 54 profiles are embedded at compile time in the profile database. Profiles are keyed by their big-endian 4-byte prefix (stored as a signed 32-bit integer). External profiles can be added at runtime via a `profiles.json` file.

### 6.1 Profile Table

| Prefix | Width | Height | Encoding | BPP | Flags |
|---|---|---|---|---|---|
| 1005 | 80 | 80 | RGB565 | 2 | — |
| 1007 | 480 | 864 | RGB565 | 2 | — |
| 1009 | 42 | 30 | RGB565 | 2 | — |
| 1010 | 240 | 240 | RGB565 | 2 | — |
| 1013 | 220 | 176 | RGB565 | 2 | Big-endian, rotation=90 |
| 1015 | 130 | 88 | RGB565 | 2 | — |
| 1016 | 140 | 140 | RGB565 | 2 | — |
| 1017 | 56 | 56 | RGB565 | 2 | — |
| 1019 | 720 | 480 | YUV422 (UYVY) | 2 | Interlaced |
| 1020 | 176 | 220 | RGB565 | 2 | Swaps dimensions, big-endian |
| 1023 | 176 | 132 | RGB565 | 2 | Big-endian |
| 1024 | 320 | 240 | RGB565 | 2 | — |
| 1027 | 100 | 100 | RGB565 | 2 | — |
| 1028 | 100 | 100 | RGB565 | 2 | — |
| 1029 | 200 | 200 | RGB565 | 2 | — |
| 1031 | 42 | 42 | RGB565 | 2 | — |
| 1032 | 42 | 37 | RGB565 | 2 | — |
| 1036 | 50 | 41 | RGB565 | 2 | — |
| 1042 | 320 | 240 | RGB565 | 2 | — |
| 1043 | 130 | 88 | RGB565 | 2 | — |
| 1044 | 128 | 128 | RGB565 | 2 | — |
| 1055 | 128 | 128 | RGB565 | 2 | — |
| 1056 | 128 | 128 | RGB565 | 2 | — |
| 1060 | 320 | 320 | RGB565 | 2 | — |
| 1061 | 55 | 55 | RGB565 | 2 | use_mhni_dimensions |
| 1062 | 56 | 56 | RGB565 | 2 | — |
| 1066 | 64 | 64 | RGB565 | 2 | — |
| 1067 | 720 | 480 | YCbCr420 | 1.5 | Padded |
| 1068 | 128 | 128 | RGB565 | 2 | — |
| 1071 | 240 | 240 | RGB565 | 2 | — |
| 1073 | 240 | 240 | RGB565 | 2 | — |
| 1074 | 50 | 50 | RGB565 | 2 | — |
| 1078 | 80 | 80 | RGB565 | 2 | — |
| 1079 | 80 | 80 | RGB565 | 2 | — |
| 1081 | 640 | 480 | RGB565 | 2 | fallback_encodings=[Jpeg] |
| 1083 | 240 | 320 | RGB565 | 2 | — |
| 1084 | 240 | 240 | RGB565 | 2 | — |
| 1085 | 88 | 88 | RGB565 | 2 | — |
| 1087 | 384 | 384 | RGB565 | 2 | — |
| 1089 | 58 | 58 | RGB565 | 2 | — |
| 1092 | 80 | 80 | RGB565 | 2 | — |
| 1093 | 512 | 512 | RGB565 | 2 | — |
| 2002 | 50 | 50 | RGB565 | 2 | Big-endian |
| 2003 | 150 | 150 | RGB565 | 2 | Big-endian |
| 3001 | 256 | 256 | ReorderedRGB555 | 2 | Morton Z-order |
| 3002 | 128 | 128 | ReorderedRGB555 | 2 | Morton Z-order |
| 3003 | 64 | 64 | ReorderedRGB555 | 2 | Morton Z-order |
| 3004 | 56 | 55 | RGB555 | 2 | Padded, slot_size=8192 |
| 3005 | 320 | 320 | RGB555 | 2 | — |
| 3006 | 56 | 56 | RGB555 | 2 | Padded, slot_size=8192 |
| 3007 | 88 | 88 | RGB555 | 2 | Padded, slot_size=16384 |
| 3008 | 640 | 480 | RGB555 | 2 | — |
| 3009 | 120 | 160 | RGB555 | 2 | Padded, slot_size=40960 |
| 3011 | 80 | 79 | RGB555 | 2 | — |

### 6.2 Prefix Numbering Conventions

- **1xxx:** Classic iPod formats (RGB565 or YUV). Formats 1000-1099 cover iPod Photo 4G through iPod Nano 7G.
- **2xxx:** Motorola ROKR E1 formats (big-endian RGB565). Only 2002 and 2003 are known.
- **3xxx:** iPhone / iPod Touch formats (RGB555 or ReorderedRGB555). Formats 3000-3011.

---

## 7. Device Profile Table

The following 18 iPod/iPhone generations are documented with their known format IDs. Each device generation produces specific format IDs in its thumbnail caches. This mapping enables per-device profile selection for sync tools.

| Device | Format IDs | Notes |
|---|---|---|
| iPod Classic 5G (Video) | 1019, 1024, 1027, 1028, 1029, 1031, 1032 | 720x480 interlaced UYVY |
| iPod Classic 5.5G (Enhanced) | 1019, 1024, 1027, 1028, 1029, 1031, 1032, 1055, 1056 | Adds 128x128 and 80x80 |
| iPod Classic 6G (Thin) | 1024, 1055, 1060, 1061, 1066, 1067, 1068 | 56x56 cover art, padded YCbCr |
| iPod Video 5G | 1019, 1024, 1027, 1028, 1029, 1031, 1032 | Same as Classic 5G |
| iPod Nano 1G | 1024, 1027 | Basic 320x240 + cover art |
| iPod Nano 2G | 1019, 1027, 1028, 1029, 1032 | Adds YUV422 |
| iPod Nano 3G | 1066, 1067, 1068, 1071, 1073, 1074 | 240x240 formats |
| iPod Nano 4G | 1071, 1073, 1074, 1078, 1079, 1083, 1084, 1085, 1087, 1089, 1092, 1093 | Most formats: 12 entries |
| iPod Nano 5G | 1087, 1092, 1093 | 384x384 + 512x512 |
| iPod Nano 6G | 1084, 1092, 1093 | Small subset |
| iPod Nano 7G | 1007, 1010 | 480x864 full-res + 240x240 |
| iPod Mini 1G/2G | 1024, 1027 | Same as Nano 1G |
| iPod Photo 4G | 1013, 1015, 1016, 1019 | Big-endian 220x176 |
| iPod Touch 1G/2G | 3001, 3002, 3003, 3004, 3005, 3008, 3009, 3011 | ReorderedRGB555 + padded RGB555 |
| iPod Touch 3G/4G | 3001, 3002, 3003, 3004, 3005, 3008, 3009, 3011 | Same as Touch 1G/2G |
| iPhone 1G/2G | 3001, 3002, 3003, 3004, 3005, 3008, 3009, 3011 | Same as Touch |
| iPhone 3G/3GS | 3001, 3002, 3003, 3004, 3005, 3008, 3009, 3011 | Same as Touch |
| Motorola ROKR E1 | 2002, 2003 | Big-endian 50x50 and 150x150 |

---

## 8. Post-Processing

After raw pixel data is decoded to BGRA, the decoder applies the following post-processing steps in order:

### 8.1 Dimension Swap

If `swaps_dimensions = true`, the image width and height metadata values are swapped after decoding. This is used when the stored pixel dimensions do not match the intended display orientation (e.g. profile 1020 stores 176x220 but displays as 220x176).

### 8.2 Cropping

A rectangular crop region is extracted from the decoded image. The crop rectangle is defined by:

- `crop_x`, `crop_y`: Offset from top-left corner
- `crop_width`, `crop_height`: Size of the crop region

When `crop_width` or `crop_height` is 0, the remaining span from the corresponding offset to the image edge is used. All values are clamped to image bounds. The crop is applied after any dimension swap.

### 8.3 Rotation

The image is rotated clockwise by the angle specified in `rotation`. Supported values:

| Value | Effect |
|---|---|
| 0 | No rotation (passthrough) |
| 90 | 90 degrees clockwise (width and height swap) |
| 180 | 180 degrees (dimensions unchanged) |
| 270 | 270 degrees clockwise / 90 degrees counter-clockwise (width and height swap) |

Other values are silently ignored. Rotation is applied after cropping.

### 8.4 Padding Handling

Padded profiles (`is_padded = true`) allocate frames within fixed-size byte slots. The decoder uses the profile's `slot_size` as the frame stride when reading pixel rows (instead of `frame_byte_length`). Extra bytes beyond the visible pixel data are zero-fill padding. This is common in iPhone/Touch formats where small thumbnails (e.g. 56x55) occupy fixed 8 KB slots.

### 8.5 Interlacing

When `is_interlaced = true`, the pixel data is organised as two separate fields. For 2 Bpp formats (UYVY, RGB565), the first half of the data contains all even-numbered rows (field 0) and the second half contains all odd-numbered rows (field 1). For YCbCr 4:2:0 planar, each of the three planes (Y, Cb, Cr) is interlaced separately using its own row stride. After decoding both fields, rows are weaved back into the correct order.

### 8.6 BT.601 Color Conversion

All YUV-based formats (UYVY, YCbCr 4:2:0, CL, CLCL) use BT.601-7 colour conversion with fixed-point integer arithmetic:

```
Forward (RGB to YCbCr):
  Y  = ( 77*R + 150*G +  29*B) >> 8
  Cb = ((-43*R -  85*G + 128*B) >> 8) + 128
  Cr = ((128*R - 107*G -  21*B) >> 8) + 128

Reverse (YCbCr to RGB):
  R = clamp(Y + ((Cr - 128) * 359) >> 8)
  G = clamp(Y - ((Cb - 128) *  88) >> 8 - ((Cr - 128) * 183) >> 8)
  B = clamp(Y + ((Cb - 128) * 454) >> 8)
```

Coefficients are derived from ITU-R BT.601-7:

| Coefficient | Value | Precision |
|---|---|---|
| R coef (Cr) | 1.402 × 256 = 359 | Truncated |
| G coef (Cb) | -0.344 × 256 = -88 | Truncated magnitude |
| G coef (Cr) | -0.714 × 256 = -183 | Truncated magnitude |
| B coef (Cb) | 1.772 × 256 = 454 | Truncated |

The decoder output is always BGRA 8-bit with alpha set to 255.

### 8.7 BGR15 Channel Swap

The `swap_rgb_channels` flag addresses a difference in pixel byte-ordering between iPod and iPhone generations. iPod devices store RGB555/RGB565 with the standard R-in-high-bits layout. iPhone 2G thumbnails use a BGR15 layout where B occupies the high 5 bits and R occupies the low 5 bits. The flag swaps the RGB output channels so that BGR15 data produces correct BGRA output.

---

## 9. Encoder Behavior

The encoder module (used for synthetic roundtrip testing and PhotoDB construction) mirrors each decoder exactly. The encoding pipeline follows these steps:

1. **Rotate** the source BGRA image in the reverse direction if `profile.rotation` is non-zero.
2. **Encode** pixels using the selected pixel format encoder (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr 4:2:0, CL, or CLCL).
3. **Interlace** fields if `profile.is_interlaced` is true, reordering rows into field 0 (even) + field 1 (odd).
4. **Pad** to `profile.frame_byte_length` with zeros if the encoded data is shorter.
5. **Prepend** the 4-byte big-endian prefix.

The encoder uses the same BT.601 forward-transform coefficients for YUV formats.

---

## 10. File Size Guard

A 32 MB file size limit is enforced before any decoding begins. This prevents out-of-memory conditions from pathological or corrupt input. All known real `.ithmb` files are under 1 MB (maximum observed: 852 KB). The 32 MB limit covers approximately 40 max-size raw frames (profile 1007 at 480x864 RGB565 = 829 KB per frame), providing a generous safety margin.

---

## References

The format specification is derived from the following open-source reverse-engineering projects:

- *iOpenPod* (TheRealSavi) — 50+ empirically validated profiles across multiple iPod models
- *libgpod* (community) — PhotoDB/ArtworkDB chunk parser and format ID tables
- *Keith's iPod Photo Reader* (kebwi) — Original 2005 reverse-engineering, 13 decode methods
- *clickwheel* (dstaley) — C# ArtworkDB read/write, 40+ format IDs
- *OrgZ* (FoxCouncil) — C# ArtworkDB + ithmb read/write
- *pyithmb* (wrinklykong) — Python YUV reference decoder

Color conversion coefficients follow ITU-R BT.601-7.
