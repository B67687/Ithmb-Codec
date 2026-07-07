# Glossary — .ithmb Codec Concepts Explained Simply

This file explains every technical term in the project for people who aren't codec engineers. If you see something in the README or docs that doesn't make sense, look here first.

---

## Codec

Short for **encoder/decoder**. A codec takes image data, compresses it (encodes) for storage, and decompresses it (decodes) for display. This project is a codec for Apple's `.ithmb` format used by iPods and iPhones for thumbnail caches.

**Analogy**: Think of a codec like a shipping container. You pack your items inside (encode), the container travels (storage/transfer), and you unpack at the destination (decode). The unpacked contents should match what you packed — but maybe not perfectly (see [Lossy vs Lossless](#lossy-vs-lossless)).

---

## Decoder vs Encoder

- **Decoder**: Reads a `.ithmb` file and produces image pixels (BGRA, explained below) that you can view or save as PNG.
- **Encoder**: Takes image pixels (BGRA) and writes a `.ithmb` file.

This project has **8 decoders** (one for each format the iPod uses) and **7 encoders** (can write back all formats).

**In practice**: Most people only use the decoder — they want to extract photos from an old iPod. The encoder exists for syncing artwork *to* an iPod without iTunes.

---

## Lossy vs Lossless

| Term | Meaning | Example |
|------|---------|---------|
| **Lossless** | Every bit of the original data is preserved. Decoding gives back the exact original. | ZIP files, PNG images |
| **Lossy** | Some detail is thrown away to save space. What you get back is close but not exact. | JPEG photos, MP3 audio |

**Every raw `.ithmb` format is lossy.** iPod thumbnails were stored with reduced color precision because:
- Thumbnails are tiny (usually 56×55 to 320×320 pixels)
- The human eye barely notices missing color detail at that size
- Apple wanted to fit thousands of thumbnails into limited iPod storage (30GB hard drives, not 256GB)

**This is NOT a bug in our codec** — we accurately reproduce whatever the format stores. The format itself decided to throw away color information.

---

## Roundtrip

The process: **take image → encode to .ithmb → decode back to image**. A roundtrip test checks that the decoded image matches what you'd get if you encoded and decoded with the original Apple software.

**What roundtrip DOESN'T mean**: Getting back your original image. Because the format is [lossy](#lossy-vs-lossless), encoding throws away information. The roundtrip tests check that our encoder and decoder are **consistent with each other** — not that they're lossless.

**Stable roundtrip**: If you encode a file, decode it, and re-encode the result, the second `.ithmb` file is bit-identical to the first. This proves our encoder/decoder pair is mathematically stable — they've agreed on what the data means.

**Why this matters**: If your roundtrip test passes, you know the decoder didn't introduce bugs. The only "loss" is what the format itself intended.

---

## Stable Roundtrip

A stronger form of the [roundtrip](#roundtrip) test. A format has a **stable roundtrip** if:

```
encode(decode(encode(input))) == encode(input)
```

In plain English: if you decode an existing `.ithmb` file and re-encode the result, you get the exact same bytes back. This proves:
1. The encoder and decoder agree on the format specification exactly
2. No information is lost beyond what the format inherently discards
3. The codec is self-consistent

**Our codec passes stable roundtrip for all 7 raw formats.**

---

## RGB, YUV, BGRA — Color Formats

These are different ways to represent the same colors:

### RGB (Red, Green, Blue)
The standard way computers store images. Each pixel is three numbers: how much Red, Green, and Blue. A pixel with R=255, G=0, B=0 is pure red.
- Standard RGB: 24 bits per pixel (8 bits for each of R, G, B)
- 16.7 million possible colors

### BGRA (Blue, Green, Red, Alpha)
Same as RGB but the order is shuffled: Blue byte first, then Green, then Red, then Alpha (transparency). This is the layout Windows and most image viewers expect. Our decoders output BGRA.

### YUV (Luma + Chroma)
A different way to store color that separates brightness from color:
- **Y** = brightness (luma) — the black-and-white part
- **U** and **V** = color (chroma) — the color part

The human eye is more sensitive to brightness than color. YUV exploits this by storing brightness at full resolution but color at reduced resolution. This is called [chroma subsampling](#chroma-subsampling--422-420).

**How it connects**: iPods store thumbnails in YUV format (it's smaller), but our decoder converts them to BGRA (what your screen displays). The math that converts YUV→RGB is called **BT.601** — a formula defined by an old broadcast television standard.

---

## Bit Depth — What Does 565 or 555 Mean?

These numbers describe how many bits are used for each color channel:

| Name | Red bits | Green bits | Blue bits | Total colors | Looks like |
|------|----------|------------|-----------|-------------|------------|
| **RGB565** | 5 | 6 | 5 | 65,536 | 16-bit color |
| **RGB555** | 5 | 5 | 5 | 32,768 | 15-bit color |

Your phone or monitor shows **8 bits per channel** (24-bit / 16.7 million colors). The iPod used 5 or 6 bits because:
- Thumbnails are tiny
- The iPod's screen was also low-color (early models had 65K color displays)
- Less data means more thumbnails fit in storage

**Real effect**: A gradient that looks smooth on your phone will show faint banding on an iPod thumbnail. Our decoders accurately reproduce the iPod's lower-quality output — they don't invent the missing bits.

---

## Chroma Subsampling — 4:2:2, 4:2:0

These numbers describe how much color information is kept versus thrown away:

| Notation | What it means |
|----------|---------------|
| **4:4:4** | Every pixel has full color — no compression |
| **4:2:2** | Horizontal pairs of pixels share color (color is half resolution horizontally) |
| **4:2:0** | Blocks of 4 pixels (2×2) share color (color is half resolution in both directions) |

**Analogy**: Imagine a coloring book:
- **4:4:4** = every pixel individually colored (full quality, large file)
- **4:2:2** = color every other pixel, let neighbors share (half the coloring work)
- **4:2:0** = color one pixel per 2×2 block (quarter the coloring work)

The iPod used 4:2:2 and 4:2:0 because thumbnails are small and the loss is barely visible at that size.

**Formats in this project**:
- **UYVY** = 4:2:2 (horizontal sharing)
- **YCbCr 4:2:0** = 4:2:0 (2×2 block sharing)
- **CL/CLCL** = even more aggressive color compression using nibble-sized (4-bit) color values

---

## Nibble — 4 Bits

A **nibble** is half a byte (4 bits). CL and CLCL formats store color information in nibbles — each byte contains two pieces of color data (high nibble and low nibble), which is why they're called "nibble-chroma" formats.

## F-prefix and T-prefix

`.ithmb` files start with a 4-byte prefix that tells the decoder what's inside:

- **F-prefix** (e.g., `F1019_1.ithmb`): Contains raw pixel data (one of the formats above). The `F` stands for "raw Frame." The number after `F` is the [format ID](#profile--format-id). These are uncompressed (but still [lossy](#lossy-vs-lossless) due to bit depth / chroma subsampling).

- **T-prefix** (e.g., `T1007.ithmb`): Contains a JPEG image. The `T` stands for "Thumbnail." These are JPEG-compressed, which means they're lossy at the JPEG level too. T-prefix files are much smaller but need a JPEG decoder.

**The D-prefix mystery**: Some documentation mentions D-prefix files (PhotoDB / ArtworkDB). These aren't actually `.ithmb` files — they're database files that contain `.ithmb` data inside a chunked container format (see [PhotoDB](#photodb--artworkdb)).

---

## Profile / Format ID

A profile is a recipe that tells the decoder how to interpret the pixel data. It includes:
- Image dimensions (width × height)
- Encoding format (RGB565, UYVY, etc.)
- Byte length of the frame
- Post-processing flags (swap RGB channels, interlace, crop, rotation)

**Format IDs** are numbers like `1007`, `1019`, `1061` that Apple assigned to specific screen resolutions on specific iPod models. We have **54 known profiles** — the most complete public reference.

**Example**: Profile `1007` = 480×864 RGB565 (iPod Photo/Classic album art). Profile `1019` = interlaced UYVY 4:2:2 (iPod Classic 6G photo thumbnail).

---

## PhotoDB / ArtworkDB

These are **database files** (not `.ithmb` files) that Apple's iPod software uses to organize thumbnails. They're named `PhotoDB` (for camera photos) and `ArtworkDB` (for album art).

**Structure**: They use a binary chunk format similar to IFF or RIFF:
```
MHFD → MHSD → MHLI → ... → MHNI (actual thumbnail data)
```

Our codec can:
1. **Parse** these files (walk the chunk tree)
2. **Extract** individual thumbnails (inline pixel data or as external `.ithmb` file references)
3. **Write** new PhotoDB files (for syncing artwork to iPod without iTunes)
4. **Check integrity** (verify the structure is consistent)

**Why this matters**: Without PhotoDB support, you can only decode individual `.ithmb` files. With it, you can extract all photos from an iPod's PhotoDB in one operation.

---

## Frame / Multi-frame

Some `.ithmb` files contain multiple images concatenated together. Each image is one **frame**. Multi-frame files are common in iPod photo caches — a single file might contain multiple thumbnail sizes of the same photo.

Our CLI can extract individual frames with `--frame N`. Frame 0 is the first image. Out-of-range frame indices return an error.

T-prefix (JPEG) files are always single-frame.

---

## Pipeline

The pipeline is the code path that a file follows when you call `open_ithmb()`:

```
input → peek prefix → JPEG scan → profile lookup → decode → crop/rotate → BGRA output
                                                                  ↓
                                                          PhotoDB parser (if mhfd detected)
```

It's called a "pipeline" because data flows through stages like a factory assembly line. Each stage handles one job, then passes the result to the next.

---

## Macroblock

A **macroblock** is a group of pixels processed together. In YCbCr 4:2:0 format, a macroblock is 2×2 pixels — 4 pixels share the same color information. Our decoders process one macroblock at a time, checking cancellation between macroblocks.

## Cancellation

Decoding can be cancelled mid-operation via an `AtomicBool` flag. When the flag is set to `true`, the decoder checks at periodic points and returns a `Canceled` error.

**Why this matters**: If a user opens 100 thumbnails and closes the window, the codec can stop decoding the remaining files immediately instead of wasting CPU cycles. The cancellation polling checkpoints are at macroblock boundaries — small images decode too fast to cancel, but large images respect it.

---

## Crop and Rotation

After decoding, the pipeline can optionally crop the image to a region or rotate it by 90°, 180°, or 270°. These parameters come from the profile or the JPEG EXIF orientation tag.

**Crop**: Some iPod profiles store a full frame but only display a portion. Crop parameters (`crop_x`, `crop_y`, `crop_w`, `crop_h`) define the visible rectangle.

**Rotation**: JPEG files from iPhone cameras embed an orientation tag (0x0112 in EXIF). Our codec reads this and rotates the decoded image to match the display orientation (not the sensor orientation).

---

## EXIF — JPEG Camera Metadata

**EXIF** (Exchangeable Image File Format) is metadata embedded in JPEG files — things like camera model, date taken, and **orientation**. The orientation tag (0x0112) tells software which way is "up" on a photo. iPhones use EXIF orientation heavily.

Our JPEG decoder reads this orientation tag and rotates the decoded image to match the correct display orientation.

## SOI / EOI — JPEG Markers

JPEG files are divided into sections, each starting with a **marker** — a 2-byte code. The important ones:

- **SOI** (Start Of Image) = `FF D8` — every JPEG file starts with this
- **EOI** (End Of Image) = `FF D9` — every JPEG file ends with this
- **APP1** (Application Segment 1) — contains EXIF data, if present

Our codec scans for SOI to detect JPEG-embedded .ithmb files (T-prefix), then extracts the region from SOI to EOI for JPEG decoding.

## SIMD / SSE2 / AVX2 / NEON

**SIMD** (Single Instruction, Multiple Data) is a way for CPUs to process multiple pixels at once — like having 8 chefs instead of 1.

These are the SIMD instruction sets supported by different CPUs:

| Term | Full name | What it does | Available on |
|------|-----------|-------------|--------------|
| **SSE2** | Streaming SIMD Extensions 2 | 128-bit SIMD (4-8 pixels at once) | Every x86-64 CPU since 2004 |
| **SSSE3** | Supplemental Streaming SIMD Extensions 3 | Byte-shuffle for pixel unpacking | Intel Core 2 / AMD Bulldozer+ |
| **SSE4.1** | Streaming SIMD Extensions 4.1 | Packed min/max/clamp for YUV math | Intel Penryn / AMD Barcelona+ |
| **AVX2** | Advanced Vector Extensions 2 | 256-bit SIMD (8-16 pixels at once) | Intel Haswell (2013) / AMD Excavator (2015) |
| **NEON** | ARM Advanced SIMD (not an acronym) | 128-bit SIMD (similar to SSE2) | All ARM64 CPUs (Apple Silicon, Android) |

**Dispatch**: the codec checks your CPU at runtime and picks the fastest available path. SSE2 is guaranteed on any x86-64 machine. SIMD is only used where it measurably helps (YUV math).

## Morton / Z-order

**Morton order** (also called Z-order) is a way of arranging pixels in memory that keeps nearby pixels close together even in 2D space. It's called Z-order because the pixel traversal path looks like the letter Z. The ReorderedRGB555 format uses Morton order instead of the usual row-by-row layout.

This means pixel (0,0) is stored first, then (1,0), then (0,1), then (1,1) — the first 4 pixels form a 2x2 block before moving to the next block. Our decoder handles this with an algorithm called **Morton de-interleave**.

## BGRA Output — Why Blue First?

Most image formats store pixels as Red-Green-Blue (RGB). But Windows graphics APIs (DirectX, GDI) expect Blue-Green-Red-Alpha (BGRA). Our decoders output BGRA because:

1. The original C# codec targeted Windows (ImageGlass viewer on Windows)
2. PNG encoding libraries expect BGRA or RGB depending on configuration
3. BGRA has become a de facto standard for in-memory pixel buffers

The decoder always outputs BGRA. If you need plain RGB, it's a simple byte swap away.

---

## Endianness — Big Endian vs Little Endian

Endianness describes the order of bytes in a multi-byte number:

- **Little-endian** (LE): Least significant byte first. Used by Intel/AMD CPUs.
  - The number `0x1234` is stored as `[0x34, 0x12]`
- **Big-endian** (BE): Most significant byte first. Used by network protocols and older formats.
  - The number `0x1234` is stored as `[0x12, 0x34]`

iPod `.ithmb` files use **big-endian byte order** for the 4-byte prefix and multi-byte values (the iPod's CPU was PowerPC or ARM, both of which can do either). Some formats also have little-endian variants.

Our codec handles both. `profile.little_endian = false` means big-endian.

---

## CLI — Command-Line Interface

**CLI** means a program you run from the terminal/command prompt by typing commands. Our `ithmb` tool is a CLI — you run `ithmb input.ithmb output.png` from your terminal.

## CLI Tool

The `ithmb` command-line tool provides:

```bash
ithmb input.ithmb output.png        # Decode to PNG
ithmb --info input.ithmb              # Show metadata
ithmb --list-profiles                 # List all 54 profiles
ithmb --frame 2 input.ithmb out.png  # Extract specific frame
ithmb --raw input.ithmb output.bin   # Raw BGRA output (no PNG)
```

Built with `clap` for argument parsing and `png` crate for PNG output.

## C ABI

**C ABI** (Application Binary Interface) is a standard way for programming languages to call each other's code. By exposing our codec as a C ABI shared library (`.so` / `.dylib` / `.dll`), any language that can call C functions (Python, C++, Swift, Go, etc.) can use our decoder without knowing Rust.

The `ithmb-core-cabi` crate (now in its [own repository](https://github.com/B67687/Imageglass-Ithmb-Plugin)) implements the ImageGlass v10 plugin API (`ig_plugin_get_api()`), which is how the ImageGlass image viewer loads native codec plugins on Windows.

---

## FFI — Foreign Function Interface

**FFI** (Foreign Function Interface) is a way for code in one programming language to call code written in another language. The ImageGlass plugin [repo](https://github.com/B67687/Imageglass-Ithmb-Plugin) exposes a C FFI, which means Python (via ctypes), C++, Swift, Go, and other languages can call our decoder without needing to know Rust.

## PyO3 — Rust ↔ Python Bridge

**PyO3** is a Rust library that makes it easy to write Python modules in Rust. Our `pymod/` crate uses PyO3 to expose three functions to Python: `decode()`, `open_ithmb()`, and `list_profiles()`. Users can `pip install ithmb-python` and use it from Python scripts.

## cdylib — C Dynamic Library

**`cdylib`** (C dynamic library) is a Rust compilation mode that produces a `.so` (Linux), `.dylib` (macOS), or `.dll` (Windows) file that other programs can load at runtime. The ImageGlass plugin [repo](https://github.com/B67687/Imageglass-Ithmb-Plugin) compiles as a `cdylib`.

## Golden Tests

A "golden" file is a known-correct reference. For each format, we have:
- An `.ithmb` file (known-good encoding)
- A `.bin` file (expected decoder output, BGRA pixels)
- A `.meta` file (profile metadata)

When tests run, the decoder processes the `.ithmb` file and the test asserts the output matches the golden `.bin` byte-for-byte. If any code change alters decoded output, the golden test catches it.

---

## Fuzz Testing

Fuzz testing feeds random, malformed, or unexpected data to the decoder to see if it crashes or panics. This finds security bugs that normal testing misses.

Our fuzz suite includes:
- **libfuzzer targets** (2 targets): 1.2M+ iterations, 0 crashes
- **Random mutation fuzz**: 10,000 random byte mutations across all 8 decoders
- **Edge cases**: empty input, truncated data, negative dimensions, garbage data

---

## Miri

**Miri** is a Rust tool that interprets Rust code and checks for **Undefined Behavior (UB)** — things like reading uninitialized memory, violating pointer aliasing rules, or using SIMD instructions incorrectly. It's an interpreter, not a native runner — about **100–500× slower** than normal execution.

All our unsafe SIMD code paths pass Miri verification (21 tests). Six additional exhaustive tests (68,000+ total iterations across 5 values⁶ for YUV and 16⁴ nibble combinations for CL) are skipped under Miri because they would take minutes — the same tests complete in ~20ms natively. The skip uses standard Rust `#[cfg_attr(miri, ignore)]` convention, not a bug or infinite loop.

---

## LRU Cache

**LRU** = Least Recently Used. The LRU cache stores recently decoded file data in memory so that if the same file is requested again, it's served from memory instead of re-decoding from disk.

The cache is behind a `cache` feature flag (not enabled by default). Size limit: 128 entries. Thread-safe via `RwLock`.

---

## OnceLock

**`OnceLock`** is a Rust type for lazy initialization. It holds a value that gets set exactly once (the first time it's accessed) and then stays fixed forever. We use it for the profile database — it's loaded from JSON the first time it's needed, then cached.

## LTO / PGO — Build Optimization

**LTO** (Link-Time Optimization) lets the compiler optimize across file boundaries during the linking step — like letting the chef rearrange the entire kitchen instead of each station independently.

**PGO** (Profile-Guided Optimization) runs the program first, observes which code paths are most used, then recompiles with that knowledge — like a restaurant that watches which dishes are most popular and rearranges the kitchen to make those faster.

Both are available in release builds but not currently configured in `Cargo.toml`.

---

## Further Reading

- [`what-is-this.md`](what-is-this.md) — What .ithmb files are
- [`FORMAT.md`](FORMAT.md) — Technical format specification (detailed)
- [`README.md`](../README.md) — Project overview
