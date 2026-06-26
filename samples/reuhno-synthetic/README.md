# Synthetic F-prefix `.ithmb` test vectors — iPod Classic 6/7G profiles

**Copyright-free, generated** test data that reproduces the iPod Classic 6/7G artwork-cache
format, for validating raw (F-prefix) `.ithmb` decoders and `ArtworkDB` parsers. No real
album art, no library info — every pixel is a generated pattern. Released as **CC0 / public
domain**: host, modify, and credit however you like.

Validated parseable by an independent parser (10-entry `ArtworkDB` walks cleanly, all 3 sizes,
one slot per thumbnail) and every raw thumbnail's `sha256` round-trips against `manifest.csv`.

## Profiles (all raw RGB565, little-endian, no alpha, no R/B swap)

| file | **slot** W×H (fixed) | bytes/thumb (stride) | row stride |
|---|---|---|---|
| `F1061_1.ithmb` | **56×55** | 6160 | 112 bytes (56 px) |
| `F1055_1.ithmb` | 128×128 | 32768 | 256 bytes |
| `F1060_1.ithmb` | 320×320 | 204800 | 640 bytes |

## ⚠️ The key subtlety: slot geometry vs. declared (content) rectangle

The **slot** dimensions above are *fixed per correlation* and are what you decode. The
`mhni` **declared `width`/`height` is a content rectangle inside the slot**, not the buffer
geometry.

For `F1061` the slot is **always 56×55**, but the declared rect varies (this set includes
`55×55, 55×54, 55×52, 54×55, 48×55, 43×55, 55×48, 44×55, 52×55, 55×50`). So:

- `stride = slot_width = 56` (112 bytes) — **constant**, regardless of declared width.
- The rule `stored_width = byte_size / 2 / height` only gives 56 when you use the **slot
  height (55)** — using the *declared* height (e.g. 54) yields 57 and misaligns every row.
  Derive the slot geometry from the correlation, not from the declared rect.
- To make stride/crop bugs **visually obvious**, every pixel outside the declared rect
  (`x ≥ declared_w` or `y ≥ declared_h`) is filled **magenta** (`0xF81F`, RGB 248,0,248).
  A correct decoder shows the pattern in the content rect and a clean magenta border in the
  padding; a buggy one smears the magenta diagonally.

`F1055` / `F1060` are square — slot = declared, no padding.

## Contents

- `ArtworkDB` — valid `mhfd → mhsd(image list) → mhli → mhii × 10 → mhni × 3`. Synthetic
  opaque dbids. (Only the image-list `mhsd` is included — the album/file-list sections aren't
  needed to exercise F-prefix decoding; the `mhni` atoms carry filename + offset + size + dims.)
- `F1061_1.ithmb`, `F1055_1.ithmb`, `F1060_1.ithmb` — 10 thumbnails each.
- `manifest.csv` — `file, offset, size_bytes, declared_w, declared_h, slot_w, slot_h,
  raw_sha256, png`. **`raw_sha256` (of the raw slot bytes) is the canonical ground truth** —
  it's decoder-convention-independent.
- `reference-decodes/*.png` — 30 decoded references for eyeballing.

## Patterns
Cycled across entries: horizontal R gradient, vertical B gradient, 8-px checkerboard,
diagonal G gradient, 8-bar color bars, 1-px checkerboard (stride stress).

## Note on the PNGs vs. raw bytes
The reference PNGs are decoded with **MSB replication** (`r8 = (r5<<3)|(r5>>2)`, full 0–255
range). If your decoder uses a plain left-shift (`r5<<3`, 0–248) the PNGs won't byte-match —
that's expected. **Validate against `raw_sha256`**, not the PNGs (or regenerate PNGs in your
own convention).
