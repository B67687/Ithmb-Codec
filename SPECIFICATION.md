# SPECIFICATION.md: The Plan IS the Spec

> **Status:** This spec describes the AS-BUILT system. The project is in the IMPLEMENTED/POLISHED phase: a working, tested, shipped codebase. Sections below record the decisions that produced the current state, not a forward-looking build plan. Where a section would normally describe intent, it records the realized contract and the verification that proves it.
>
> **Three layers:** MACRO (system), MESO (component), MICRO (implementation). An AI executor reads this and knows exactly what the system is and how to change it without guessing.
>
> **Input:** This spec consumes validated assumptions from the C# reference implementation (B67687/Ithmb-Codec-CSharp, archived), hardware validation from the iOpenPod community, and the standards audit at HEAD aae56f4 (105 passed / 10 failed / 9 pending). Validated learnings directly inform sections 2, 5, and 7.

---

## How to Read This Spec

**Design influences:** Volere (Robertson & Robertson 2006) requirements shell; IEEE 830 / ISO 29148 SRS structure; Shape Up (Singer 2019) pitch format; Jackson Problem Frames (2001) domain analysis.

| Layer | Level | Scope | Changing this requires |
| --- | --- | --- | --- |
| **MACRO** | System | Decisions constraining the entire project | A learning shift (RULES.md section 5) |
| **MESO** | Component | Contracts between components | Interface renegotiation |
| **MICRO** | Implementation | Bounds within which the executor has freedom | None, the executor decides within bounds |

**Priority tiers:** Tier 1 (sections 0-7) required for ANY project; Tier 2 (8-11) production; Tier 3 (12-14) open-source/long-lived. All tiers are filled for this project.

---

## 0. Constitution (Immutable Project Rules)

The constitution constrains ALL executor actions. If an action would violate these rules, the executor MUST refuse.

### MACRO: System Principles

```
Ithmb-Codec Constitution:
1. Correctness: wrong output at any speed is useless. A decoded pixel that
   differs from the reference implementation is a bug, not a tradeoff.
2. No magic: explicit > implicit. Every dependency, feature flag, and config
   is declared in Cargo.toml or deny.toml. No hidden runtime behavior.
3. Inward dependencies: core knows nothing about edges. ithmb-core imports
   zero CLI, WASM, Python, or C-ABI code.
4. Test what matters: one behavior per test, edge cases before happy path.
   Roundtrip, corruption, and boundary tests outnumber happy-path tests.
5. Fail with context: every error includes the values that caused it, not
   just a message. DecodeError carries expected/actual sizes and limits.
6. Tool-first: never hand-roll what a deterministic tool handles. rustfmt,
   clippy, cargo-deny, cargo-audit, cargo-fuzz, and Miri do the mechanical work.
7. No new runtime dependency without a Y-Statement decision recorded in
   section 2. The dependency table in section 5 is the closed set.
8. Memory safety: no unsafe code outside the SIMD kernels. Workspace lint
   unsafe_code = "deny"; every unsafe block carries a SAFETY comment.
```

**MESO/MICRO:** Inward dependencies means `crates/ithmb-core` never imports `crates/ithmb-cli`, `crates/ithmb-wasm`, `pymod`, or `crates/ithmb-gen`; all four wrappers depend on core. `unsafe` is confined to `crates/ithmb-core/src/simd/*.rs` (SSE2/AVX2/NEON kernels) and the C-ABI boundary in `c_api.rs`. All public functions in ithmb-core have doc comments (`#![warn(missing_docs)]`) and a corresponding test. Every decoder returns `DecodeError` on failure, never a panic and never a raw I/O error.

---

## 1. Overview & Derived Ambition

### MACRO: System Vision

```
Project name: Ithmb-Codec
One-line: Pure Rust codec library, CLI tool, and C ABI shared library for
  decoding and encoding Apple .ithmb thumbnail-cache files (iPod/iPhone PhotoDB).
Core ambition: Decode every known .ithmb thumbnail file from iPod Photo 4G
  through iPhone 2G and iPod Nano 7G with pixel-exact output, verified against
  hardware samples and the C# reference implementation.
Why now: The C# reference was trapped inside ImageGlass (Windows-only, no
  crates.io/PyPI distribution). Rust enables a library, CLI, Python bindings,
  and WASM from one codebase, and the iOpenPod community provides hardware
  validation that no single developer could assemble alone.

Success criteria:
- WHEN the full test suite runs THEN 570 unit tests across 17 suites pass
  with zero failures (docs/STATS.md).
- WHEN a known-profile .ithmb file is decoded THEN the output matches the
  C# reference golden vectors (14 golden test vectors pass).
- WHEN fuzzing runs THEN 1.2M+ iterations complete with zero crashes and
  zero Miri UB findings (3 libfuzzer targets, 21 Miri tests).
- WHEN a user runs the CLI on a 480x864 RGB565 frame THEN decode completes
  in under 100 ms on commodity hardware (measured 45 us at 720x480).
- WHEN a real iPod Classic 6G sample is decoded THEN the BGR15 channel-swap
  and MSB replication match the reference PNGs (F1061/F1055/F1060 validated).

Stakeholders: iPod/iPhone owners recovering photos, digital forensics
  practitioners, ImageGlass users (via the separate plugin repo), Python and
  ML pipeline developers, browser users (via the WASM decoder), the iOpenPod
  reverse-engineering community, and the single maintainer B67687.
```

