# Getting Photos Off an iPod — A Practical Guide

## What You'll Need

- An iPod Classic, Nano, Touch, or iPhone (any model that syncs photos via iTunes)
- A computer with USB port (Windows, Mac, or Linux)
- The `ithmb` CLI tool ([build from source](../README.md#build) or download a release)
- Optional: a PNG viewer to look at extracted photos

## Step 1: Connect Your iPod

Plug your iPod into your computer via USB. On Windows, iTunes should open automatically. On Mac, it appears in **Finder** (macOS Catalina+) or launches **iTunes** (older macOS). On Linux, the iPod should mount as a USB mass storage device (latest iPod Classic 6G/7G only; older models require libgpod).

> **Important**: Your iPod must NOT be encrypted or passcode-locked. The file system must be readable.

## Step 2: Find the Thumbnail Files

On the iPod's internal storage, navigate to:

```
/iPod_Control/Photos/PhotoData/
```

or (older models):

```
/Photos/Thumbnails/
```

You should see files like:

```
PhotoDB      — the photo database (contains multiple thumbnails)
ArtworkDB    — album artwork database
F1019_1.ithmb — individual thumbnail files
T1007.ithmb   — JPEG-embedded thumbnails
```

> **Can't find it?** On Windows, the iPod appears as a portable device in "This PC" — you may need to enable "Show hidden files" in File Explorer. On Mac, check `~/Music/iTunes/iPod Photo Cache/`.

## Step 3: Extract Thumbnails

### Option A: Individual .ithmb files (simplest)

```bash
# Decode a single file to PNG
ithmb F1019_1.ithmb thumbnail.png

# Decode and show metadata only
ithmb --info F1019_1.ithmb

# Extract a specific frame from a multi-frame file
ithmb --frame 2 F1019_1.ithmb frame2.png
```

### Option B: PhotoDB container (all photos in one file)

If you have a `PhotoDB` or `ArtworkDB` file:

```bash
# Open the container and decode all entries
ithmb --open PhotoDB
```

This scans the container chunk structure, extracts every thumbnail, and writes them as PNG files named `thumb_0001.png`, `thumb_0002.png`, etc.

### Option C: Using Python (for scripting)

```python
import ithmb_core

# Decode a single file
with open("F1019_1.ithmb", "rb") as f:
    result = ithmb_core.decode_ithmb(f.read())
    # result is a dict with "width", "height", "data" (BGRA bytes)
    width, height = result["width"], result["height"]
    pixels = result["data"]  # bytes, length = width * height * 4

# Open a PhotoDB container
with open("PhotoDB", "rb") as f:
    images = ithmb_core.open_ithmb(f.read())
    for i, img in enumerate(images):
        print(f"Image {i}: {img['width']}x{img['height']}")
```

## Step 4: What You'll Get

The decoded output is **BGRA** pixel data (Blue-Green-Red-Alpha). When saved as PNG, the colors will look correct in any standard image viewer.

**Quality note**: iPod thumbnails are lossy. They use reduced color precision (5-6 bits per channel) or chroma subsampling. A 320×320 thumbnail decoded from an iPod will not look as sharp as the original photo on your phone. That's normal — it's how the iPod stored them to save space.

## What NOT To Do

- **Don't write to the iPod's filesystem** unless you know what you're doing. The decoder is read-only.
- **Don't expect full-resolution photos**. The iPod only stores thumbnails, not originals. To recover full photos, use a tool that accesses the iPod's photo database directly (like iMazing or libgpod).
- **Don't use `--open` on random files**. It's designed for PhotoDB/ArtworkDB files. Bare .ithmb files should use the default decoder.

## Troubleshooting

| Symptom | Likely cause / Fix |
|---------|-------------------|
| "File not found: PhotoDB" | The file path is wrong — double-check the iPod's file structure |
| "Buffer too short" | The file is truncated or not a valid .ithmb file |
| "Unknown format prefix" | This iPod may use a format variant not yet documented. [Open an issue](https://github.com/B67687/Ithmb-Codec/issues) with a sample |
| Garbled image | JPEG false positive or wrong format ID. Try with `--info` to see metadata first |
| "32 MB file size guard" | File is unreasonably large for a thumbnail. May not be a valid .ithmb file |

## Further Reading

- [`what-is-this.md`](what-is-this.md) — What .ithmb files are
- [`GLOSSARY.md`](GLOSSARY.md) — Explanation of technical terms
- [`ECOSYSTEM.md`](ECOSYSTEM.md) — Research contributions to the .ithmb format
- [GitHub Issues](https://github.com/B67687/Ithmb-Codec/issues) — Report problems or ask questions
