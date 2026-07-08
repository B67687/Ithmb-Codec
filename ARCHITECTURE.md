```
┌─────────────────────────────────────────────────────────────┐
│                   Ithmb-Codec (workspace)                    │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  crates/ithmb-core          [lib]                     │   │
│  │  ───────────────────────                              │   │
│  │  • 7 decoders (RGB565..JPEG)                         │   │
│  │  • 54 built-in profiles                              │   │
│  │  • PhotoDB/ArtworkDB parser                          │   │
│  │  • 7 encoders                                        │   │
│  │  • SIMD (SSE2/AVX2/NEON)                             │   │
│  │  • LRU cache (--features cache)                     │   │
│  │  • C API header (--features c)                       │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                          │                                   │
│          ┌───────────────┼───────────────┐                   │
│          │               │               │                   │
│  ┌───────▼──────┐ ┌─────▼──────┐ ┌──────▼──────┐            │
│  │ ithmb-cli    │ │ ithmb-wasm  │ │  pymod/     │            │
│  │  [bin]       │ │  [cdylib]   │ │  [PyO3]     │            │
│  │              │ │              │ │             │            │
│  │ cargo        │ │ wasm-pack    │ │ pip install │            │
│  │ install      │ │ → .wasm      │ │ → .so      │            │
│  └──────────────┘ └──────────────┘ └─────────────┘            │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐                          │
│  │ ithmb-gen    │  │  fuzz/       │                          │
│  │  [bin]       │  │  [libfuzzer] │                          │
│  │  sample gen  │  │  3 targets   │                          │
│  └──────────────┘  └──────────────┘                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘


                     ┌──────────────────────┐
                     │ Imageglass-Ithmb-Plugin│
                     │ (separate repo)       │
                     │                      │
                     │ Uses ithmb-core      │
                     │ via cargo dep        │
                     │ + ig_plugin C ABI    │
                     └──────────────────────┘
```

**How the APIs serve different audiences:**

| Interface | Who uses it | Why it exists |
|-----------|-------------|---------------|
| **`ithmb-core`** (Rust lib) | Rust projects | `cargo add ithmb-core` |
| **`ithmb-cli`** (CLI) | End users | `cargo install ithmb-cli` to decode `.ithmb` files |
| **`ithmb-python`** (PyO3) | Python devs | `pip install ithmb-python` for ML/scraping |
| **`ithmb-wasm`** (WASM) | Browser/web | Drag-drop demo page, WASM from any web app |
| **C API** (`--features c`) | Any language | Ruby `ffi`, Go `cgo`, Zig, etc. — no PyO3 needed |
| **ImageGlass plugin** | ImageGlass users | Separate repo for `ig_plugin_get_api()` |
| **`ithmb-gen`** (encoder) | Devs/testers | Generate synthetic `.ithmb` files for validation |