**MESO/MICRO:** The library crate, CLI, WASM bindings, Python bindings, and sample generator are in scope. The ImageGlass plugin is out of scope for this repo (separate repository, ADR-0002). No cloud dependencies, no accounts, no telemetry: the codec is fully offline and privacy-preserving.

### OUT OF SCOPE (V1)

- iOS 13+ thumbnail format, a different proprietary format, explicitly not decodable by this codec (README "Not an iOS 13+ thumbnail decoder")
- HEIC/AVIF thumbnail extraction, deferred to v2.1 (ROADMAP)
- GPU-accelerated decode via compute shaders, long-term and low priority (ROADMAP)
- `no_std` support for embedded targets, deferred to v2.0 (ROADMAP)
- Plug-in system for custom format handlers, deferred until the internal decoder trait stabilizes (ROADMAP)
- Streaming / batched decode API, deferred to v2.1 (ROADMAP)
- FFmpeg / libav integration for video thumbnail extraction, speculative, no confirmed format (ROADMAP)
- Formal verification of SIMD kernels via KLEE, long-term and toolchain-dependent (ROADMAP)

---

## 2. Architecture & Design Decisions

### MACRO: System Architecture

Each architecture-level decision uses the Y-Statement format:

> In the context of {{situation}}, facing {{concern}}, we decided for {{option}} to achieve {{goal}}, accepting {{downside}}.

```
### Decision 1: Rust port over the C# Native AOT plugin
In the context of a proven C# codec that was trapped inside the Windows-only
ImageGlass plugin ABI,
facing the need to distribute the codec as a library, CLI, Python bindings,
and WASM,
we decided for a pure Rust workspace port (ithmb-core + wrappers)
to achieve crates.io, PyPI, and cross-platform reach from one codebase,
accepting a full rewrite and a parity gap that took 3 waves of quality work
to close (docs/EVOLUTION.md).

Alternatives considered:
- Keep the C# plugin and add wrappers, rejected because Native AOT DLLs
  cannot ship as crates.io libraries or PyPI wheels.
- Port to Go, rejected because Go lacks the SIMD control and zero-cost
  FFI that the decoder kernels need.

### Decision 2: Pipelined dispatch over a monolithic decoder
In the context of 8 pixel formats and 53 active profiles sharing one file
format,
facing the combinatorial explosion of per-format, per-profile code paths,
we decided for a central pipeline module (crates/ithmb-core/src/pipeline/)
that reads the format prefix, resolves a profile, dispatches to the right
decoder, and applies post-processing,
to achieve one dispatch path for every format and profile combination,
accepting an indirection layer that callers must learn.

Alternatives considered:
- One function per format with inline profile logic, rejected because
  profile post-processing (crop, rotation, channel swap) would be duplicated
  across every decoder.

### Decision 3: Runtime SIMD dispatch with scalar fallback
In the context of YUV-to-BGRA conversion being the ALU-bound hot path,
facing the need to run correctly on SSE2, AVX2, and NEON hardware without
per-target builds,
we decided for runtime `is_x86_feature_detected!` dispatch with a scalar
fallback (ADR-0001),
to achieve 2-5x throughput on YUV decoders while remaining correct everywhere,
accepting that macOS ARM NEON is gated on CI runners and falls back to scalar
(known edge case, documented in docs/standards/STANDARDS.md).

Alternatives considered:
- Compile-time feature gating per target, rejected because it multiplies
  the CI matrix and breaks prebuilt binary distribution.
- Hand-written SIMD for RGB565/RGB555, rejected after measurement: the
  hand-written SSE2/AVX2 was 34x slower on Intel due to AVX frequency
  downclock and port-5 bottleneck; auto-vectorized scalar loops win.

### Decision 4: C ABI plugin split into a separate repository
In the context of the ImageGlass v10 native plugin ABI being a Windows-only
consumer,
facing scope coupling between the format codec and a single GUI application,
we decided to maintain the C ABI plugin in its own repository
(ImageGlass-Ithmb-Plugin, ADR-0002)
to achieve independent versioning and CI for the plugin,
accepting that the C ABI surface in ithmb-core (c_api.rs, feature "c") is
verified by CI but consumed elsewhere.

### Decision 5: 8 MB file size guard
In the context of decoding untrusted .ithmb files where the largest known
real frame is 810 KB,
facing OOM/DoS risk from pathological input,
we decided to reject files over 8 MB before any allocation (ADR-0005),
to achieve a ~10x safety margin over the largest observed frame,
accepting that a hypothetical legitimate file over 8 MB would be rejected.

### Decision 6: zune-jpeg over the image crate for JPEG decode
In the context of T-prefix files that embed a JPEG stream,
facing the need for a pure Rust JPEG decoder with no C dependencies,
we decided for zune-jpeg 0.5 (ADR-2026-08-16)
to achieve a small, auditable, dependency-light decode path,
accepting a narrower feature set than the full image crate.

### Decision 7: Feature-gated optional capabilities
In the context of a library that must stay lean for embedded and WASM users,
facing the tension between optional features and default simplicity,
we decided for feature flags (cache, metrics, c, logging) with an empty
default set,
to achieve a minimal dependency footprint by default,
accepting that optional features must be explicitly enabled by consumers.
```

