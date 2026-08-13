# AGENTS.md — AI Agent Guide for Ithmb-Codec

This file tells AI coding agents (Claude Code, Copilot, Cursor, Codex, OpenCode) how to work with this repository effectively. Read this first before editing any code.

## Repository Purpose

Pure Rust codec for Apple `.ithmb` thumbnail-cache files (iPod/iPhone photo thumbnails). Decodes 8 raw pixel formats, encodes 7, parses PhotoDB/ArtworkDB containers. Published on crates.io as `ithmb-core`. Also the source of the WebAssembly decoder used by the sibling web repo `B67687/Ithmb-Codec-Web`.

## Repository Layout

```
Ithmb-Codec/
├── crates/
│   ├── ithmb-core/       # Core library (lib) — published to crates.io
│   │   └── src/
│   │       ├── pipeline/      # Decode entry points (open_ithmb, decode_ithmb, decode_with_profile)
│   │       ├── jpeg.rs        # JPEG-embedded decoder — read_info() dimension pre-check (CWE-400)
│   │       ├── profile.rs     # Profile type + lookup
│   │       ├── profile_db.rs  # Static profile database (53 profiles)
│   │       ├── photodb/       # PhotoDB/ArtworkDB chunk parser
│   │       ├── enc/           # 7 synthetic encoders
│   │       ├── simd/          # SSE2/AVX2/NEON YUV conversion (only unsafe in the codebase)
│   │       ├── c_api.rs       # C ABI (feature "c") — undersized-buffer guard before copy
│   │       └── error.rs       # DecodeError enum
│   ├── ithmb-cli/        # CLI binary (cargo install ithmb-cli)
│   ├── ithmb-gen/        # Synthetic sample generator binary
│   ├── ithmb-python/     # Python bindings (PyO3/maturin)
│   └── ithmb-wasm/       # WASM target (wasm-pack) → consumed by Ithmb-Codec-Web
├── fuzz/                 # libfuzzer targets (3 targets; pinned nightly via fuzz/rust-toolchain.toml)
├── scripts/
│   ├── local-ci.sh       # Full Linux-runnable CI set (fmt, clippy, tests, builds, deny, audit, C-API)
│   ├── check-ci-pins.sh  # Fails on unpinned Actions/installs (run in pr-checks)
│   ├── check-file-sizes.sh # 250-LOC ceiling helper (default: crates/)
│   ├── bench-prep.sh / run-bench-perf.sh / check-baseline.py
│   └── git-commit-dated.sh
├── tools/check-benchmark-regression.sh  # CI bench gate (divan vs .github/baseline.json)
└── docs/                 # adr/, guides/, standards/, benchmarks/, RELEASING.md
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
- **Unsafe**: `unsafe_code = "deny"` at workspace level; individual unsafe blocks use `#[allow(unsafe_code)]` (SIMD + c_api only)
- **No `unwrap()`**: Use `?` or `.expect("reason")` — never bare `.unwrap()`
- **250 LOC ceiling**: Files > 250 lines of pure logic need a `// SIZE_OK` comment or splitting (`scripts/check-file-sizes.sh` verifies)
- **Edition**: Rust 2024, MSRV 1.88

## Test Patterns

Run in this order:

```bash
cargo check                     # Catches 90% of errors (~5s)
cargo clippy --fix --allow-dirty  # Auto-fix mechanical lints
cargo test --workspace          # Full suite (~40-60s)
```

Key test categories (see STATS.md for live counts):

- **Golden vectors**: Reference `.ithmb` → expected `.bin` byte-for-byte comparison
- **Exhaustive roundtrip**: All 65,536 RGB565 values, all 32,768 RGB555 values
- **SIMD tail**: 42 boundary widths (1..65) verifying SIMD matches scalar
- **Fuzz**: 3 libfuzzer targets (cargo-fuzz, pinned 0.13.2) + 10,000+ random byte mutations
- **Concurrency**: 11 stress scenarios (Barrier sync, cancellation, cache contention)
- **C API**: `cargo test -p ithmb-core --features c --test c_api_test` — includes the undersized-buffer guard test
- **Profile validation**: All 53 profiles decode without error

## CI Policy (3 layers)

Checks are triaged by **fast-and-runnable vs slow/platform-specific**:

| Layer | What runs | When | Speed |
|---|---|---|---|
| **1. Pre-commit hook** (`.githooks/pre-commit`) | fmt --check always; clippy + `cargo test --workspace --tests` when `.rs` files staged | every commit, auto | ~10-60s |
| **2. `./scripts/local-ci.sh`** | fmt, clippy, tests, builds (workspace/logging/wasm/C-API), cargo-deny, cargo-audit; `--fuzz` opt-in | before pushing, on demand | ~30s+ |
| **3. GitHub CI** | `pr-checks.yml` (fast: fmt/clippy/typos/links/deny/audit/doc/CI-pins/secrets) on PR+push; `ci-full.yml` (3-OS matrix, fuzz, benchmark, wasm, C-API) on main push; `miri.yml` weekly; `release.yml` tag-gated | every push, auto | 2-6min |

**Activate the pre-commit hook once per clone:** `git config core.hooksPath .githooks` (full install in docs/SETUP.md).

