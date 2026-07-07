# ithmb-core

Pure Rust decoder and encoder for Apple `.ithmb` thumbnail-cache files — the format used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo and album art thumbnails.

**Part of the [Ithmb-Codec](https://github.com/B67687/Ithmb-Codec) workspace.** See the root README for full documentation, CLI tooling, Python bindings, and architecture overview.

## Features

- **8 decoders** — RGB565, RGB555, ReorderedRGB555, UYVY (linear + interlaced), YCbCr 4:2:0, CLCL nibble-chroma, CL per-pixel chroma, JPEG-embedded
- **54 built-in profiles** covering known iPod/iPhone formats
- **PhotoDB/ArtworkDB** binary chunk parser, writer, and integrity checker
- **SIMD acceleration** — SSE2/AVX2/NEON runtime dispatch for YUV conversion (feature-gated)
- **Multi-frame** F-prefix raw file support
- **JPEG carving** fallback for non-standard file layouts
- **Synthetic encoders** for all raw pixel formats (roundtrip-tested)
- **Cancellation** via `&AtomicBool`

## Usage

```toml
[dependencies]
ithmb-core = { git = "https://github.com/B67687/Ithmb-Codec", branch = "main" }

# Or from crates.io:
# ithmb-core = "1.9.0"
```

```rust
use ithmb_core::pipeline::decode_bytes;

let data = std::fs::read("photo.ithmb").unwrap();
let img = decode_bytes(&data, &Default::default()).unwrap();
// img.data: Vec<u8> — BGRA pixel data
// img.width, img.height: decoded dimensions
```

## Crate features

| Feature | Description |
|---------|-------------|
| `simd` | SSE2/AVX2/NEON YUV conversion |
| `cache` | LRU raw file cache |
| `metrics` | Decode timing counters |

## License

MIT