**MESO/MICRO:** Component contracts: `ithmb-core` exposes `decode_ithmb`, `open_ithmb`, `decode_with_profile`, and the `_with_config`/`_with_transform` variants, all returning `Result<DecodedImage, DecodeError>`. `DecodedImage` is BGRA8 pixel data plus width and height. The CLI, WASM, and Python wrappers call only these public entry points. No wrapper reaches into decoder internals. Coding constraints: no `unwrap` in library code (test-only unwraps are justified per REVIEW 2.4), no unsafe outside simd/ and c_api.rs, exhaustive `match` on `Encoding`.

### PROJECT_MODEL: Whole-Project State Machine (MANDATORY)

Every project MUST document its whole-project state machine at `docs/PROJECT_MODEL.md`. Not optional polish, it is the project's health contract.

| Element | What it is | Example |
| --- | --- | --- |
| **States** | The project's lifecycle states | `IDEA → SPEC'D → PROTOTYPED → IMPLEMENTED → POLISHED → SHIPPED → MAINTAINED → EVOLVED` |
| **Valid transitions** | Allowed state changes, including feature additions and rollbacks | `SHIPPED → EVOLVED` (new feature), `SHIPPED → MAINTAINED` (bugfix) |
| **Invalid transitions** | Forbidden state changes (the invariants) | `IMPLEMENTED → SHIPPED` without passing POLISHED |
| **Invariants** | What must never change during any transition | "The 8 MB file size guard never shrinks below the largest known frame", "API v1 responses never remove fields" |
| **Blast radius map** | Which components are coupled and co-change together | "changing the profile schema requires checking pipeline/, profile_db/, profile_parser/, and the CLI" |

**Gate check:** SPECIFICATION is incomplete without PROJECT_MODEL.md. REVIEW checks that every addition's transition is in the table. The full table lives at `docs/PROJECT_MODEL.md`.

---

## 3. File Tree & Module Responsibilities

### MACRO: Directory Structure

```
Ithmb-Codec/: Rust workspace, 5 members, fuzz/ excluded from the workspace
+-- Cargo.toml: workspace manifest: version 1.9.9, edition 2024, rust-version 1.88.0, MIT
+-- crates/ithmb-core/: the codec library (lib + cdylib), all decoder logic
+-- crates/ithmb-cli/: the ithmb CLI binary (clap, png output)
+-- crates/ithmb-gen/: sample generator binary for synthetic test vectors
+-- crates/ithmb-wasm/: WASM bindings (wasm-bindgen, cdylib)
+-- pymod/: Python bindings (PyO3, abi3-py312, cdylib)
+-- fuzz/: libfuzzer targets (6 targets, excluded from workspace)
+-- .github/workflows/: pr-checks.yml, ci-full.yml, release.yml
+-- docs/: architecture, ADRs, benchmarks, guides, standards, STATS.md
+-- deny.toml: cargo-deny policy (licenses, bans, sources allowlist)
+-- scripts/: CI helper scripts (check-ci-pins.sh)
+-- tools/: benchmark regression checker (check-benchmark-regression.sh)
```

### MESO: Per-Module Contracts (Parnas-style)

```
crates/ithmb-core/src/pipeline/: central decode dispatch
  HIDES: prefix parsing, profile resolution order, JPEG-carving fallback,
         post-processing order (swap, crop, rotate)
  EXPORTS: decode_ithmb, decode_ithmb_with_config, decode_ithmb_with_transform,
           decode_with_profile, decode_with_profile_with_config,
           decode_with_profile_with_transform, open_ithmb, open_ithmb_with_config,
           encoding_name_for_prefix
  CALLER: ithmb-cli, ithmb-wasm, pymod, c_api.rs, tests
  Precondition: input slice is the full file bytes; canceled is a valid AtomicBool
  Postcondition: Ok(DecodedImage) with BGRA8 data, or Err(DecodeError)
  Invariant: never panics on malformed input; size guard enforced before decode

crates/ithmb-core/src/profile_db.rs: built-in profile database
  HIDES: embedded JSON loading, DISABLED_PREFIXES (1044), Nano 7G alternates
  EXPORTS: ProfileDb::load_builtin, get, resolve, all
  CALLER: pipeline, CLI, tests
  Precondition: embedded data/profiles.json parses
  Postcondition: 53 active profiles keyed by prefix; 1044 filtered out
  Invariant: profile 1044 stays disabled (writing it corrupts iPod cover art)

crates/ithmb-core/src/profile.rs: profile type and encoding enum
  HIDES: the IthmbVariantProfile/IthmbEncoding field semantics
  EXPORTS: Profile, Encoding, built_in_profiles
  CALLER: pipeline, profile_db, profile_parser, encoders, decoders
  Precondition: none
  Postcondition: a Profile fully describes one format variant
  Invariant: Encoding is exhaustive; match on Encoding covers all 8 decoders

crates/ithmb-core/src/{rgb565,rgb555,reordered_rgb555,uyvy,ycbcr420,cl,clcl}.rs: raw decoders
  HIDES: per-format pixel unpacking and YUV-to-BGRA conversion
  EXPORTS: decode(data, profile, canceled) -> Result<DecodedImage, DecodeError>
  CALLER: pipeline::dispatch_decode
  Precondition: data is the frame payload (prefix already stripped)
  Postcondition: BGRA8 pixels at profile dimensions
  Invariant: output length == width * height * 4

crates/ithmb-core/src/jpeg.rs: JPEG-embedded stream decoder
  HIDES: zune-jpeg integration, EXIF orientation (0x0112) parsing
  EXPORTS: decode(data, profile, canceled)
  CALLER: pipeline::dispatch_decode
  Precondition: data starts with JPEG SOI (FF D8)
  Postcondition: decoded BGRA8 image, orientation applied
  Invariant: JPEG SOI must be within the first 4 MB (covers all known real files)

