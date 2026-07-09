# Perfect Development Plan: Ithmb Codec

**Recursive specification-driven framework**: ambition → full spec → macro plan → micro plans → zero qualms.

**Design principle**: 80% of the structure is planned and researched upfront (file tree, README template, badge SVGs, commit strategy, CI stages, attribution policy, every file's purpose). 20% is discoverable data (profile counts, test counts, device quirks, benchmark numbers) — filled in during research, with placeholders pre-planned.

**Leverage principle**: Never write what a tool already does. Every check, analysis, or transformation should use an existing tool before considering custom code. Cargo-deny for licenses (not manual audits), cargo-mutants for mutation testing, nextest for test execution, divan for benchmarks, git-cliff for changelogs, **typos (pin to MSRV-compatible version)** for spell checking, semver-checks for API breakage. Only write custom tooling when no existing tool serves the specific need.

---

## Layer 0: Seed Ambition

```
"I want to open .ithmb thumbnail files in ImageGlass on Windows."
```

This is the seed. The error is stopping here and writing code.

---

## Layer 1: Full Interrogation

### The Mistake That Changed Everything

The single biggest deviation from the ideal path: **C# first**.

The entry point was ImageGlass — a C# app. Natural assumption: "C# plugins, C# codec." But ImageGlass's plugin ABI (`ig_plugin_get_api`) is a **C ABI** — any language exposing C-compatible FFI can serve it.

**Researched decision**: Rust + C ABI from day 1.
- Source: ImageGlass plugin docs (`ig_plugin_get_api` is `__declspec(dllexport)` — pure C ABI)
- Source: Rust `cdylib` crate type produces a `.dll`/`.so`/`.dylib` with C-compatible ABI
- Source: `ithmb-core-cabi` crate is ~100 lines of `extern "C"` wrapper functions
- Verdict: Rust serves ImageGlass plugin + CLI + cross-platform + Python bindings from one codebase. C# would require Mono/.NET for CLI and Linux, eliminating the "no runtime" advantage.

### Full Interrogation

| Seed question | Discovered requirement | Research source |
|--------------|----------------------|----------------|
| "What .ithmb files?" | 6 prefix variants, 7 pixel formats, 54+ profiles | Keith's iPod Photo Reader, iOpenPod, libgpod format surveys |
| "What language?" | Rust + C ABI (not C#) | ImageGlass `ig_plugin_get_api` C ABI requirement |
| "What about CLI?" | Cross-platform decode/info/extract tool | Needed for validation without ImageGlass |
| "What about Python?" | PyO3 bindings for research workflows | Community requested, enables scripting |
| "What about performance?" | SIMD SSE2 + NEON for hot decoders | Benchmarking showed RGB565/UYVY are decode bottlenecks |
| "What about corrupt files?" | Fuzz testing, error tolerance, crash safety | CWE-674 (uncontrolled recursion), CWE-125 (OOB read) mitigations |
| "What about PhotoDB?" | ArtworkDB parser/writer for iPod cache access | Half of all .ithmb files are in PhotoDB containers |
| "What about correctness?" | Exhaustive roundtrip + cross-reference with libgpod | Reference implementations differ — must validate against multiple sources |
| "What about documentation?" | AGENTS.md for AI, FORMAT.md for spec, ARCHITECTURE.md for flow | OMO best practice: agents need structured onboarding |
| "What about attribution?" | AI credits, reference project credits, license | MIT license, acknowledge Keith/iOpenPod/libgpod/gnupod |
| "What about presentation?" | README with centered logo, badges, SVG diagrams | GitHub standards: badge grouping, centered hero, visual architecture |
| "What about git history?" | Conventional commits, squash policy, thematic grouping | 500+ raw commits → messy filter-repo. Plan commit structure upfront. |
| "What about file organization?" | Single workspace, logical crate split, one file per concern | 6→15→structured files reorganization cost time. Design final tree first. |

### Full Spec

```
ITHMB CODEC — Full Specification

What it MUST do:
- Decode all known .ithmb variants (T/F/P/B/M/S prefix) to BGRA8 pixels
- Encode all raw pixel formats (7 formats, synthetic + roundtrip)
- Serve as ImageGlass v10 plugin via C ABI cdylib
- Serve as cross-platform CLI tool (Linux/macOS/Windows)
- Serve as C library for any C-compatible host
- Serve as Python library via PyO3 bindings
- Pass exhaustive roundtrip tests for every pixel format (65,536 values for RGB565)
- Survive fuzz testing (5 min per format, 0 crashes)
- Match reference implementations on real device data
- Profile all known format IDs across 18 iPod/iPhone generations
- Read and write PhotoDB/ArtworkDB binary containers (9 chunk types)
- SIMD acceleration on x64 (SSE2) and ARM64 (NEON)
- Zero unsafe UB (Miri Level 3 clean for all unsafe blocks)
- Complete documentation suite (AGENTS.md, FORMAT.md, ARCHITECTURE.md, ADRs, CHANGELOG)
- Single binary CLI with zero runtime dependencies
- Archive-quality: single-commit repos for reference, locked deps, green CI

What it must present (researched upfront):
- README: centered logo at top, language/platform badges row, SVG screenshot, feature bullets, AI credits section
- 2 SVG diagrams: decode pipeline (data flow) + architecture (code structure)
- Badges: GitHub stars, license, language, platform (Win/Mac/Linux), tests passing, code coverage
- Badge generation: scripted (not hand-edited SVGs) — source from CI output

What it MUST NOT do:
- NOT be a C# project (C ABI + Rust serves all targets)
- NOT decode hypothetical/unconfirmed format variants
- NOT have premature CI gates (minimal CI until Phase 6)
- NOT be distributed as separate repos (single workspace, legacy branches/tags)
- NOT have ambiguous commit history (conventional commits, enforced by hook)

"DONE" criteria:
- All 7 decoders pass exhaustive roundtrip + 5-min fuzz (3+ targets) — 0 crashes [B1, B2]
- All unsafe blocks pass Miri Level 3 (strict provenance + symbolic alignment) — zero UB [A2]
- Real .ithmb files decode correctly (checked visually + SHA256 against reference)
- &AtomicBool cancellation works at macroblock boundaries — 11+ concurrency stress tests pass
- ImageGlass loads plugin and renders thumbnails
- CLI tool published (crates.io or GitHub Releases)
- WASM target builds (wasm-pack, no native SIMD)
- README complete with logos, badges, SVG diagrams, AI credits
- AGENTS.md + ARCHITECTURE.md + FORMAT.md + STANDARDS.md complete
- All reference implementations cross-referenced in ECOSYSTEM.md with original research contributions documented
- Single-commit squashed repos (Rust main, legacy/csharp tag)
- CHANGELOG.md with every release (updated per-commit, not retroactively) [D3]
- ADR-0001 through ADR-000N documenting every key decision
- deny.toml green (cargo-deny, cargo audit) [E2]
- All public types implement Debug + Send + Sync [A1: C-DEBUG, C-SEND-SYNC]
- Coverage >=90% branch coverage across decoder core — measured via llvm-cov/grcov per-format in Phase 4, gated in Phase 6 CI merge, final verification in Phase 8 [B2]
- Mutation score >=80% across decoder core — runs nightly (too expensive for per-commit), gated pre-release in Phase 8 [B3]
- CWE mitigations verified: 125, 674, 770, 190, 415, 476 [C1]
- Conventional Commits enforced by hook — history is clean by construction [D1]
- Semver policy documented: 0.y.z until public API stabilizes [D2]
- MIT license [F2]
```

---

## Layer 2: Macro Plan

### Phase 0: Deep Discovery & Research (Day 1-5)

**WHY**: Every wrong decision traces back to incomplete research. This phase eliminates goalpost shifts by researching EVERYTHING before any code.

**WHAT**: Three parallel research tracks — Format, Tooling, Presentation — producing research reports with concrete recommendations.

**ACCEPTANCE**: Every decision in Phases 1-8 has a citation. Presentation template chosen. Attribution policy written.
**STANDARDS**: Rust API Guidelines C-METADATA; GitHub Community Standards; Conventional Commits 1.0.0; Keep a Changelog 1.1.0
**ANTI-PATTERN**: Researching only the format and skipping presentation/tooling standards (leads to 3 README rewrites, badge regeneration, SVG recreation).
#### Discovery Protocol (for first-time builds)

When you don't yet know what sources exist, use this methodology to FIND them:

1. **GitHub search queries**: Run these searches in order until saturation:
   - `"ithmb"` — exact format name
   - `"iPod thumbnail"` — broader ecosystem
   - `"iPod Photo" decoder` — related tooling
   - `"PhotoDB" iPod` — container format
   - `"ArtworkDB"` — related database format
   - `topic:ithumb` — GitHub topic tags
   - `"thumbnail_format" AND iPod` — format reverse engineering
   - `libgpod thumbnail` — device library

2. **For each repo found**: skim README, extract: format description, decoder algorithm, profile tables, device format IDs, test vectors
3. **Cross-reference discovery**: if repo A mentions repo B, RESEARCH B TOO. Do not stop until you reach zero new sources from any discovered repo.
4. **Dead end check**: if no GitHub results, search web for "ithmb", "iPod thumbnail cache reverse engineering"

Confidence gate: >= 3 independent sources confirming the same format variant details. If sources disagree, note the discrepancy and investigate with hex dumps.

*After discovery is complete, the following sources were identified. Use these as the authoritative references:*

#### Track A: Format RE Research (the obvious one)

| Source | What to extract | Method |
|--------|----------------|--------|
| Keith's iPod Photo Reader | Raw decoder algorithms, F-prefix handling | Read source, note hex patterns |
| iOpenPod | Full toolkit, PhotoDB parser, profile DB | Read Python source, extract profile table |
| libgpod | 42 device profiles, device capabilities | Read C source, cross-ref format IDs |
| ithmbrdr | Multi-frame display | Read source, note frame concatenation |
| pyithmb / andrewmalta/ithmb | CLCL/CL nibble algorithms | Read source, verify against hex dumps |
| wrinklykong/pyithmb | CLCL×17 scaling bug | Read source, note discrepancy |
| gnupod | Device format tables | Read XML/Perl, extract tables |
| OrgZ clickwheel | SysInfoExtended, format 1062 discovery | Read source, extract format table |
| Steee29/ithmb_converter | Real iPhone 2G format 3009 dimensions | Read source, note portrait vs landscape |

**Deliverable**: `references/format-research.md` with consolidated profile table, hex dump analysis, algorithm notes, and discrepancies between sources.

**Profile table format** (this is a STRUCTURAL decision — the table format doesn't change, only the rows fill in):
```
| Prefix | Format ID | Width | Height | Encoding | FrameBytes | Devices | Source |
|--------|-----------|-------|--------|----------|------------|---------|--------|
| F      | 1001      | 56    | 56     | RGB565   | 6272       | Classic | libgpod + iOpenPod |
```

#### Track B: Tooling & Presentation Research (the one we missed)

| Question | Research source | Structural decision |
|----------|---------------|-------------------|
| What README format? | GitHub community standards, awesome-readme | Centered hero, badge groups, screenshots, toc |
| What badge style? | Shields.io standard, GitHub repo badges | `![Label](https://img.shields.io/badge/...)` generated by script |
| Badge generation? | Scripted vs hand-edited | Python script (`tools/generate-badges.py`) driven by CI |
| SVG diagram tools? | draw.io, mermaid, excalidraw, d2 | d2lang (text-to-diagram, version-controllable, deterministic) |
| Commit convention? | Conventional Commits, OMO git-master style | `type(scope): description` enforced by commit-msg hook |
| Changelog format? | Keep a Changelog standard | `## [version] — date` sections, unreleased at top |
| License choice? | MIT vs Apache vs GPL | MIT (permissive, standard for OSS codecs) |
| AI disclosure format? | OMO convention, GitHub community | "Built with" section in README, AI declaration in CHANGELOG |
| Attribution for reference projects? | Standard academic citation + README section | ACKNOWLEDGMENTS.md with project name, URL, contribution |
| AGENTS.md format? | OMO init-deep skill standards | Root AGENTS.md with structure, where-to-look, conventions, commands |
| Architecture diagram content? | What flows need visual explanation | Pipeline (data flow) + Architecture (code structure) |
| ImageGlass plugin packaging? | ImageGlass igplugin.json format | Read ImageGlass docs, template the JSON upfront |

**Deliverable**: `references/tooling-standards.md` with exact templates for every file format.

**Researched decisions documented**:
```
RESEARCHED: README format
Source: awesome-readme checklist, GitHub community standards
Decision: Centered hero section with logo, then badges row, then screenshot/feature matrix, then body
Why: Matches top-starred Rust projects (ripgrep, fd, bat, tokei) — users expect this layout
Template: See references/tooling-standards.md

RESEARCHED: SVG tooling
Source: d2lang vs mermaid vs excalidraw comparison
Decision: d2lang (text-to-SVG, deterministic output, version-controllable, single binary)
Why: Mermaid doesn't support exact positioning. draw.io files are XML blobs. d2 gives reproducible SVGs from plaintext.
Template: See references/pipeline.d2 and references/architecture.d2
```

**ANTI-PATTERN TO KILL**: Designing README layout by hand in a text editor. Instead, design it in research phase as a mockup, then fill in data.

#### Track C: Architecture Research

| Question | Research source | Decision |
|----------|---------------|----------|
| Rust workspace layout? | Cargo workspace best practices, 3-repo pain in actual project | Single workspace, 6 crates + pymod + fuzz |
| CLI framework? | clap vs structopt (clap is the successor) | clap derive API |
| C ABI export? | Rust `extern "C"` cdylib patterns | Feature of ithmb-core (`crate-type = ["lib", "cdylib"]`) not a separate crate |
| WASM target? | wasm-pack compatibility | `ithmb-wasm` crate for browser-based .ithmb viewers |
| Python bindings? | PyO3 vs cffi vs rust-cpython | PyO3 via maturin, separate `pymod/` directory |
| SIMD strategy? | SSE2 vs AVX2 vs auto-vectorization | SSE2/AVX2 for x64, NEON for ARM64 (macOS ARM uses scalar fallback), feature-gated |
| Fuzz framework? | libfuzzer vs proptest vs cargo-fuzz | cargo-fuzz (libfuzzer), 3+ targets in `fuzz/` directory |
| Unsafe verification? | Miri vs loom vs manual audit | Miri Level 3 (strict provenance + symbolic alignment) on all `unsafe` blocks |
| Cancellation support? | `&AtomicBool` polling in decoders | Required — prevents UI hangs in ImageGlass, checked at macroblock boundaries |
| Concurrency testing? | loom vs thread scope vs stress | `thread::scope` + Barrier for synchronized multi-threaded cancellation and cache contention |
| Error handling? | thiserror vs anyhow | thiserror for library, anyhow for CLI |
| Test framework? | cargo test vs nextest | cargo nextest (parallel, per-test timeout, faster) |

---

### Phase 1: Structural Bootstrap (Day 5-8)

**WHY**: 80% of the structure is designed upfront. No reorganizations later.

**WHAT**: Complete file tree, README template with badges, SVG diagram stubs, commit hooks, CI skeleton, AGENTS.md.

**STANDARDS**: Rust API Guidelines C-CRATE-DOC, C-EXAMPLE, C-METADATA, C-RELNOTES, C-LINK, C-HIDDEN; Conventional Commits 1.0.0; Semver 2.0.0; Keep a Changelog 1.1.0; MIT license
**ACCEPTANCE**: All structural files exist. `cargo check` passes. CI runs tests. Commit hooks enforce conventional commits.

**ANTI-PATTERN**: Starting with implementation and designing structure later.

#### 1a — File Tree (designed, not evolved)

```
ithmb/                              # Root (single workspace)
├── Cargo.toml                      # Workspace manifest
├── AGENTS.md                       # AI onboarding — written Phase 1, never refactored
├── ARCHITECTURE.md                 # Code structure + data flow
├── CHANGELOG.md                    # Keep a Changelog, starting from [Unreleased]
├── CONTRIBUTING.md                 # Optional — add only if external contributions appear
├── CREDITS.md                      # AI disclosure + tooling credits
├── ECOSYSTEM.md                    # Original research: 15+ discrepancies, 18 new format IDs, BGR15, 8MB guard
├── FORMAT.md                       # Complete format specification
├── STANDARDS.md                    # Coding standards, SIMD dispatch rules, safety
├── GLOSSARY.md                     # .ithmb terms explained
├── GUIDE.md                        # How to extract .ithmb from real iPod
├── LICENSE                         # MIT
├── README.md                       # Centered logo, badges, screenshot, body
├── references/                     # Research artifacts (RE notes, source comparisons)
│   ├── format-research.md
│   └── tooling-standards.md
├── docs/
│   ├── adr/                        # Architecture Decision Records
│   │   ├── 0001-use-rust-cabi.md
│   │   ├── 0002-simd-dispatch-strategy.md
│   │   ├── 0003-profile-discovery.md
│   │   ├── 0004-audit-protocol.md
│   │   └── 0005-file-size-guard.md
│   ├── badges/                     # Script-generated badge SVGs
│   │   ├── tests.svg
│   │   └── platform.svg
│   └── diagrams/                   # d2lang source files
│       ├── architecture.d2
│       └── pipeline.d2
├── crates/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── decoder.rs          # Core decode dispatch (cancellation, profile routing)
│   │   │   ├── error.rs            # DecodeError enum with Canceled variant
│   │   │   ├── decoders/           # One file per pixel format
│   │   │   │   ├── rgb565.rs
│   │   │   │   ├── rgb555.rs
│   │   │   │   ├── reordered_rgb555.rs
│   │   │   │   ├── uyvy.rs
│   │   │   │   ├── ycbcr420.rs
│   │   │   │   ├── clcl.rs
│   │   │   │   ├── cl.rs
│   │   │   │   ├── jpeg.rs
│   │   │   │   └── mod.rs
│   │   │   ├── encoders/           # One file per pixel format
│   │   │   ├── profile/            # Profile DB, device tables
│   │   │   │   ├── mod.rs
│   │   │   │   ├── profiles.rs
│   │   │   │   └── devices.rs
│   │   │   ├── photodb/            # PhotoDB parser/writer
│   │   │   │   ├── mod.rs
│   │   │   │   ├── parser.rs
│   │   │   │   ├── writer.rs
│   │   │   │   └── types.rs
│   │   │   └── simd/               # SSE2/AVX2/NEON dispatch (feature-gated)
│   │   │       ├── mod.rs
│   │   │       ├── yuv_sse2.rs
│   │   │       ├── yuv_avx2.rs
│   │   │       └── yuv_neon.rs
│   │   ├── benches/                # Divan benchmarks
│   │   │   ├── decoders.rs
│   │   │   ├── encoders.rs
│   │   │   ├── pipeline.rs
│   │   │   └── simd_compare.rs
│   │   └── tests/                  # Integration tests
│   │       ├── roundtrip.rs
│   │       ├── concurrency.rs      # 11+ concurrency stress tests
│   │       ├── cancellation.rs     # Cancel via &AtomicBool at macroblock boundaries
│   │       └── golden_vectors.rs   # SHA256-verified reference decode tests
│   ├── ithmb-cli/                  # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── ithmb-wasm/                 # WASM target (wasm-pack)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── ithmb-gen/                  # Synthetic test vector generator
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── pymod/                      # Python bindings (PyO3)
│       ├── Cargo.toml
│       ├── src/lib.rs
│       └── python/
├── fuzz/                            # libfuzzer targets
│   ├── Cargo.toml
│   ├── fuzz_targets/
│   │   ├── fuzz_decode_ithmb.rs
│   │   ├── fuzz_open_ithmb.rs
│   │   └── fuzz_photodb.rs
│   └── build.rs
├── samples/                        # Test vectors (synthetic + real)
│   ├── synthetic/                  # Generated by ithmb-gen
│   └── reuhno-reference/           # Real validated test vectors with SHA256 manifest
├── tools/                          # Infrastructure scripts
│   ├── generate-badges.py          # Badge SVG generation
│   ├── check-readme-stats.sh       # Verify README numbers match code
│   ├── check-benchmark-regression.sh
│   ├── generate-benchmark-report.py
│   └── install-ithmb-magick.sh     # ImageMagick delegate
├── deny.toml                       # cargo-deny configuration
├── rust-toolchain.toml              # MSRV + components (clippy, rustfmt, miri)
└── .github/
    └── workflows/
        ├── ci.yml                  # Build matrix (OS × features) + clippy + link check
        ├── miri.yml                # Nightly Miri with strict provenance
        ├── coverage.yml            # Code coverage (added Phase 6)
        ├── benchmark.yml           # Benchmark regression (added Phase 7)
        └── release.yml             # Publish (added Phase 8)
```
- `cargo doc --no-deps -D warnings` added to CI to catch broken links and missing docs
- `cargo msrv verify` added to CI (via cargo-msrv) for MSRV enforcement

**Rationale for every structural decision**:
- Single workspace, not 3 repos: cross-crate refactoring, shared test vectors, one CI. `legacy/csharp` branch suffices.
- C ABI is a feature of ithmb-core, not a separate crate: simpler build, one version, no cross-crate ABI drift.
- `ithmb-wasm` for browser targets: wasm-pack produces .wasm from a separate crate, native SIMD excluded.
- `fuzz/` at workspace root: 3 libfuzzer targets independent of test suite, OSS-Fuzz compatible.
- `deny.toml` + `rust-toolchain.toml`: supply chain audit + MSRV enforcement, configured in Phase 1 not Phase 8. deny.toml initial allowlist: [MIT, Apache-2.0, BSD-3-Clause, ISC].
- `miri.yml` CI: Miri nightly with strict provenance on every unsafe block. Blocking gate for SIMD/FFI.
- One decoder per file: the C# version had a 1200-LOC monolith. Single-responsibility per file prevents this.
- `simd/` subdirectory: SSE2/AVX2/NEON dispatch, feature-gated. macOS ARM uses scalar fallback per STANDARDS.md.
- Cancellation in every decoder: `&AtomicBool` checked at macroblock boundaries. Prevents ImageGlass UI hangs.
- `concurrency.rs` test suite: 11+ stress scenarios for cache contention, multi-frame decode, profile publication.
- `references/` directory: research artifacts live with the code, written during Phase 0, referenced during Phase 2-4.
- `tools/` scripts: badge generation is a script (badges change), stats checker ensures README counts match codebase.
- ECOSYSTEM.md as original research deliverable: not just reference list but documentation of 15+ discrepancies found, 18 new format IDs, BGR15 discovery, etc.

#### 1b — README Template

> Centered hero (logo + title + tagline) → badge row (platform, tests, license, stars) → pipeline SVG → feature bullets → quick start → usage → architecture → performance → dev → ecosystem → credits → license.
> Full template generated at Phase 1, data filled during Phases 2-7. See actual README.md for the authoritative version.
#### 1c — SVG Diagram Specs

**Pipeline diagram** (`docs/diagrams/pipeline.d2`):
```
Input: .ithmb file → Prefix Dispatch
  ├── T prefix → JPEG Decoder → Orientation → Output BGRA8
  ├── F prefix → Profile Lookup → SIMD/Scalar Decode → Rotation → Cropping → Output BGRA8
  ├── PhotoDB → Chunk Parser → per-entry dispatch to above paths
  └── Unknown → JPEG carving → JPEG Decoder → Output BGRA8
```

**Architecture diagram** (`docs/diagrams/architecture.d2`):
```
Workspace → ithmb-core → ithmb-cli, ithmb-core-cabi, ithmb-gen, pymod
ithmb-core → decoders/ (7 files), encoders/ (7 files), profile/, photodb/
decoders → rgb565.rs, rgb555.rs, ..., cl.rs (SIMD dispatch per format)
```

Diagrams are generated via `d2 docs/diagrams/pipeline.d2 docs/diagrams/pipeline.svg`. Deterministic, version-controlled, scriptable.

#### 1d — Commit Strategy

```
Rules:
- Conventional commits enforced by commit-msg hook: type(scope): description
- Types: feat, fix, docs, refactor, test, chore, cleanup
- One commit per logical change (not per file, not per day)
- CHANGELOG updated before every commit: add a line to [Unreleased] section in the same commit as the change. This is not a post-hoc activity — it's part of the commit workflow.
- `.githooks/` directory contains commit-msg hook — configured via `git config core.hooksPath .githooks` in setup script
- Squash policy: every PR/merge to main is squashed to one conventional commit
- Tags on main only: v1.0.0, v1.1.0, etc.

Why:
- No need for git-filter-repo (history is clean by construction)
- CHANGELOG is always up to date — never a retrospective batch update
- Single-commit archive is just `git merge --squash` away
```

#### 1e — CI Skeleton

```yaml
# ci.yml — minimal, Phase 1
steps:
  - cargo check
  - cargo clippy -- -D warnings
  - cargo test

# coverage.yml — added Phase 6
# benchmark.yml — added Phase 7
# release.yml — added Phase 8
```

# Additional CI tools leveraged (Phase 1):
# - typos (spell check) — cargo install typos-cli --locked --version 1.42.3, run on src/ docs/ README.md. MUST pin to version compatible with MSRV (latest may require newer rustc)
# - cargo-udeps (unused deps) — nightly only, run weekly
# - cargo-semver-checks (API breakage) — run on PRs that change public API
# - git-cliff (changelog gen from conventional commits) — run at release time

#### 1f — AGENTS.md Structure
> Spec: Repository Purpose → Structure → Decoder Pipeline Flow → Where to Look → Code Conventions → Test Patterns → What NOT to Do → Building → Key Decisions → Commands.
> See AGENTS.md for the authoritative version.

### Phase 2: Format Specification (Day 8-13)

**WHY**: The format spec IS the source of truth. Code is an implementation of it. Wrong spec → wrong code.

**WHAT**: `FORMAT.md` with bit-accurate layout descriptions, hex dumps, equations.

**ACCEPTANCE**: Every format variant is documented. Profile table is cross-referenced from >=2 sources. Device tables are complete.

**ANTI-PATTERN**: Updating spec as you go instead of writing it first.

**80/20 split**:
- 80% fixed: Document structure, section ordering, profile table columns, hex dump format
- 20% fill-in: Exact hex values, profile counts, device IDs

**Discipline**: Every time a new format variant is discovered while coding, update FORMAT.md BEFORE writing code.

---

### Phase 3: Synthetic Encoder (Day 13-18)

**WHY**: Encoder before decoder = TDD for binary formats. Forces full format understanding before any reading code exists.

**WHAT**: One encoder per format. Test vector generation.

**ACCEPTANCE**: `encode()` output matches known reference data. Test vectors are deterministic (same input → same file).
**STANDARDS**: OSS-Fuzz seed corpus preparation; Rust API Guidelines C-VALIDATE (validate arguments, _unchecked variants)

**ANTI-PATTERN**: Writing encoder and decoder simultaneously (double the debugging surface).

**Order**: RGB565 → RGB555 → UYVY → YCbCr420 → ReorderedRGB555 → CLCL → CL.

---

### Phase 4: Decoder Implementation (Day 18-38)

**WHY**: Core functionality.

**WHAT**: 7 decoders, each with cancellation support, scalar-first → roundtrip → concurrency → SIMD → fuzz → Miri → benchmark.
**ACCEPTANCE**: All 7 formats pass exhaustive roundtrip. 11+ concurrency tests. 5-min fuzz = 0 crashes. Miri Level 3 clean. SIMD matches scalar. &AtomicBool cancellation works at macroblock boundaries.
**STANDARDS**: Rust API Guidelines C-VALIDATE, C-COMMON-TRAITS, C-GOOD-ERR, C-SEND-SYNC, C-DEBUG, C-FAILURE, C-NEWTYPE, C-CUSTOM-TYPE, C-STRUCT-PRIVATE; Unsafe Code Guidelines / Nomicon (all unsafe blocks); SIMD Safety (target_feature dispatch, repr(packed) UB rules); OSS-Fuzz (8 target requirements, ASAN+UBSAN, seed corpus, dictionary); Google Testing Standards (test sizes, 90%+ branch coverage); CWE-125/674/770/190/415/476; OWASP Input Validation; Mutation Testing (80%+ score); Regression Testing (failing test before fix)
**ANTI-PATTERN**: SIMD before scalar (SSE2 buffer overrun in the actual project).

**Rules**:
9. Scalar first — simplest correct version. No SIMD until roundtrip works.
10. **CWE-125 pattern**: Every buffer access uses one of two patterns: (a) `buffer.get(index..index+len).ok_or(DecodeError::UnexpectedEof(pos))?` for safe path, (b) `// SAFETY: bounds checked at line N above` + `unsafe { buffer.get_unchecked(range) }` for hot path. Never use unchecked without a SAFETY comment referencing the bounds check.
11. **Input validation (OWASP)**: Every format field parsed from bytes must pass both syntactic (structure) and semantic (value range) validation. Reject unknown version numbers, type discriminators, and sizes on first encounter — not after partial decode.
12. Exhaustive roundtrip — 65,536 values for RGB565, 32,768 for RGB555, gradients, edges
13. Every decoder accepts `&AtomicBool` — checked at macroblock boundaries. Canceled decode returns `DecodeError::Canceled`.
14. Concurrency tests: 11+ scenarios (LRU cache contention, multi-frame decode, thread-safe profile publication, Barrier-sync cancel)
15. SIMD only after scalar verified — SSE2 → NEON → AVX2 (if bench shows need)
16. Fuzz at 5 min minimum per target, 0 crashes across all 3+ targets
17. Miri Level 3 (strict provenance + Tree Borrows) on all unsafe blocks — zero UB
18. Benchmark for information, not CI gate

**Parallel execution**: All 7 formats can run in parallel after Phase 2+3 complete. 7 agents, 2-3 days.

---

### Phase 5: PhotoDB & Device Profiles (Day 25-33)

**WHY**: ArtworkDB is half the ecosystem.

**WHAT**: Profile table, 18 device tables, PhotoDB parser/writer.

**ACCEPTANCE**: Parse→rebuild→parse identity. Each device table matches >=2 sources.
**STANDARDS**: CWE-770 (allocation limits — max field sizes, max record counts); OWASP Input Validation; Rust API Guidelines C-VALIDATE

**ANTI-PATTERN**: Incomplete profile table (Nano inversion bug).

**Starts**: As soon as Phase 2 (format spec) is done. Independent of decoders.

---

### Phase 6: Real Device Validation (Day 35-40)

**WHY**: Synthetic tests can't catch real-device quirks.

**WHAT**: Decode real files, compare with reference tools. Fuzz. Cross-reference.

**ACCEPTANCE**: 100% of known real files decode correctly. 0 fuzz crashes.
**STANDARDS**: OSS-Fuzz regression corpus; Regression Testing discipline (every bug = new test case); Google Testing (large test sizing)

**ANTI-PATTERN**: Optimizing before validating.

---

### Phase 7: CLI + C ABI + Python + Polish (Day 40-50)

**WHY**: Deliver the original ambition + all the bonus targets.

**WHAT**: `ithmb` CLI binary, `ithmb-core-cabi` cdylib, `pymod` Python package, published README, green CI.

**ACCEPTANCE**: CLI `--help` works. ImageGlass loads plugin. Python `pip install` works. README complete. All CI green.
**STANDARDS**: Conventional Commits; Semver 2.0.0; Keep a Changelog; cargo-deny (advisories + licenses + bans); cargo-audit

**ANTI-PATTERN**: Over-featuring the CLI.
**C ABI test harness**: Write `tests/c_api_test.rs` testing every exported `ig_plugin_*` function via `extern "C"` linkage. Verify return codes, decode output, and cancellation from C side. Do not defer to ImageGlass — C ABI bugs (calling convention, lifetime, ABI mismatch) are hardest to debug inside the host.
**ImageGlass ABI reference**: Document the exact entry points in `docs/imageglass-plugin.md`: `ig_plugin_get_api`, `CodecDecodeStaticRaster` signature, error codes. An agent should not need to read ImageGlass source.
**Release workflow**: 1. bump version → 2. CHANGELOG [Unreleased]→[X.Y.Z] + date → 3. `git commit -m "chore(release): vX.Y.Z"` → 4. `git tag vX.Y.Z` → 5. `git push --tags` → 6. GitHub Release auto-created by CI

---

### Phase 8: Archive (Day 50-55)

**WHY**: Lock in quality, prevent decay.

**WHAT**: Squash reference repos, archive C# branch, freeze.

**ACCEPTANCE**: Single-commit repos, tags correct, CI green, AGENTS.md + ARCHITECTURE.md present.
**STANDARDS**: ALL standards from Layer 6 verified as met — this is the final compliance gate

---

## Layer 3: Micro Plan Example (RGB565)

Same as before — every step has WHAT/WHY/ACCEPTANCE/VERIFY. See the full file for all 7 micro plans.

**Key innovation**: The 7 format micro-plans are IDENTICAL in structure. Only the format-specific constants differ (bit count, mask, chroma vs RGB). This is deliberate — a template micro-plan designed in Phase 1, parameterized per format.

---

## Layer 4: Dependency Matrix

```
Phase 0 (Deep Discovery) — 5 days
  │
  ├──→ Phase 1 (Structural Bootstrap) — 3 days [needs: Phase 0]
  │     │
  │     ├──→ Phase 2 (Format Spec) — 5 days [needs: Phase 0]
  │     │
  │     ├──→ Phase 3 (Encoder) — 5 days [needs: Phase 1]
  │     │      │
  │     │      └──→ Phase 4 (Decoders) — 20 days [needs: Phase 2, 3]
  │     │             │                   └── 7 formats in parallel = 3 days with 7 agents
  │     │             │
  │     │             └──→ Phase 6 (Validation) — 5 days [needs: Phase 4, 5]
  │     │
  │     ├──→ Phase 5 (PhotoDB) — 8 days [needs: Phase 2]
  │     │     (independent of decoders — parallel track)
  │     │
  │     └──→ Phase 7 (CLI + C ABI + Python) — 10 days [needs: Phase 4]
  │
  └──→ Phase 8 (Archive) — 5 days [needs: ALL above]
```

**Total calendar time**: ~55 days sequential, ~25 days with max parallelization.

---

## Layer 5: Verification Chain

```
Step done
  → cargo test + clippy pass (+ fuzz if applicable for the phase)
  → micro-plan gate: all tests in this micro-plan pass
  → phase gate: all micro-plans in this phase complete + phase-level tests pass
  → plan gate: all phases complete + "done" criteria from Layer 1 met
  → ambition satisfied: .ithmb files open in ImageGlass, CLI works, docs complete
```

---

## Layer 6: Authoritative Standards Registry

Every phase complies with these standards. Source URLs in the research phase documents.

- **A. Rust**: API Guidelines (C-VALIDATE, C-COMMON-TRAITS, C-GOOD-ERR, C-SEND-SYNC, C-FAILURE, C-DEBUG, C-NEWTYPE, C-CUSTOM-TYPE, C-STRUCT-PRIVATE, C-CRATE-DOC, C-EXAMPLE, C-METADATA) / Unsafe Code (// SAFETY: on every block) / SIMD (target_feature with runtime check)
- **B. Testing**: OSS-Fuzz (8 reqs, seed corpus, ASAN+UBSAN) / Google (S/M/L, 90%+ branch) / Mutation (80%+ score) / Regression (failing test before fix)
- **C. Security**: CWE-125/674/770/190/415/476 / OWASP Input Validation (syntactic+semantic, allowlist)
- **D. Project**: Conventional Commits / Semver / Keep a Changelog / GitHub (README+LICENSE required)
- **E. CI/CD**: Build matrix (3 OS x 2 features) / cargo-deny + cargo-audit / typos (MSRV-pinned) / cargo-udeps / semver-checks / git-cliff / All tools MSRV-verified
- **F. AI and Licensing**: MIT / CREDITS.md / DCO / [ai-assisted] PR annotations
## Plan Verification Loop (Recursive Refinement)

**Every time the plan changes, dry-run it against the actual project trajectory to find gaps.** This is not a one-time activity — the plan evolves until no gaps remain.

### Protocol

1. **Load the current plan**
2. **Walk through each phase** and ask: "If I followed this exactly, would it produce equal or better quality than what actually happened?"
3. **For each gap found**:
   - Add a new requirement to Layer 1 (Full Spec)
   - Add a new step to the relevant Phase in Layer 2
   - Document WHY it was missing (so the pattern doesn't repeat)
4. **Repeat until zero gaps** — the plan is complete when dry-run finds nothing to add
5. **Execute** — stick to the plan. If a mid-project discovery reveals a new requirement, pause execution, update the plan first, then resume.

### What to check in a dry-run

| Dimension | Check | Example gap found in this session |
|-----------|-------|-----------------------------------|
| Research depth | Does it cover ALL reference tools? | Was missing decompilation methodology until flagged |
| File organization | Does the tree match the final state? | Was missing references/ directory, tools/ scripts |
| README | Are all sections planned? | Was missing centered logo, badge grouping, AI credits |
| CHANGELOG | When is it updated? | Was "retrospective batch update" — now per-commit |
| Commit history | Is it clean by construction? | Would have needed git-filter-repo without squash policy |
| Badges | Are they scripted? | Were hand-edited SVGs, now `tools/generate-badges.py` |
| Attribution | When is it decided? | Was "at the end", now Phase 1 |
| Diagrams | How are they generated? | Were hand-edited, now d2lang source files |
| CI | When does each stage activate? | Would have added all CI at once, now phased |
| Plan itself | Does the plan include plan refinement? | Was missing — added this section |

---

## Summary: What This Plan Guarantees

| Concern | How the plan addresses it |
|---------|--------------------------|
| No C# detour | Layer 0 interrogation + researched decision |
| File organization stable | Phase 1 designs the final tree, never reorganized |
| README perfect from start | Phase 1 templates the README with researched format |
| Badges scripted, not hand-edited | `tools/generate-badges.py` from Phase 1 |
| SVG diagrams deterministic | d2lang source files in Phase 1 |
| Commit history clean | Conventional commits + squash policy from Phase 1 |
| No git-filter-repo needed | Clean commits from the start |
| Attribution correct | CREDITS.md + ECOSYSTEM.md from Phase 1 |
| AGENTS.md exists from day 1 | Written in Phase 1, maintained through project |
| Reference projects credited | ECOSYSTEM.md with cross-reference table |
| Research depth | Phase 0 Track A covers ALL reference implementations |
| 80/20 data fill-in | Placeholders in templates, replaced when discovered |
| One-shot archive | No filter-repo, no squashing after the fact |
| CHANGELOG current | Updated per-commit (part of commit workflow) — was retrospective batch |
| Plan evolves with gaps | Plan Verification Loop after every plan change |
| Rust API Guidelines | 12 critical items spec'd: C-VALIDATE, C-COMMON-TRAITS, C-GOOD-ERR, C-SEND-SYNC, C-FAILURE, C-DEBUG, C-NEWTYPE, C-CUSTOM-TYPE, C-STRUCT-PRIVATE, C-CRATE-DOC, C-EXAMPLE, C-METADATA [A1] |
| Unsafe code safety | All unsafe blocks have SAFETY comments, Miri Level 3, cargo-geiger tracked [A2] |
| SIMD safety | target_feature dispatch with runtime detection, repr(packed) rules documented [A3] |
| Fuzz quality | OSS-Fuzz: 8 reqs, seed corpus, dictionary, ASAN+UBSAN, CI integration [B1] |
| Test quality | Google sizes (S/M/L), 90%+ branch coverage, mutation score 80%+ [B2, B3] |
| Regression discipline | Failing test before fix, reproducer-per-bug, git bisect run [B4] |
| Security | 6 CWE mitigations spec'd: 125, 674, 770, 190, 415, 476. OWASP validate+allowlist [C1, C2] |
| Supply chain | cargo-deny + cargo-audit from Phase 1 [E2] |
| CI/CD | Build matrix (OS × features × arch), benchmark regression, platform docs [E1, E3, E4] |
| Licensing | MIT [F2] |
| AI disclosure | CREDITS.md + DCO + [ai-assisted] annotation in PRs [F1] |
| 80/20 data fill-in | Placeholders in templates, replaced when discovered |
| One-shot archive | No filter-repo, no squashing after the fact |
| CHANGELOG current | Updated per-commit (part of commit workflow) | Was retrospective batch update
| Plan evolves with gaps | Plan Verification Loop after every plan change | Was missing — plan was treated as static

