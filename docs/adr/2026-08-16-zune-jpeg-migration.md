# ADR-0007: Migrate JPEG decoding from jpeg-decoder to zune-jpeg

**Status:** Accepted (2026-08-16)

## Context

The core's thumbnail decoding path used `jpeg-decoder` 0.3 (a well-regarded pure-Rust JPEG decoder) for baseline/progressive JPEG frames embedded in `.ithmb` files. In 2026 the `jpeg-decoder` crate entered **maintenance mode**: the image-rs organization is migrating its decoder backend to `zune-jpeg`, and `jpeg-decoder` is now kept alive only for lossless-JPEG support (a niche DICOM-oriented format `.ithmb` files never contain — Apple only ever wrote baseline JPEG thumbnails). The crate was effectively dormant (~14 months without meaningful activity at assessment time).

Key facts weighed:

1. **`.ithmb` content is baseline JPEG only.** Apple's iPod Photo Cache pipeline wrote 8-bit baseline JPEG thumbnails. No iPod firmware variant ever produced lossless (predictive-mode) JPEG, progressive is rare/absent, and arithmetic coding is unsupported by the format. Therefore the one capability `jpeg-decoder` retains over `zune-jpeg` (lossless JPEG) is dead weight for this codebase — migrating loses **zero** decoding capability.
2. **`zune-jpeg` 0.5 is actively maintained**, ships SIMD-accelerated paths (SSE2/NEON), and is the default JPEG backend of `image` 0.25 — the ecosystem consensus choice.
3. **Security posture**: the migration preserves the CWE-400 allocation-abort guard (see below) — the single most important property of the decode path.
4. **Downstream consumers**: the WebAssembly build and the ImageGlass C-ABI plugin statically link the core, so both pick up the new decoder on the next regen/rebuild.

## Decision

**Replace `jpeg-decoder = "0.3"` with `zune-jpeg = "0.5"` in the core workspace dependency set, rewriting the JPEG decode path in `crates/ithmb-core/src/jpeg.rs` around `zune_jpeg::JpegDecoder`.**

### CWE-400 allocation guard (must never regress)

`jpeg-decoder` 0.3.2 allocated the progressive-JPEG coefficient buffer at first SOS from frame dimensions alone — a 166-byte SOF2-65535×65535 stream triggered an ~8 GiB allocation that aborts the process. The guard, preserved in the migration:

- `decode_headers()` parses the SOF header with **zero pixel allocations** (stops before any coefficient buffer)
- An explicit `width × height × 11 > MAX_JPEG_PIXEL_BYTES (256 MiB)` pre-check is the **sole** budget gate (11 bytes/px is the measured worst-case working set)
- `set_max_width/max_height(u16::MAX)` remains as an inert belt-and-braces (SOF-time, behavior-identical to the old `set_max_decoding_buffer_size`)

Regression coverage: `alloc_contract.rs` keeps the 193-byte SOF2-65535×65535 fixture that must be rejected.

### Output identity

Both decoders emit standard RGB for baseline/progressive JPEG. Pixel-level comparison between old and new decoders on a real 2000×470 JPEG showed **0.135% of channels differing, all within ±1-2 LSB** (max 2, mean 1.01) — standard IDCT rounding tolerance, spec-permitted, visually imperceptible. Downstream exact-hash comparisons of decoded cover art will see up to ±3/255 deltas; consumers comparing byte-exact decoded output across core versions must account for this (noted in the 1.9.9 release notes).

Side benefit: grayscale JPEGs (single-channel) now decode correctly (previously mishandled).

## Consequences

- **Positive**: actively maintained dependency; SIMD acceleration; ecosystem alignment; one fewer maintenance-mode crate; grayscale JPEG support fixed.
- **Negative**: wasm bundle grows (247 KB vs 200 KB at 1.9.6) and native binaries shrink (the removed jpeg-decoder dependency tree outweighed zune-jpeg's size) — both benign. Lossless-JPEG decoding is no longer possible (never needed by the format).
- **Neutral**: decoded pixel output varies by ±1-3/255 from the previous decoder due to IDCT rounding; encode path unaffected.

## Related

- ADR-0006 (Dependency Management Policy) — this migration is the first application of the "audit and swap on maintenance risk" discipline.
- `zune-jpeg` documentation: https://docs.rs/zune-jpeg — API surface used: `JpegDecoder::new`, `decode_headers`, `info`, `decode`, `DecoderOptions`.
- CWE-400 guard rationale: see comments in `crates/ithmb-core/src/jpeg.rs` (MAX_JPEG_PIXEL_BYTES, w·h·11 pre-check).
