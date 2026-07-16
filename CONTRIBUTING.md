# Contributing to Ithmb-Codec

Thanks for your interest. This project is a pure Rust codec for Apple `.ithmb` thumbnail-cache files. It uses strict lints, no `unsafe` by default, and exhaustive error handling. The guide below covers what you need to get started.

## Development Setup

**Rust toolchain.** Install via [rustup](https://rustup.rs). The project pins `channel = "1.88.0"` and `edition = "2024"` in `rust-toolchain.toml` at the repository root -- `rustup` auto-detects and installs the correct version.

**Recommended tools.**

| Tool           | Install                             | Purpose                                                 |
| -------------- | ----------------------------------- | ------------------------------------------------------- |
| `cargo clippy` | Ships with Rust                     | Lint checking (all pedantic lints are errors)           |
| `cargo fmt`    | Ships with Rust                     | Code formatting (`rustfmt.toml`: 120 col, 4-space tabs) |
| `wasm-pack`    | `cargo install wasm-pack`           | WASM target builds                                      |
| `cargo-fuzz`   | `cargo +nightly install cargo-fuzz` | Fuzz testing (requires nightly)                         |
| `cargo audit`  | `cargo install cargo-audit`         | Advisory checking before release                        |

**Building.**

```bash
# Full workspace
cargo build --workspace

# Just the library
cargo build -p ithmb-core

# With SIMD acceleration (cache feature enables SIMD runtime dispatch)
cargo build --features cache -p ithmb-core

# WASM target
wasm-pack build crates/ithmb-wasm

# Python bindings
maturin develop --release -m pymod/Cargo.toml
```

Available features for `ithmb-core`: `cache` (LRU raw file cache), `metrics` (decode timing counters), `c` (C ABI).

## Code Style

**Strictness.** The workspace denies all clippy pedantic lints via `[workspace.lints.clippy]` in `Cargo.toml`. Every pedantic lint is an error -- no exceptions in production code.

```toml
# From Cargo.toml -- do not relax these
[workspace.lints.clippy]
pedantic = "deny"
```

**Unsafe.** `unsafe_code = "deny"` at workspace level. Individual unsafe blocks use `#[allow(unsafe_code)]` with a documented reason.

**No `unwrap()`.** Use `?` for propagation or `.expect("reason")` for infallible operations. Bare `.unwrap()` is not allowed in production code. Test files are exempt (`#![allow(clippy::unwrap_used)]`).

**No `as any` / type erasure.** Use exhaustive error types and match arms. No lossy casts.

**250 LOC ceiling.** Files with more than 250 lines of pure logic need a `// SIZE_OK` comment explaining why they cannot be split, or the module must be refactored.

**Formatting.** `rustfmt.toml` sets `max_width = 120`, `tab_spaces = 4`, `use_field_init_shorthand = false`. Always run `cargo fmt` before committing.

**Patterns to follow.** The repository includes `AGENTS.md` which documents the decoder pipeline, module layout, and conventions. Read it before making structural changes.

## Pull Request Workflow

1. **Create a feature branch** from `main`. Use a descriptive name like `fix/rgb555-boundary` or `feat/new-encoder`.
2. **Make changes** following the code style above.
3. **Run verification** in this order:

    ```bash
    cargo check                     # Catches 90% of errors (~5s)
    cargo clippy --fix --allow-dirty  # Auto-fix mechanical lints
    cargo test --workspace          # Full suite (~40-60s)
    ```

4. **Test with SIMD** (if your changes touch decode paths):

    ```bash
    cargo test --features cache -p ithmb-core
    ```

5. **Check fuzz targets compile** (requires nightly):

    ```bash
    cargo +nightly fuzz build
    ```

6. **Update `AGENTS.md`** if you add new modules, change the pipeline flow, or introduce new patterns contributors should know about.
7. **Open a PR** with a clear description of what changed and why. Reference related issues if applicable.

**Before opening a PR**, also run:

```bash
cargo audit          # Advisory check
cargo fmt --check    # Formatting check
```

## Testing

**Unit tests** live in-module under `#[cfg(test)] mod tests` blocks. Integration tests go in `tests/` directories within each crate.

**Key test categories:**

| Category             | What it covers                                                              |
| -------------------- | --------------------------------------------------------------------------- |
| Golden vectors       | Reference `.ithmb` -> expected `.bin` byte-for-byte comparison              |
| Exhaustive roundtrip | All 65,536 RGB565 values, all 32,768 RGB555 values                          |
| SIMD tail            | 42 boundary widths (1..65) verifying SIMD matches scalar                    |
| Fuzz                 | 3 libfuzzer targets plus 10,000+ random byte mutations                      |
| Concurrency          | 11 stress scenarios (Barrier sync, cancellation, cache contention)          |
| Profile validation   | All 54 profiles decode without error                                        |
| Encoder              | Interlace fields, BT.601 color conversion, all format generators            |
| PhotoDB              | Roundtrip write, integrity, JPEG blob decode, device-specific format tables |
| Benchmarks           | Performance regression baseline in `benches/`                               |

**Running tests.**

```bash
cargo test --workspace                       # Everything
cargo test -p ithmb-core                     # Library only
cargo test -p ithmb-core --features cache    # With SIMD/cache features
cargo test --test integration                # Integration tests
```

Test files use `#![allow(clippy::pedantic, clippy::unwrap_used)]` -- they are exempt from production strictness.

## Fuzz Testing

Fuzz targets live in the `fuzz/` directory (excluded from workspace). They use `cargo-fuzz` with libfuzzer.

**Setup** (requires nightly Rust):

```bash
rustup toolchain install nightly
cargo +nightly install cargo-fuzz
```

**Targets:**

| Target                  | Purpose                                             |
| ----------------------- | --------------------------------------------------- |
| `fuzz_decode_ithmb`     | Fuzzes the raw pixel decoder against mutated inputs |
| `fuzz_open_ithmb`       | Fuzzes file open and format detection               |
| `fuzz_encode_roundtrip` | Fuzzes encode -> decode roundtrip consistency       |

**Running:**

```bash
# Verify targets compile
cargo +nightly fuzz build

# Run a target for 30 seconds
cargo +nightly fuzz run fuzz_decode_ithmb -- -max_total_time=30

# Run a specific corpus entry
cargo +nightly fuzz run fuzz_open_ithmb <corpus-file>
```

## Project Structure

```
Ithmb-Codec/
├── crates/
│   ├── ithmb-core/       # Core library (lib) -- published to crates.io
│   │   └── src/
│   │       ├── pipeline/      # Decode entry points (open_ithmb, decode_bytes)
│   │       ├── enc/           # 7 synthetic encoders
│   │       ├── photodb/       # PhotoDB/ArtworkDB chunk parser
│   │       ├── simd/          # SSE2/AVX2/NEON YUV conversion
│   │       ├── profile.rs     # Profile type + lookup
│   │       ├── profile_db.rs  # Static profile database (54 profiles)
│   │       ├── rgb565.rs      # RGB565/RGB555 decoder
│   │       ├── jpeg.rs        # JPEG-embedded decoder
│   │       └── error.rs       # DecodeError enum
│   ├── ithmb-cli/        # CLI binary (cargo install ithmb-cli)
│   ├── ithmb-gen/        # Synthetic sample generator binary
│   └── ithmb-wasm/       # WASM target (wasm-pack)
├── pymod/                # Python bindings (PyO3/maturin)
├── fuzz/                 # libfuzzer targets (3 targets)
├── docs/                 # Documentation
│   ├── adr/              # Architecture Decision Records
│   ├── guides/           # GUIDE.md, HARDWARE_GUIDE.md
│   ├── standards/        # STANDARDS.md, RUST_STANDARDS.md
│   └── benchmarks/       # BENCHMARKS.md
└── scripts/              # Utility scripts
```

## What NOT to Do

- Do not add new dependencies without checking if existing ones cover the need.
- Do not suppress type errors with `as _`, `#[allow]`, or `expect("unreachable")`.
- Do not edit `deny.toml` or CI workflows without understanding the full impact.
- Do not commit without running `cargo check` on the changed crate first.
- Do not add attribution lines (`Co-authored-by`, `Ultraworked with`) to commit messages.