crates/ithmb-core/src/simd/: SIMD kernels (SSE2/AVX2/NEON) + scalar fallback
  HIDES: ISA-specific YUV conversion, runtime dispatch
  EXPORTS: per-format conversion functions behind a common signature
  CALLER: yuv.rs, uyvy.rs, ycbcr420.rs, cl.rs, clcl.rs
  Precondition: feature detection has selected the active ISA
  Postcondition: correct BGRA output for the selected ISA
  Invariant: scalar fallback always available; unsafe confined to this module

crates/ithmb-core/src/enc/: synthetic encoders for all raw formats
  HIDES: BGRA-to-YUV conversion, interlace fields, BT.601 coefficients
  EXPORTS: encode functions per format
  CALLER: ithmb-gen, roundtrip tests
  Precondition: BGRA input at known dimensions
  Postcondition: valid .ithmb frame bytes for the target format
  Invariant: encoder output roundtrips through the matching decoder

crates/ithmb-core/src/photodb/: PhotoDB/ArtworkDB chunk parser
  HIDES: MHFD, MHSD, MHNI chunk tree traversal
  EXPORTS: try_parse_photodb, PhotoDbEntry, PhotoDbMetadata, writer, integrity checker
  CALLER: pipeline::open, CLI --open, tests
  Precondition: input starts with "mhfd" magic
  Postcondition: parsed entries with inline pixel data or external file references
  Invariant: chunk offsets validated against buffer length before reads

crates/ithmb-core/src/error.rs: typed error enum and image container
  HIDES: error formatting and structured fields
  EXPORTS: DecodeError (8 variants), DecodedImage
  CALLER: every module
  Precondition: none
  Postcondition: every failure returns a DecodeError with context
  Invariant: DecodeError is #[non_exhaustive]; no raw I/O errors escape

crates/ithmb-core/src/c_api.rs: C ABI exports (feature "c")
  HIDES: FFI-safe marshalling of DecodedImage/DecodeError
  EXPORTS: ithmb_decode, ithmb_prefix_to_profile (verified by nm in CI)
  CALLER: external C consumers (ImageGlass plugin repo)
  Precondition: valid pointers and lengths from the C caller
  Postcondition: decoded BGRA buffer or error code
  Invariant: no panics cross the FFI boundary

crates/ithmb-cli/src/main.rs: CLI binary
  HIDES: clap argument parsing, PNG encoding, output file handling
  EXPORTS: the ithmb binary (decode, --info, --list-profiles, --frame,
           --raw, --open, --frame-count, --extract-all, --format)
  CALLER: end users, scripts
  Precondition: valid input path
  Postcondition: decoded output file or metadata printed
  Invariant: anyhow::Result at the boundary; DecodeError converted to a message

crates/ithmb-wasm/src/lib.rs: WASM bindings
  HIDES: wasm-bindgen marshalling
  EXPORTS: decode functions callable from JavaScript
  CALLER: browser web apps (ithmb-codec.dev decoder)
  Precondition: valid input bytes from JS
  Postcondition: decoded image data returned to JS
  Invariant: no panics across the wasm-bindgen boundary

pymod/src/lib.rs: Python bindings (PyO3)
  HIDES: PyO3 marshalling, abi3-py312 ABI
  EXPORTS: decode_ithmb, open_ithmb, list_profiles
  CALLER: Python scripts and ML pipelines
  Precondition: valid bytes input
  Postcondition: dict with width, height, BGRA data, format, rotation
  Invariant: no panics across the PyO3 boundary

crates/ithmb-gen/src/main.rs: sample generator binary
  HIDES: encoder invocation and file writing
  EXPORTS: the ithmb-gen binary
  CALLER: developers and testers
  Precondition: valid dimensions and format arguments
  Postcondition: synthetic .ithmb file written
  Invariant: generated files decode with the matching decoder

fuzz/: libfuzzer targets (6: decode_ithmb, decode_pipeline, encode_roundtrip,
        open_ithmb, parse_photodb, parse_profile)
  HIDES: fuzz harness setup
  EXPORTS: fuzz targets run by cargo-fuzz in CI
  CALLER: CI (ci-full.yml fuzz job)
  Precondition: none
  Postcondition: no crashes, no hangs, no OOM across iterations
  Invariant: fuzz corpus committed; artifacts uploaded on failure
