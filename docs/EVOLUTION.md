# Evolution: C# Reference → Rust Codec

This document describes the evolution of the `ithmb` codec from its original C# implementation to the current Rust workspace, the rationale behind each major decision, and the relationship between the two.

> **C# repository (archived):** [B67687/Ithmb-Codec-CSharp](https://github.com/B67687/Ithmb-Codec-CSharp)
> **Rust repository (current):** [B67687/Ithmb-Codec](https://github.com/B67687/Ithmb-Codec)

---

## The C# Version (Original)

### What it was

The C# codec was a **pure-managed Native AOT shared library** implementing the [ImageGlass](https://imageglass.org) v10 plugin ABI. It was a single-purpose binary that the ImageGlass image viewer could load as a plugin to decode `.ithmb` thumbnail files from iPods, iPhones, and iPod Touches.

**Key characteristics:**
- **Single target**: a `.dll`/`.so`/`.dylib` loaded by ImageGlass
- **Framework**: .NET 10 Native AOT (`TreatWarningsAsErrors`, `AnalysisLevel=latest-recommended`)
- **Quality**: 5 rounds of multi-agent systematic review, ~47 issues caught and fixed
- **Tests**: 594 passing, including 30 golden reference vectors from real iPod samples
- **Coverage**: exhaustive RGB565 roundtrip (65,536 values), statistical validation (entropy, histogram, decorrelation), SIMD constant validation, cancellation tests
- **Decoders**: 7 (RGB565, RGB555, UYVY, YCbCr 4:2:0, CL, CLCL, JPEG) + 53 profiles
- **Thoroughness**: 30 reference decode PNGs from Reuhno's synthetic iPod data
- **Limitations**: Windows-first (ImageGlass is Windows-only), no crates.io/PyPI distribution, no standalone CLI

### Strengths that carried forward

The C# code established every format decoding algorithm used in the Rust port. The BT.601 coefficients, the SIMD shuffle patterns, the YUV conversion math, the profile database, and the PhotoDB/ArtworkDB chunk parser were all proven in C# first. The Rust port is algorithmically a direct translation of the C# reference.

---

## Why Rust?

### Motivation

The C# codec was excellent at its job — decoding `.ithmb` files inside ImageGlass. But it was **trapped in that role**. To distribute the codec more broadly required capabilities the C# plugin couldn't provide:

| Capability | C# Plugin | Rust |
|---|---|---|
| **crates.io library** | Not possible (Native AOT DLL) | `cargo add ithmb-core` |
| **Standalone CLI** | Not possible | `cargo install ithmb-cli` |
| **Python bindings** | Not possible | `pip install ithmb-python` (PyO3) |
| **Fuzz testing** | No equivalent | `cargo fuzz` with libfuzzer |
| **Cross-platform** | Windows-primary | Linux/macOS/Windows native |
| **Ecosystem reach** | ImageGlass only | Any Rust/Python project |

The decision to port was **ecosystem reach**, not quality — the C# code was already excellent.

### What was lost in translation

The initial Rust port prioritized coverage over thoroughness. Comparing the two after the port:

| Area | C# (after 5 review rounds) | Rust (initial port) |
|---|---|---|
| Golden vectors | 30 reference PNGs from real samples | 14 tiny synthetic fixtures (2×2, 4×4) |
| RGB565 roundtrip | All 65,536 values | Partial |
| SIMD const validation | Dedicated test file | None |
| Statistical validation | Decorrelation, entropy, histogram | Partial |
| SIMD tail coverage | Widths 2,3,7,15,16,17 | None |
| Benchmark regression | baseline.csv + CI gate | None |
| ADR documentation | 3 decision records | None |
| Review documentation | 5 rounds documented | None |

This gap exists because the Rust code was written in fewer, faster cycles. The C# code had more review rounds applied to it. **The Rust port is not inherently lower quality — it simply hasn't had the same number of review passes yet.** This document is part of closing that gap.

---

## Major Architecture Decisions

### ADR-1: Cross-platform SIMD dispatch

**Decision:** Use cargo feature gates (`--features simd`) instead of runtime CPUID dispatch for SIMD paths.

**Rationale:** Runtime dispatch in Rust requires `#[target_feature]` + `#[cfg]` macros, leading to per-ISA abstraction layers for every decoder function. Feature-gating keeps the code cleaner and lets users opt out entirely on platforms where SIMD doesn't apply.

**Trade-off:** macOS ARM NEON had to be gated (known CI runner edge case). Documented in `STANDARDS.md`.

### ADR-2: C ABI plugin as a separate repository

**Decision:** Extract the ImageGlass C ABI plugin into its own repo ([Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin)) rather than keeping it in the workspace.

**Rationale:** The plugin has different dependencies (ImageGlass SDK), build profile (Native AOT cdylib), and release cycle. Co-locating it forced every workspace member to deal with ABI concerns. Splitting let the core codec evolve independently.

**Trade-off:** crates.io publishing became a manual step (publish `ithmb-core`, then update the plugin's dependency). The plugin's CI matrix (3 OS + clippy + cargo-deny + symbol export verification) runs independently.

### ADR-3: 54 built-in profiles + external profiles.json

**Decision:** Ship 54 profiles embedded in the binary (from iOpenPod, libgpod, and hardware validation), with an optional external `profiles.json` for runtime overrides.

**Rationale:** The C# version proved the profile database is stable across iPod generations. Embedding it avoids runtime file lookups while letting advanced users override without recompilation. External profiles are parsed by a custom AOT-safe JSON parser (no reflection).

**Trade-off:** Profile discovery relies on the 4-byte F-prefix in `.ithmb` files. If Apple introduces a new format variant that reuses an existing prefix, the fallback-encoding chain handles it — but only if the profile is known.

---

## The Migration Path

### Phase 1: Rust port (initial)

The Rust codebase started as a direct port of the C# decoder algorithms and profile database. The initial commit was structured as a Rust workspace with the same decode pipeline, same profiles, same test structure — just translated to Rust idioms.

### Phase 2: Workspace expansion

The Rust workspace grew to include:
- **`ithmb-core`**: core library (published to crates.io)
- **`ithmb-cli`**: standalone CLI with `--open`, `--info`, `--list-profiles`, `--frame`
- **`ithmb-python`**: PyO3 bindings (abi3-py312)
- **`ithmb-gen`**: synthetic sample generator

### Phase 3: C ABI split

The `cabi` crate was extracted into its own repository. This:
- Removed the ImageGlass SDK dependency from the workspace
- Let the plugin version independently
- Allowed the core codec to evolve without ABI constraints

### Phase 4: Quality parity (current)

Closing the quality gap between the C# reference and Rust port through:
- Golden reference vector tests (30 Reuhno samples)
- Exhaustive roundtrip coverage (65,536 RGB565 values)
- SIMD constant validation
- Statistical test completeness
- Zero-alloc hot paths
- Architecture decision records (this document)
- Review documentation

---

## Version History

| Version | Date | Notes |
|---------|------|-------|
| C# v1.0–v1.6 | 2025 | Original C# development, 5 review rounds |
| C# v1.9.0 | 2026-06 | Final C# release, repo archived |
| Rust v1.9.0 | 2026-06 | Initial crates.io publish of ithmb-core |
| Rust v1.9.1 | 2026-06 | Crate-level README added |

---

## Known Gaps

The following areas are deferred (not blocking functionality):

| Gap | Reason | Status |
|-----|--------|--------|
| **NEON CI runner** | No reliable free ARM64 CI. macOS runners have known edge cases (STANDARDS.md). NEON code exists but untested in CI. | Deferred |
| **Cache concurrency stress** | LRU cache is feature-gated and simple — low user impact. | Done (2026-07) |
| **Real-device validation** | Golden vectors are from synthetic data. Savi (iOpenPod) validated against real hardware. | Deferred (needs hardware donation) |
## Acknowledgments

The Rust codec stands on the shoulders of the C# version, which was itself built on the work of the iPod reverse-engineering community:

- **iOpenPod (Savi)** — primary format profile reference, 50+ empirically validated entries
- **libgpod** — PhotoDB/ArtworkDB chunk parser foundations
- **Keith's iPod Photo Reader (kebwi)** — original reverse engineering (2005), 13 decode methods, multi-frame confirmation
- **clickwheel (dstaley)** — C# ArtworkDB read/write, format ID tables
- **pyithmb (wrinklykong)** — Python YUV reference decoder
- **Reuhno** — first public F-prefix .ithmb test vectors + 30 reference PNGs (CC0)
- **mgminformatique** — iPod photo recovery tool, independent format analysis
- **Frulko** — iPod sync tool and analysis
- **ImageGlass** — plugin ABI that motivated the C# implementation

The C# reference codebase ([B67687/Ithmb-Codec-CSharp](https://github.com/B67687/Ithmb-Codec-CSharp)) remains the authoritative source for algorithm verification. Its thoroughness — 594 tests, 30 golden vectors, documented review rounds — set the quality standard that the Rust port continues to pursue.