Rules:
- Run `./scripts/local-ci.sh` before pushing. The pre-commit hook is the floor; local-ci.sh is the full Linux-runnable set.
- Fuzz is slow — opt in via `./scripts/local-ci.sh --fuzz`.
- miri, benchmark regression, and the macOS/Windows legs stay on GitHub.
- **Public CI is the gate.** The dev repo (`origin` = `Ithmb-Codec-Dev`, PRIVATE) has its Actions blocked by the account's paid-minute billing state; the PUBLIC repo (`public` = `Ithmb-Codec`) runs the same workflows free. A red dev CI is cosmetic — check the public repo's runs.
- All Actions are SHA-pinned; `scripts/check-ci-pins.sh` (wired into pr-checks) fails on any future unpinned ref or install.
- CI commit-message types allowed: `feat, fix, docs, refactor, test, chore, cleanup, perf` (not `ci`).

## Dev / Public Dual-Repo Workflow (CRITICAL)

**Canonical standard: `docs/standards/RELEASE_WORKFLOW.md`** — this section is a summary; the standard is the source of truth.

```
origin  → https://github.com/B67687/Ithmb-Codec-Dev   (PRIVATE — editing repo, CI billing-blocked)
public  → https://github.com/B67687/Ithmb-Codec       (PUBLIC — shipped repo, FREE CI)
```

- All work lands on dev `main` → push `origin/main`.
- Public ships are built on a branch from `public/main`: cherry-pick the net commits (e.g. collapse a split/revert/re-apply trio into one clean commit), verify `git diff --quiet <dev-head> <public-branch>` is empty, then `git push public <branch>:main`.
- Version bumps + CHANGELOG entries go on dev and ride the ship.
- Release tags live on the PUBLIC repo (`vX.Y.Z`); see `docs/RELEASING.md`.

## WASM Regeneration → Ithmb-Codec-Web

The browser decoder consumes this crate. Shipping a core change to the web:

```bash
cd crates/ithmb-wasm
cargo check -p ithmb-wasm --target wasm32-unknown-unknown
wasm-pack build --target web --release
cp pkg/ithmb_wasm_bg.wasm ../../../Ithmb-Codec-Web/ithmb-decoder/ithmb_wasm_bg.wasm
```

**Copy ONLY `ithmb_wasm_bg.wasm`** — the web repo's `ithmb_wasm.js` loader and `ithmb_wasm_bg.js` glue are hand-adapted and must not be replaced. A rebuild that adds a wasm import the glue doesn't define breaks the decoder at runtime; the web repo's `scripts/check-wasm-drift.sh` detects this. Do NOT add `console_error_panic_hook` (its `js_sys::Error` glue import breaks the loader) — the decoder is panic-free by design.

## Security Posture

- **CWE-400 JPEG cap** (`jpeg.rs`): `read_info()` pre-check rejects frames over a 256 MiB w·h·3 budget before decode — `set_max_decoding_buffer_size` alone does NOT cover the progressive coefficient buffer, and `set_max_dimensions` doesn't exist in jpeg-decoder 0.3.2. Regression test: 193-byte SOF2-65535×65535 fixture.
- **CWE-787 C-API guard** (`c_api.rs`): `ithmb_decode` rejects undersized caller buffers (area-based, so EXIF rotation doesn't false-positive). Guard test in `test_ithmb.c`.
- **No attacker-reachable panics** — zero `unwrap()`/`panic!` outside tests; unsafe confined to SIMD on validated slices.
- `SECURITY.md` + gitleaks scan in pr-checks; `cargo-audit` + `cargo-deny` gated in pr-checks. Dependency upgrades are a LOCAL check (`cargo-outdated` in local-ci.sh) — commit upgrades on dev and ship them dev-first, so the public tree always mirrors dev.
- Secrets history scan: clean; no `${{ secrets.* }}` values ever committed.

## Building

```bash
cargo build --workspace           # All crates
cargo build --release             # SIMD always compiled for x86_64 and aarch64
cargo build -p ithmb-core --features c   # C ABI cdylib
wasm-pack build crates/ithmb-wasm # WASM target (requires wasm-pack)
maturin develop --release -m crates/ithmb-python/Cargo.toml  # Python bindings
```

## Release Process

Follow `docs/RELEASING.md`. In short: bump `Cargo.toml` (workspace) + CHANGELOG → dev commit → local-ci → ship to public → create `vX.Y.Z` tag on the PUBLIC repo (tag-gates `release.yml`). **Do not release without a tag** — early 1.9.x versions shipped untagged once; tags are the traceability.

## Key Decisions

- **SIMD compiled unconditionally** — SSE2/AVX2 for x64, NEON for ARM64 (runtime dispatch)
- **C ABI plugin in separate repo** — [ImageGlass-Ithmb-Plugin](https://github.com/B67687/ImageGlass-Ithmb-Plugin)
- **53 built-in profiles** — embedded in binary, optionally overridable via external `profiles.json`
- **File size guard**: 8 MB max (ADR-0005), covers all known real-world files with 10× margin
- **`cache` / `metrics` are feature-gated**; `c` is a feature too (cdylib only when enabled)

## What NOT to Do

- Do NOT add new dependencies without checking if existing ones cover the need
- Do NOT suppress type errors with `as _`, `#[allow]`, or `expect("unreachable")`
- Do NOT edit `deny.toml` or CI workflows without understanding the full impact
- Do NOT commit without running `cargo check` on the changed crate first
- Do NOT add attribution lines (`Co-authored-by`, `Ultraworked with`) to commit messages
- Do NOT push the dev repo's `main` to `public` without the squash/cherry-pick ritual above