```

**MICRO:** Naming conventions follow `snake_case` modules and `decode` entry points per decoder. The 250 LOC ceiling per module is enforced by review (rust-workflow rule 10); `pipeline/mod.rs` is the largest module and is split into `open.rs` and `profile_loader.rs` submodules. All public functions have doc comments and tests.

---

## 4. Quality Gates & Verification

### MACRO: Quality Gates

Acceptance criteria in EARS notation. These are the gates that CI enforces on every change.

> WHEN a pull request is opened or a commit lands on main
> THEN pr-checks.yml SHALL run: cargo fmt --check, cargo clippy --workspace --all-targets -- -D warnings, typos-cli, lychee link check, cargo-deny check, cargo-audit, cargo doc with RUSTDOCFLAGS="-D warnings", gitleaks secrets scan, and scripts/check-ci-pins.sh
> WHERE any check fails
> THEN the change SHALL be rejected with the failing check's output.

> WHEN a commit lands on main
> THEN ci-full.yml SHALL run: the 3-OS build and test matrix (ubuntu, macos, windows), the benchmark regression check (FAIL_THRESHOLD=1.25 against .github/baseline.json), 6 fuzz targets (30 s each), the C-API build with symbol verification (nm -D | grep ithmb_decode), and the WASM build
> WHERE any job fails
> THEN the change SHALL be rejected and the failure SHALL be visible on the main branch status.

> WHEN a version tag v* is pushed
> THEN release.yml SHALL run: clippy, cargo-audit, cargo test --workspace, then cross-compile the CLI for 5 targets and build Python wheels for 5 targets via maturin
> WHERE the release build fails
> THEN no GitHub Release SHALL be created.

> WHEN a decoder is modified
> THEN the roundtrip tests for that format SHALL pass (RGB565: 65,536 values; RGB555: 32,768; CL: 15,625 nibble combos) and the golden vector tests SHALL pass
> WHERE a golden vector differs
> THEN the change SHALL be rejected as a regression against the C# reference.

**MESO/MICRO:** The fuzz targets in `fuzz/` are the verification requirement for the parser and pipeline modules. Miri runs 21 SSE2 tests with zero UB findings. The benchmark regression gate (25% threshold) protects the SIMD hot paths. `cargo-deny` enforces the license allowlist, multiple-versions ban, and the crates.io-only source allowlist.

---

## 5. Dependencies & External Contracts

### MACRO: System Dependencies

| Package | Version | Purpose | Contract | License |
|---------|---------|---------|----------|---------|
| zune-jpeg | 0.5 | Pure Rust JPEG decoder for T-prefix embedded streams | JPEG SOI to EOI decode, EXIF orientation | MIT/Apache-2.0 |
| lru | 0.18.0 | LRU raw file cache (feature "cache") | Bounded cache of decoded raw files | MIT |
| thiserror | 2 | Derive std::error::Error for DecodeError | Error enum derivation | MIT/Apache-2.0 |
| log | 0.4 | Logging facade (feature "logging") | debug/info/warn emission | MIT/Apache-2.0 |
| clap | 4 | CLI argument parsing (derive) | CLI definition and help text | MIT/Apache-2.0 |
| anyhow | 1 | Application-level error propagation in the CLI | Contextual error wrapping | MIT/Apache-2.0 |
| png | 0.18 | PNG encoding in the CLI (feature "png-output") | BGRA to PNG encoding | MIT/Apache-2.0 |
| pyo3 | 0.29 | Python bindings (abi3-py312) | Python module marshalling | MIT/Apache-2.0 |
| wasm-bindgen | 0.2 | WASM bindings | JS/Rust marshalling | MIT/Apache-2.0 |
| wasm-bindgen-test | 0.3 | WASM runtime tests (dev) | Browser/Node test runner | MIT/Apache-2.0 |
| divan | 0.1.21 | Benchmark harness (dev) | Benchmark execution | MIT/Apache-2.0 |
| image | 0.25 | Golden-vector reference decode (dev) | Reference PNG comparison | MIT/Apache-2.0 |
| proptest | 1.5 | Property-based testing (dev) | Random input generation | MIT/Apache-2.0 |

**MESO/MICRO:** Every dependency has a version constraint in Cargo.toml. Runtime dependencies are the closed set above; the `image` crate is a dev-dependency for golden-vector tests only. `deny.toml` pins the source registry to crates.io (unknown-registry = deny, unknown-git = deny) and allows a fixed license allowlist. The single advisory ignore (RUSTSEC-2024-0436, paste 1.0.15) is a transitive dev-only dependency through rav1e/ravif/image, not a runtime concern. No new runtime dependency is added without a Y-Statement in section 2.

---

## 6. UX & Interface Contract

### MACRO: User-Facing Behavior

```
### Entry Points
- ithmb CLI binary: decode, inspect, and extract .ithmb files (end users, scripts)
- ithmb-core library API: decode_ithmb / open_ithmb / decode_with_profile (Rust developers)
- ithmb-python module: decode_ithmb / open_ithmb / list_profiles (Python developers)
- ithmb-wasm: decode functions callable from JavaScript (browser web apps)
- C ABI: ithmb_decode / ithmb_prefix_to_profile (any language with C FFI)
- ithmb-gen: synthetic sample generator (developers and testers)

### User-Facing Behavior (EARS)
WHEN a user runs `ithmb input.ithmb output.png`
THEN the system SHALL decode the file and write a PNG
WHERE the file is a known profile
THEN the system SHALL produce pixel-exact output matching the reference.

WHEN a user runs `ithmb --info input.ithmb`
THEN the system SHALL print size, prefix, profile, and frame count
WHERE the file is unreadable
THEN the system SHALL print an error message and exit non-zero.

WHEN a user runs `ithmb --list-profiles`
THEN the system SHALL print the 53-profile database as a formatted table.

WHEN a user runs `ithmb --frame N input.ithmb`
THEN the system SHALL extract frame N from a multi-frame file
WHERE N is out of range
THEN the system SHALL return an error.

WHEN a user runs `ithmb --open PhotoDB`
THEN the system SHALL parse the container and extract all thumbnails.

WHEN a user calls `decode_ithmb(data, canceled)` from Python or Rust
THEN the system SHALL return a dict/struct with width, height, BGRA data, format, and rotation
WHERE the input exceeds the 8 MB guard
THEN the system SHALL raise FileTooLarge with the size and limit.

