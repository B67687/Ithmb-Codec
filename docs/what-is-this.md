# What Is This?

This is a plugin for [ImageGlass](https://imageglass.org) (an image viewer for Windows) that lets it open `.ithmb` files.

## What's an .ithmb File?

When you sync photos to an iPod, iPhone, or iPod Touch via iTunes, the device creates small thumbnail versions of your photos and stores them in `.ithmb` files. These files live in the device's internal storage under `PhotoData/Thumbnails/` (or a similar path).

There are two kinds:

- **T-prefix** files (like `T1024_1.ithmb`) — contain a regular JPEG image inside. These are straightforward to decode.
- **F-prefix** files (like `F1019_1.ithmb`) — contain raw pixel data in various Apple-specific formats. These are the hard ones.

## What Does the Codec Do?

It reads an `.ithmb` file and converts it to a normal image (BGRA pixels) that ImageGlass can display.

The logic goes:

1. Look for a JPEG inside the file (check the first few bytes for JPEG markers). If found, extract and decode the JPEG.
2. If no JPEG, check the file's 4-byte prefix against a list of known profiles. Each profile describes the image dimensions, pixel format, and encoding. Pick the matching decoder and run it.
3. If the file starts with `mhfd`, it's a PhotoDB/ArtworkDB database — parse the chunk structure, find the thumbnail entries, and decode each one.
4. If nothing matches, reject the file.

## What's a Profile?

A profile is a set of settings that tells the decoder how to interpret a raw .ithmb file. Each profile has:

- A **format ID** — a number like 1019 or 1024 that identifies the image format
- **Width and height** — the image dimensions
- An **encoding** — which pixel format to use (RGB565, RGB555, UYVY, YCbCr420, CLCL, or CL)
- Various flags — things like whether the pixel data is packed or padded, whether channels are swapped, etc.

There are 53 built-in profiles covering known iPod/iPhone devices from 2004 through 2016.

## What's a Decoder?

A decoder is code that converts raw pixel data from one format to BGRA (blue, green, red, alpha — what ImageGlass displays). This project has 7 decoders:

| Decoder | Used by iPod/iPhone |
|---------|-------------------|
| RGB565 | 16-bit RGB (5 bits red, 6 bits green, 5 bits blue) |
| RGB555 | 15-bit RGB (5 bits per channel) |
| UYVY | YUV 4:2:2 (luminance + color difference, packed) |
| YCbCr 4:2:0 | YUV with subsampled color (common in video) |
| YUV422 Interlaced | Same as UYVY but stored as two interleaved fields |
| CLCL | Nibble-based chroma (compact Apple-specific format) |
| CL | Per-pixel chroma (another Apple-specific format) |

## What's SIMD?

SIMD (Single Instruction, Multiple Data) is a technique where the CPU processes multiple pixels at once instead of one at a time. This makes decoding 4-6× faster. This project uses SSE2 (on x64 Intel/AMD CPUs) and NEON (on ARM64 CPUs like Apple Silicon and Raspberry Pi).

## What's PhotoDB?

iPods and iPhones don't just store individual `.ithmb` files — they also have a database file (PhotoDB or ArtworkDB) that catalogs all the thumbnails. This database uses a binary chunk format. The plugin can:

- **Read** the database, find all thumbnails, and decode them
- **Write** a new database from scratch (useful for syncing artwork back to an iPod without iTunes)
- **Check integrity** — validate that a database file is well-formed

## What's the CLI?

The `tools/IthmbDecoder/` directory has a command-line tool that doesn't need ImageGlass. It can:

- `IthmbDecoder file.ithmb out.bmp` — decode a single file to BMP
- `IthmbDecoder --list-pd PhotoDB` — list all entries in a PhotoDB file
- `IthmbDecoder --pd-index 0 PhotoDB` — decode entry 0 from a PhotoDB
- `IthmbDecoder --extract-all-pd PhotoDB` — decode all entries from a PhotoDB
- `IthmbDecoder --check-pd PhotoDB` — validate a PhotoDB file
- `IthmbDecoder --list-devices` — show which formats each iPod model uses
- `IthmbDecoder --help` — show all options

## Why BMP?

The CLI outputs BMP files because BMP is the simplest image format — a short header followed by raw pixel data. No compression, no encoding libraries needed. The decoder already has the raw BGRA pixels in memory, so writing them to a BMP file is just adding a 54-byte header and saving.

ImageMagick can convert BMP to anything else (`magick out.bmp out.png`), or you can use the ImageMagick delegate to open `.ithmb` files directly:

```
magick file.ithmb out.jpg
```
