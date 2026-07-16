# ithmb-core

Pure Rust decoder and encoder for Apple `.ithmb` thumbnail-cache files — the format used by iPod Classic/Nano/Photo/Video, iPhone 2G, and iPod Touch to store photo and album art thumbnails.

**Part of the [Ithmb-Codec](https://github.com/B67687/Ithmb-Codec) workspace.** See the root README for full documentation, CLI tooling, Python bindings, and architecture overview.

## Features

- **8 decoders** — RGB565, RGB555, ReorderedRGB555, UYVY (linear + interlaced), YCbCr 4:2:0, CLCL nibble-chroma, CL per-pixel chroma, JPEG-embedded
- **54 built-in profiles** covering known iPod/iPhone formats
- **PhotoDB/ArtworkDB** binary chunk parser, writer, and integrity checker
- **SIMD acceleration (SSE2/AVX2/NEON runtime dispatch))
- **Multi-frame** F-prefix raw file support
- **JPEG carving** fallback for non-standard file layouts
- **Synthetic encoders** for all raw pixel formats (roundtrip-tested)
- **Cancellation** via `&AtomicBool`

## Usage

```toml
[dependencies]
ithmb-core = { git = "https://github.com/B67687/Ithmb-Codec", branch = "main" }

# Or from crates.io:
# ithmb-core = "1.9.1"
```

### Basic decode

```rust
use ithmb_core::decode_ithmb;
use std::sync::atomic::AtomicBool;

let data = std::fs::read("photo.ithmb").unwrap();
let canceled = AtomicBool::new(false);
match decode_ithmb(&data, &canceled) {
    Ok(img) => {
        // img.data: Vec<u8> — BGRA pixel data
        // img.width, img.height: decoded dimensions
        println!("Decoded {}x{} image", img.width, img.height);
    }
    Err(e) => eprintln!("Decode failed: {e}"),
}
```

### With a specific profile

```rust
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile_db::ProfileDb;
use std::sync::atomic::AtomicBool;

let data = std::fs::read("F1061_1.ithmb").unwrap();
let db = ProfileDb::load_builtin().unwrap();
let profile = db.get(1061).cloned().unwrap();
let canceled = AtomicBool::new(false);
let img = decode_with_profile(&data, &profile, &canceled).unwrap();
// img.data, img.width, img.height
```

### PhotoDB/ArtworkDB container

```rust
use ithmb_core::photodb::parser::try_parse_photodb;

let data = std::fs::read("PhotoDB").unwrap();
let mut entries = Vec::new();
try_parse_photodb(&data, &mut entries).unwrap();
for entry in &entries {
    println!("Frame: {}x{} format_id={}", entry.width, entry.height, entry.format_id);
}
```

## Crate features

| Feature   | Description            |
| --------- | ---------------------- |
| `c`       | C ABI exports          |
| `cache`   | LRU raw file cache     |
| `logging` | Logging support        |
| `metrics` | Decode timing counters |

## License

MIT