WHEN a user cancels a decode via the AtomicBool flag
THEN the system SHALL stop at the next cancellation checkpoint and return Canceled.
```

**MESO: Error Contract:**

| Condition | Error | Remediation | Log Level |
|-----------|-------|-------------|-----------|
| Input shorter than 4 bytes | DecodeError::BufferTooShort { expected: 4, actual } | Caller supplies a complete file | error |
| File over 8 MB | DecodeError::FileTooLarge { size, limit } | Caller rejects the file before decode | error |
| Unknown format prefix, no JPEG | DecodeError::Unsupported(format) | Caller reports the prefix; user opens an issue with a sample | warn |
| Corrupt or unsupported JPEG | DecodeError::Jpeg(detail) | Caller reports the failure | error |
| Invalid chunk structure | DecodeError::InvalidFormat(detail) | Caller reports the failure | error |
| Profile mismatch or config error | DecodeError::Profile(detail) | Caller checks the profile | warn |
| Caller requested cancellation | DecodeError::Canceled(detail) | Caller retries or aborts | info |
| I/O-level failure | DecodeError::Io(detail) | Caller checks the source | error |

**MICRO:** The CLI prints DecodeError messages directly (anyhow context wrapping). The Python module raises exceptions carrying the same detail strings. The WASM module returns error strings to JS. No error path panics; every failure is a typed DecodeError.

---

## 7. Timeline, Milestones & Checkpoints

### MACRO: Project Appetite

```
Appetite: the project is already shipped. The realized timeline is recorded here
as the milestone history; future work follows the ROADMAP (v1.10, v2.0, v2.1).

| Milestone | What shipped | Checkpoint | Acceptance Criteria |
|-----------|------------|------------|---------------------|
| M1 | C# reference codec (archived) | 594 tests, 30 golden vectors | WHEN the C# suite runs THEN all tests pass |
| M2 | Rust core port | 8 decoders, 53 profiles | WHEN a known profile decodes THEN output matches C# |
| M3 | CLI + sample generator | ithmb and ithmb-gen binaries | WHEN the CLI decodes a file THEN a PNG is written |
| M4 | PhotoDB/ArtworkDB support | read, write, integrity | WHEN a PhotoDB opens THEN entries extract |
| M5 | SIMD acceleration | SSE2/AVX2/NEON dispatch | WHEN YUV decoders run THEN 2-5x throughput vs scalar |
| M6 | Python + WASM bindings | PyO3 and wasm-bindgen modules | WHEN bindings load THEN decode works from Python/JS |
| M7 | Fuzz + hardening | 6 fuzz targets, Miri | WHEN fuzzing runs THEN 1.2M+ iterations, 0 crashes |
| M8 | CI pipeline | pr-checks, ci-full, release | WHEN a commit lands THEN all gates pass |
| M9 | Release pipeline | 5-target cross-compile + wheels | WHEN a v* tag is pushed THEN a Release is created |

Circuit breaker: IF a decoder change regresses a golden vector or the benchmark
regression gate (25% threshold) fails THEN the change SHALL be reverted before
further work.
Contingency: if a planned ROADMAP item exceeds its appetite, it is deferred to
the next release rather than shipped half-done.
```

**MESO/MICRO:** The core library ships in every milestone; wrappers ship independently. Quality level is production across the board: every error state is tested, fuzzed, and documented. Future milestones are tracked in ROADMAP.md with explicit deferral rules.

---

## 8. Testing Strategy (Tier 2)

### MACRO: Test Philosophy

```
Unit coverage target: every public decoder/parser entry point has a test; 570 unit tests
  across 17 suites (docs/STATS.md). Line coverage is measured locally via cargo llvm-cov
  and is not enforced in CI; the enforced floor is the per-module test tables below.
Integration scope: module boundaries (pipeline dispatch, profile resolution, PhotoDB parsing, C-ABI)
E2E coverage: CLI end-to-end decode, WASM runtime smoke tests, Python module tests
Framework: built-in #[test] + divan benches + proptest + cargo-fuzz (libfuzzer) + Miri
```

**MESO: Per-Component Test Requirements:**

| Module | Test Type | Target | Notes |
|--------|-----------|--------|-------|
| ithmb-core decoders | unit + roundtrip | RGB565 65,536 values, RGB555 32,768, CL 15,625 | exhaustive per-format roundtrip |
| pipeline | unit + fuzz | fuzz_decode_ithmb, fuzz_decode_pipeline | dispatch and transform paths |
| photodb parser | unit + fuzz | fuzz_parse_photodb | chunk tree parsing |
| profile_db / profile_parser | unit + fuzz | fuzz_parse_profile | JSON parsing and resolution |
| SIMD kernels | Miri | 21 SSE2 tests, zero UB | unsafe code verification |
| encoders | unit + fuzz | fuzz_encode_roundtrip | encode/decode roundtrip |
| CLI | integration | tests/cli.rs | end-to-end binary behavior |
| WASM | runtime smoke | tests/wasm_runtime.rs | wasm-pack test |
| C ABI | integration | tests/c_api_test.rs | FFI contract, nm symbol check |
| concurrency | stress | 11 tests | Barrier sync, cancellation, repeated decode |
| golden vectors | golden | 14 reference files | C# reference parity |

**MICRO:** One behavior per test. Test names describe the expected outcome (test_decode_unknown_format_returns_error). Edge cases are written before happy path. Regression tests lock bugs: when a bug is found, the first action is a test that reproduces it. Tests anchor to features via F-### IDs (docs/FEATURES.md).

---

## 9. Operational Resilience (Tier 2)

### MACRO: Resilience Strategy

```
Error tracking: typed DecodeError enum with structured fields; every failure
  carries the values that caused it (expected/actual sizes, limits, prefixes).
