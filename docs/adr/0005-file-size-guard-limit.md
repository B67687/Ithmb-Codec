# ADR-0005: File Size Guard Limit

**Status**: Revised (2026-07-07)
**Context**: The C# codebase defined a 32 MB `MaxDecodeFileSize` limit (see `IthmbCodecPlugin.Helpers.cs:29`). The Rust port initially omitted this guard entirely — the README documented it but the code never enforced it. We needed to implement the guard properly and determine the right limit for the Rust codec.

## Decision

Keep the **8 MB** limit. This is tighter than the C# original (32 MB) but still provides ~10× margin on the largest known frame (810 KB). The C# limit of 32 MB was arbitrary — 40× margin on a 810 KB frame is excessive. 8 MB is a reasonable engineering safety margin that covers all known real-world usage while still catching pathological inputs early.

## Research Sources

### 1. Profile Database (this codebase)

The `data/profiles.json` defines 54 raw-format profiles. The largest frame is P1007 (480×864 RGB565) at **829,440 bytes** (~810 KB). Average frame size across all profiles: ~118 KB.

```python
# From crates/ithmb-core/data/profiles.json
# Max:  829,440 bytes (prefix 1007, 480×864 Rgb565)
# Avg:  121,259 bytes (non-zero only)
# 32 MB / max frame = 40.5 frames
```

All 54 profile frame sizes are under 1 MB.

### 2. iPod Firmware Limit (iLounge Forum, 2005)

The most authoritative documented limit comes from iLounge forum analysis published in 2005, referenced by multiple OSS implementations (Keith's iPod Photo Reader, ithmbrdr):

> "The ITHMB file format seems to cap off at around **500 MB**, at which point a new file is created."
> — iLounge Forums, "Photo Storage on the iPod — The Gory Details" (2005)

This 500 MB cap is the actual iPod firmware limitation. Multiple files (e.g. F1019_1, F1019_2, F1019_3) appear when the thumbnail cache exceeds this threshold. The limit is consistent across 4G Photo iPod, 5G iPod Video, and Nano generations.

Confirmed by `cyianor/ithmbrdr` (Go):

> "Files called F1067_x.ithmb where x indicates the chunk, since these files are broken apart if there are too many images in one."

### 3. libgpod Limit (256 MB)

The libgpod project cites a 256 MB limit. This is an application-level buffer allocation limit, not an iPod firmware constant. No evidence links it to any Apple specification.

### 4. Real-World Samples

| Source | File | Size |
|--------|------|------|
| This codebase | `samples/reuhno-reference/F1060_1.ithmb` | 2.0 MB |
| This codebase | `samples/reuhno-reference/F1055_1.ithmb` | 320 KB |
| This codebase | `samples/reuhno-reference/F1061_1.ithmb` | 64 KB |
| This codebase | `samples/synthetic/sample.ithmb` | 152 KB |
| iLounge (2005) | F1024_1.ithmb (1743 frames) | 255 MB |
| iLounge (2005) | F1019_1.ithmb (759+ frames) | 500 MB |

The largest confirmed real file (F1024_1 at 255 MB from 2005) fits within the iPod's 500 MB firmware cap but exceeds both libgpod's 256 MB limit and the original C# codec's 32 MB guard. The Rust codec uses **8 MB** (see Decision).

## Consequences

### Positive
- **8 MB protects against OOM** on low-memory devices (ImageGlass plugin on 32-bit Windows) while maintaining ~10× margin over the largest known frame.
- **Zero false positives** for single-frame decode — no real thumbnail exceeds 810 KB.
- **Free operation** — the size check is a single `io::Error` before any allocation.
- **PhotoDB containers stay under limit** — a real ArtworkDB (5-20 frames) is at most ~16 MB (the 8 MB guard applies per blob after container splitting, not to the container itself).

### Negative
- **8 MB will reject** very large multi-frame containers (500+ album art entries, or a full PhotoDB with thousands of photos) at the individual blob level. These are decoded via `open_ithmb()` in the CLI, which splits the container before decoding.
- **Mitigation**: The guard is checked per-blob after `open_ithmb` splits the container. A real 255 MB F1024 file would be rejected at the blob level if any individual frame exceeds the 8 MB guard — but no single frame exceeds 810 KB.

### Comparison Table

| Limit | Source | Basis | Single-Frame Coverage | Multi-Frame Coverage |
|-------|--------|-------|----------------------|---------------------|
| 8 MB | This codec (Rust, revised) | Engineering safety margin | ✅ All frames | ✅ Up to 10 max-size frames |
| 32 MB | This codec (C# original) | Arbitrary (40× margin) | ✅ All frames | ✅ Up to 40 max-size frames |
| 256 MB | libgpod | Application buffer | ✅ All frames | ✅ Up to 315 max-size frames |
| 500 MB | iPod firmware | Hardware memory map | ✅ All frames | ✅ All known collections |
