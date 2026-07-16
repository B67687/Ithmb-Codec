# AGENTS.md — AI Agent Guide for Ithmb-Codec

This file tells AI coding agents (Claude Code, Copilot, Cursor, Codex) how to work with this repository effectively. Read this first before editing any code.

## Repository Purpose

Pure Rust codec for Apple `.ithmb` thumbnail-cache files (iPod/iPhone photo thumbnails). Decodes 8 raw pixel formats, encodes 7, parses PhotoDB/ArtworkDB containers. Published on crates.io as `ithmb-core`.

## Workspace Layout

```
Ithmb-Codec/
├── crates/
│   ├── ithmb-core/       # Core library (lib) — published to crates.io
│   │   └── src/
│   │       ├── pipeline/      # Decode entry points (open_ithmb, decode_ithmb, decode_with_profile)
│   │       ├── profile.rs     # Profile type + lookup
│   │       ├── profile_db.rs  # Static profile database (54 profiles)
│   │       ├── photodb/       # PhotoDB/ArtworkDB chunk parser
│   │       ├── enc/           # 7 synthetic encoders
│   │       ├── rgb565.rs      # RGB565/RGB555 decoder
│   │       ├── jpeg.rs        # JPEG-embedded decoder
│   │       ├── simd/          # SSE2/AVX2/NEON YUV conversion
│   │       └── error.rs       # DecodeError enum
│   ├── ithmb-cli/        # CLI binary (cargo install ithmb-cli)
│   ├── ithmb-gen/        # Synthetic sample generator binary
│   └── ithmb-wasm/       # WASM target (wasm-pack)
├── pymod/                # Python bindings (PyO3/maturin)
├── fuzz/                 # libfuzzer targets (3 targets)
└── docs/                 # Documentation
    ├── adr/              # Architecture Decision Records
    ├── guides/           # GUIDE.md, HARDWARE_GUIDE.md
    ├── standards/        # STANDARDS.md, RUST_STANDARDS.md
    └── benchmarks/       # BENCHMARKS.md
```

## Decoder Pipeline Flow

```
.ithmb file → peek prefix → JPEG scan → profile lookup → decode → crop/rotate → BGRA output
                  ↑                              ↓
            PhotoDB (mhfd)              Unknown prefix → fallback → JPEG carving
```

1. If file starts with JPEG SOI (`FF D8`) → decode as JPEG
2. If file starts with `mhfd` → parse as PhotoDB/ArtworkDB container
3. If 4-byte prefix matches a known profile → decode raw pixels
4. If prefix unknown → scan for embedded JPEG markers (carving)

## Code Conventions

- **Strictness**: `#![deny(clippy::pedantic)]` across workspace — every pedantic lint is an error
- **Unsafe**: `unsafe_code = "deny"` at workspace level; individual unsafe blocks use `#[allow(unsafe_code)]`
- **No `unwrap()`**: Use `?` or `.expect("reason")` — never bare `.unwrap()`
- **No `as any` / type erasure**: Exhaustive error types, no lossy casts
- **250 LOC ceiling**: Files > 250 lines of pure logic need a `// SIZE_OK` comment or splitting
- **Edition**: Rust 2024, MSRV 1.88

## Test Patterns

Run in this order:

```bash
cargo check                     # Catches 90% of errors (~5s)
cargo clippy --fix --allow-dirty  # Auto-fix mechanical lints
cargo test --workspace          # Full suite (~40-60s)
```

Test files use `#![allow(clippy::pedantic, clippy::unwrap_used)]` (test files are exempt from production strictness).

Key test categories (see STATS.md for live counts):

- **Golden vectors**: Reference `.ithmb` → expected `.bin` byte-for-byte comparison
- **Exhaustive roundtrip**: All 65,536 RGB565 values, all 32,768 RGB555 values
- **SIMD tail**: 42 boundary widths (1..65) verifying SIMD matches scalar
- **Fuzz**: 3 libfuzzer targets + 10,000+ random byte mutations
- **Concurrency**: 11 stress scenarios (Barrier sync, cancellation, cache contention)
- **Profile validation**: All 54 profiles decode without error

## What NOT to Do

- Do NOT add new dependencies without checking if existing ones cover the need
- Do NOT suppress type errors with `as _`, `#[allow]`, or `expect("unreachable")`
- Do NOT edit `deny.toml` or CI workflows without understanding the full impact
- Do NOT commit without running `cargo check` on the changed crate first
- Do NOT add attribution lines (`Co-authored-by`, `Ultraworked with`) to commit messages

## Building

```bash
cargo build --workspace           # All crates
cargo build --release       # SIMD always compiled for x86_64 and aarch64
cargo build -p ithmb-core         # Just the library
wasm-pack build crates/ithmb-wasm # WASM target (requires wasm-pack)
maturin develop --release -m pymod/Cargo.toml  # Python bindings
```

## Key Decisions

# - **SIMD compiled unconditionally** — not default. SSE2/AVX2 for x64, NEON for ARM64 (macOS ARM NEON fixed in v1.9.3 — full acceleration on Apple Silicon)

- **C ABI plugin in separate repo** — [Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin)
- **54 built-in profiles** — embedded in binary, optionally overridable via external `profiles.json`
- **File size guard**: 8 MB max (ADR-0005), covers all known real-world files with 10× margin