Fallback behavior: unknown prefixes trigger a data-size heuristic, then byte-level
  JPEG carving, then a typed Unsupported error. Decoder failures try fallback
  encodings before returning the primary error.
Recovery mechanism: decode is pure and stateless; a failed decode leaves no
  partial state. Callers retry with a fresh buffer. Cancellation via AtomicBool
  returns Canceled at the next checkpoint.
Load handling: the 8 MB file size guard prevents OOM from pathological input;
  the LRU cache (feature) bounds repeated-access memory; cancellation checkpoints
  bound worst-case scan time.
```

**MESO/MICRO:** Errors propagate as typed DecodeError across module boundaries; the CLI converts them to messages, the Python module to exceptions, the WASM module to JS strings. No module swallows an error silently. The `logging` feature emits debug/info/warn at the pipeline dispatch points for diagnosis.

---

## 10. Build & Release Pipeline (Tier 2)

### MACRO: Release Strategy

```
Versioning scheme: Semantic Versioning (workspace version 1.9.9; crates share the workspace version)
Release cadence: tag-based (v* tags trigger release.yml), milestone-driven
Changelog: git-cliff generated from conventional commits (cliff.toml), committed as CHANGELOG.md
```

**MESO/MICRO:** The CLI cross-compiles for 5 targets (Linux x64/ARM64, macOS x64/ARM64, Windows x64) and Python wheels build for the same 5 targets via maturin. The WASM package builds via wasm-pack. The C ABI plugin ships from its own repository. Release artifacts are uploaded to a GitHub Release with generated notes. All binaries are built with SIMD acceleration enabled.

### MESO: Distribution Surfaces (enumeration required)

_Every product surfaces to its audience through a set of distribution surfaces. Enumerate ALL of them and when each is built._

| Surface | Built when? | Purpose / audience |
| --- | --- | --- |
| Website / landing | Shipped (ithmb-codec.dev) | Converts visitors, hosts the web decoder |
| Web-app / demo | Shipped (ithmb-codec.dev/ithmb-decoder) | Browser-based drag-drop decode, zero install |
| Docs site | Shipped (docs.rs/ithmb-core + docs/) | API reference, guides, standards |
| CLI | Shipped (cargo install ithmb-cli) | Power users, scripting, batch processing |
| WASM demo | Shipped (crates/ithmb-wasm) | Browser reach, zero-install evaluation |
| C ABI / bindings | Shipped (feature "c" + plugin repo) | Language interoperability (ImageGlass, FFI) |
| Python bindings | Shipped (pymod, PyPI via maturin) | ML pipelines, scripting |

Distribution surfaces differ by money tier: **library** = docs + examples + web demo; **platform** = hosted service + dashboard + status page. This project is a library-tier product; the web decoder is the adoption engine and was planned alongside the codec, not retrofitted.

---

## 11. Design for Change (Tier 2)

Intent: how does this project make goalpost shifts cheap instead of expensive? The rules below are the realized mechanisms, not aspirations.

| Rule | Applied? | How |
| --- | --- | --- |
| Interface Rule (no interface before 2nd consumer) | Yes | The decoder contract stabilized only after 8 decoders existed; the C ABI surface (c_api.rs, feature "c") is the stable interface for external consumers and is verified by nm symbol checks in CI |
| Test Rule (contract over implementation) | Yes | Golden vectors (14) and exhaustive roundtrip tests lock decoder contracts; proptest covers properties; tests anchor to features via docs/FEATURES.md Test Anchoring tables |
| Module Boundary (single entry point) | Yes | pipeline/ is the single decode entry; CLI, WASM, Python, and C wrappers call only the public entry points, never decoder internals |
| Size Rule (250/40 LOC limits) | Yes | 250 LOC ceiling per module enforced by review (rust-workflow rule 10); pipeline/mod.rs is split into open.rs and profile_loader.rs submodules |
| Cycle Rule (shippable per cycle) | Yes | Milestone-based releases; every cycle ends with green CI (pr-checks + ci-full) and a signed v* tag |
| Appetite Rule (time before scope) | Yes | ROADMAP v1.10/v2.0/v2.1 with explicit deferral rules; circuit breaker on golden-vector or benchmark regression |
| AI Rule (same structural checks) | Yes | AI-generated code passes the same fmt/clippy/deny/audit/doc gates; 5 review rounds fixed ~47 findings (README Quality Assurance) |
| Rule of Three (extract on 3rd) | Yes | SIMD kernels extracted per-format after 3+ YUV formats shared conversion; enc/helpers.rs shared across 7 encoders |
| Dependency Rule (core != infra) | Yes | ithmb-core imports zero edge crates; infra (cache, metrics, logging) is feature-gated and out of the default build |
| Clean Backlog (no perpetual) | Yes | ROADMAP items re-compete per release; deferred items are versioned (v2.0, v2.1), not perpetual |

---

## 12. Documentation Strategy (Tier 3)

### MACRO: Documentation Plan

```
README: overview, quick start, how-it-works, architecture, CLI tool, benchmarks,
  limitations, troubleshooting, development, quality assurance, license
API docs: rustdoc (cargo doc --no-deps --workspace with RUSTDOCFLAGS="-D warnings" in CI),
  published at docs.rs/ithmb-core
Tutorials: docs/guides/GUIDE.md (photo recovery walkthrough), docs/what-is-this.md
  (plain-english explainer), docs/GLOSSARY.md (term definitions)
Examples: ithmb-gen sample generator, samples/ directory, CLI examples in README
Migration guides: docs/EVOLUTION.md (C# to Rust migration), docs/RELEASING.md (release process)
```

**MESO/MICRO:** Public API doc comments are required (`#![warn(missing_docs)]` in ithmb-core). Architecture decisions live in docs/adr/ (8 ADRs). Format details in docs/FORMAT.md, profile tables in docs/PROFILES.md, known divergences in docs/divergence-catalog.md. Markdown for all docs; Conventional Commits enforced by .commitlintrc.json.

---

## 13. Ecosystem & Community (Tier 3)

### MACRO: Governance

```
License: MIT (SPDX: MIT)
Contribution model: PR-based, single maintainer (B67687); contribution breakdown in README
Code of conduct: not yet adopted (single maintainer); SECURITY.md defines the security reporting path
Plugin API: C ABI (feature "c") + ImageGlass plugin in a separate repository; WASM and Python bindings
Standards compliance: Conventional Commits, editorconfig, typos, cargo-deny license allowlist,
  crates.io-only source registry, clean-room MIT (no GPL code from ImageGlass PR #2316)
```

**MESO/MICRO:** Upstream references: iOpenPod (profile validation), libgpod (PhotoDB chunk parser), Keith's iPod Photo Reader (multi-frame), clickwheel and OrgZ (C# ArtworkDB), pyithmb (Python YUV reference). Full list in docs/ACKNOWLEDGMENTS.md (33 projects). Downstream: ImageGlass-Ithmb-Plugin, ithmb-codec.dev web decoder, crates.io and PyPI consumers. PR requirements: CI green, tests pass, Conventional Commit message.

---

## 14. AI Attribution & Transparency (Tier 3)

### MACRO: Policy

```
Disclosure level: Full
Rationale: built with heavy AI assistance (OpenCode + OMO harness); full transparency
  builds trust with users and contributors (README badges + docs/CREDITS.md)
```

**MESO: Tool Inventory:**

| Tool | Version | Permitted Uses | Citation Format |
|------|---------|----------------|-----------------|
| OpenCode | current | Code generation, testing, documentation, CI | README badge + docs/CREDITS.md |
| OMO (Oh My OpenAgent) | current | Code generation, review, refactoring | README badge + docs/CREDITS.md |
| rustfmt / clippy | toolchain 1.88.0 | Formatting, linting | CI gate |
| cargo-deny / cargo-audit | 0.20.2 / 0.22.2 | License and advisory checks | CI gate |
| cargo-fuzz (libfuzzer) | 0.13.2 | Fuzz testing | CI gate |
| Miri | toolchain | Unsafe-code verification | CI gate |
| typos / lychee / gitleaks | 1.42.3 / v2 / v3.0.0 | Typo, link, and secret scanning | CI gate |
| git-cliff | current | Changelog generation | release.yml |

**MICRO:** docs/CREDITS.md lists AI tools; README carries the "Built with AI assistance" badge. Commit metadata carries no AI attribution trailers; the author field is the sole attribution.

---

## 15. Verification Checklist (Executor Reads Before Starting)

- [x] All `{{placeholders}}` across all sections are filled (none remain)
- [x] No "TODO" or "TBD" remains
- [x] Constitution (section 0) has at least 3 principles (8 present)
- [x] Out-of-scope list (section 1) is non-empty (7 items)
- [x] Each architecture decision (section 2) includes a Y-Statement (7 decisions)
- [x] Each dependency (section 5) has a version constraint
- [x] Timeline (section 7) has a circuit breaker condition
- [x] Tier 1 sections 0-7 are fully filled
- [x] Tier 2 sections 8-11 are filled for production projects
- [x] Tier 3 sections 12-14 are filled for open-source projects
- [x] **FEATURES.md** exists (docs/FEATURES.md): every IN SCOPE item is an `approved`/`applied` entry; every `applied` feature has linked tests; statuses are valid (proposed/approved/applied/archived)
- [x] **Test anchoring**: every feature in docs/FEATURES.md has a Test Anchoring table naming its tests; per-test F-### tags are the forward convention (existing tests predate the inventory and anchor at the feature level via the tables)

For engineering deliverables, also verified:
- [x] Quality gates (section 4) have concrete CI commands
- [x] Fuzz targets exist in `fuzz/` (6 targets, run in ci-full.yml)
- [x] Benchmark suite exists (divan benches in ithmb-core/benches, 6 benches)
- [x] cargo-deny / deny.toml exists (license allowlist, bans, source registry)
- [x] Multi-platform CI matrix configured (3-OS build matrix + 5-target release)
- [x] Test-to-source ratio meets the 0.5x minimum (570 unit tests across 21 source modules)

---

## Origin

Completed August 2026 as part of the REVIEW gate (Godel Gate) governance backfill. Sections 0-10 record the realized contracts of the AS-BUILT codebase at HEAD aae56f4; sections 11-15 complete the 16-section schema. Grounded in the actual workspace: Cargo.toml, crates/, CI workflows, deny.toml, README, ARCHITECTURE, ROADMAP, and docs/STATS.md.