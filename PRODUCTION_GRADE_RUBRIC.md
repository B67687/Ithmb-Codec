# Production-Grade Assessment Rubric — Ithmb Codec (Rust)

This document defines the 8-axis maturity rubric used to evaluate the Ithmb Codec
Rust workspace.  Every PR, release, or major refactor SHOULD self-assess against
it so we never regenerate the framework from scratch.

## Scoring

Each criterion is scored **0 / 1 / 2**:

| Score | Meaning |
|-------|---------|
| **0** | Fails — known issue, no plan |
| **1** | Partial — exists but has gaps or is incomplete |
| **2** | Passes — meets the bar with evidence |

The **category score** is the sum of its criterion scores divided by the maximum
possible, expressed as a percentage.  The **overall score** is the arithmetic
mean of all eight category percentages.

---

## Axis 1 — Structural Integrity (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 1.1 | Module single-responsibility | Every file owns one domain noun | ≤2 files own >1 domain | Multiple god-classes |
| 1.2 | File size ≤250 pure LOC | All files under limit | 1–3 files exceed | 4+ files exceed |
| 1.3 | Dependency hygiene | Minimal deps, all pinned, Cargo.lock committed | Some unpinned, no SBOM | Unnecessary or risky deps |
| 1.4 | Cyclic dependency check | No cycles, proven by compiler module system | No cycles proven by tool | Undocumented or cycles present |
| 1.5 | Module-split discipline | Logical split, no cross-file coupling | Some cross-file coupling | Chaotic split |

**Structural Integrity Score =** (sum of 1.1–1.5) / 10
**Current: 90% (9/10)** — 1.1/1.3/1.5 at 2/2 (21 modules, each decoder in own file, PhotoDB in submodule). 1.2 at 1/2 — `src/simd.rs` is 2038 SLOC (SIMD intrinsics with ISA-gated blocks; SIZE_OK rationale applies but technically exceeds threshold). 1.4 at 2/2 — Rust module system enforces no cycles at compile time.

---

## Axis 2 — Code Quality (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 2.1 | Type safety | No escape hatches (`unsafe` denied at workspace level) | 1–3 justified escapes | Broad suppression |
| 2.2 | Error handling | `thiserror` enum catches all, no empty blocks | Majority catch, some gaps | Bare catch or empty blocks |
| 2.3 | Edge-case coverage | Known edges guarded (NUL, bounds, zero, 32 MB guard) | Major edges covered | Reactive only |
| 2.4 | Locale/explicit culture | All string ops invariant or explicit | Most ops safe, 1–2 gaps | Turkish-i style bugs |
| 2.5 | Code-smell discipline | No negative naming, >3 params, redundant verify | ≤2 smells | 3+ or systematic |

**Code Quality Score =** (sum of 2.1–2.5) / 10
**Current: 100% (10/10)** — All five criteria at 2/2. `unsafe_code = "deny"` at workspace level (only `cabi/` lifts it per-target for FFI). `thiserror` enum with typed variants (`Io`, `Jpeg`, `InvalidFormat`, `Unsupported`, `BufferTooShort`, `Profile`, `Canceled`). NUL guard in path handling, 32 MB file-size guard in pipeline. No locale-dependent string operations in codec logic. No negative naming or parameter bloat found in audit.

---

## Axis 3 — Performance (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 3.1 | SIMD coverage | All hot YUV paths have SIMD (SSE2+AVX2+NEON runtime dispatch). RGB565/RGB555 use auto-vectorized scalar (hand-written SIMD was 34× slower via AVX-512). | SSE2+NEON only, or feature-gated | Scalar-only paths in hot loop |
| 3.2 | Memory discipline | Vec reuse, LRU cache, no per-frame allocs | Mostly pooled, 1 alloc risk | Per-frame alloc in hot path |
| 3.3 | Zero-alloc hot path | Raw decode path has zero managed allocations (pre-allocated output buffer) | Once-per-decode alloc (JPEG slice) | Alloc per frame/pixel |
| 3.4 | Benchmark regression gate | CI compares artifact, fails on >10% regression | CI runs benchmarks, no gate | No benchmark CI |
| 3.5 | Release build optimization | `[profile.release]` with LTO + codegen-units = 1 configured | LTO considered | Not configured |

**Performance Score =** (sum of 3.1–3.5) / 10
**Current: 50% (5/10)** — 3.2 at 2/2 (Vec reuse, LRU cache behind `cache` feature, no per-frame allocs in raw decoders). 3.3 at 1/2 — raw decoders are zero-alloc (caller-provided output buffer), but JPEG decode allocates through `jpeg-decoder` crate. 3.1 at 1/2 — SIMD (SSE2, AVX2, NEON) covers UYVY, YCbCr420, CL, CLCL YUV paths only, behind `--features simd` flag; RGB565/RGB555 use auto-vectorized scalar (AVX-512 was removed — caused 34× slowdown from frequency downclock + port-5 bottleneck). 3.4 at 1/2 — `cargo bench` runs but no automated regression comparison against baselines. 3.5 at 0/2 — no `[profile.release]` with LTO/codegen-units configured in `Cargo.toml`.

**Benchmarks (divan, 256×256, this machine):**

| Decoder | Throughput |
|---------|-----------|
| RGB565 | 7.5 µs, 35 GB/s |
| RGB555 | 7.7 µs, 34 GB/s |
| ReorderedRGB555 | 106 µs, 2.5 GB/s |
| UYVY (YUV422) | 17 µs, 15 GB/s |
| YCbCr420 | 38 µs, 6.9 GB/s |
| CL | 49 µs, 5.3 GB/s |
| CLCL | 2.9 µs, 90 GB/s |
| JPEG (64×64) | 54 µs, 301 MB/s |

---

## Axis 4 — Security (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 4.1 | Input validation at boundary | Every untrusted source validated (NUL, size, bounds, 32 MB guard) | Major paths validated | Minimal validation |
| 4.2 | Supply-chain integrity | All GH actions SHA-pinned, Cargo.lock committed, `cargo deny` in CI | Pinned, Cargo.lock, no deny/audit in CI | Unpinned actions |
| 4.3 | SAST in CI | Clippy pedantic=deny + cargo audit + cargo deny in CI | Clippy only, no audit/deny in CI | None |
| 4.4 | Secret scanning | gitleaks or trufflehog in CI | Manual scanning | None |
| 4.5 | Profiles integrity | External profiles verified by hash before use | CRC logged but not verified | Loaded with trust |

**Security Score =** (sum of 4.1–4.5) / 10
**Current: 50% (5/10)** — 4.1 at 2/2 (NUL guard, 32 MB file-size guard, buffer-too-small guards in all decoders, frame-index bounds check). 4.2 at 1/2 — GH actions SHA-pinned (`actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0`), `Cargo.lock` committed, but `cargo deny` and `cargo audit` not wired into CI. 4.3 at 1/2 — clippy with `pedantic = "deny"` enforced, but no dependency audit in CI. 4.4 at 0/2 — no secret scanning. 4.5 at 1/2 — `profiles.json` parsed at init with full validation, but not verified against a trusted hash.

---

## Axis 5 — Testing (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 5.1 | Unit coverage | ≥85% line rate, every public function tested | ≥70% line rate | <70% or no gate |
| 5.2 | Integration tests | Real roundtrip (encode → decode → match), golden vectors, synthetic vectors | Partial roundtrip | Happy-path only |
| 5.3 | Stress/concurrency tests | Concurrent read/write, cancellation, race detection | Some concurrency tests | None |
| 5.4 | Fuzz / property-based | 2 libfuzzer targets, 1.2M+ iterations, proptest for decoders | Fuzz for one parser | None |
| 5.5 | Regression suite runtime | <30 s | <60 s | >60 s |

**Testing Score =** (sum of 5.1–5.5) / 10
**Current: 90% (9/10)** — 5.2–5.5 all at 2/2 (roundtrip across all 8 decoders + encoders; 11 concurrency stress tests with Barrier sync + cancellation; 2 libfuzzer targets with 1.2M+ iterations at 0 crashes; 0.45s runtime for 489 tests across 12 suites). 5.1 at 1/2 — coverage data collected by `cargo-llvm-cov` (report published) but no minimum rate confirmed; SIMD paths are unreachable on x64 CI without `--features simd`.

---

## Axis 6 — CI/CD (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 6.1 | Format gate | `cargo fmt --check` enforced | Exists but not enforced | None |
| 6.2 | Build gate | 0 errors, 0 warnings (`pedantic = "deny"` in Cargo.toml) | 0 errors, warnings tolerated | Errors in CI |
| 6.3 | Test gate | All tests pass on every push | Most pass, known failures tolerated | No test stage |
| 6.4 | Coverage gate | ≥85% enforced, report published | ≥70% enforced | No coverage check |
| 6.5 | Release validation | Tag pattern check, CHANGELOG diff, crates.io publish action | Tag check only | Manual release |

**CI/CD Score =** (sum of 6.1–6.5) / 10
**Current: 70% (7/10)** — 6.1–6.3 at 2/2 (fmt, clippy pedantic=deny, all 489 tests pass). 6.4 at 1/2 — `cargo-llvm-cov` runs in CI and publishes report, but no minimum threshold enforces the gate. 6.5 at 0/2 — no crates.io publishing yet (git deps only), no tag-pattern verification in CI, no signed tag enforcement.

**CI pipeline** (`.github/workflows/rust-ci.yml`):
- Build (`cargo build --workspace`)
- Test (`cargo test --workspace`)
- Clippy (`cargo clippy --workspace -- -D warnings`)
- Format check (`cargo fmt --check`)
- cabi cdylib build + symbol export verification (`nm -D | grep ig_plugin_get_api`)
- Also runs: `--features simd` test matrix, aarch64 cross-compilation, `cargo audit`, `cargo-llvm-cov`, `cargo fuzz build`

---

## Axis 7 — Documentation (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 7.1 | README | Stats gate-verified, architecture diagram, getting-started | Comprehensive but not gate-verified | Minimal or stale |
| 7.2 | CHANGELOG | `[Unreleased]` kept current, categorized, no duplication | Exists, occasionally stale | None |
| 7.3 | Profiles documentation | PROFILES.md reflects actual code, 54 entries, complete | Mostly accurate | Not maintained |
| 7.4 | Rustdoc comments | All public API documented | Key methods documented | None |
| 7.5 | Architecture decision records | docs/adr/ with rationale for major design choices | Inline comments only | No rationale |

**Documentation Score =** (sum of 7.1–7.5) / 10
**Current: 90% (9/10)** — 7.1–7.3 at 2/2 (README stats gate-verified via `tools/check-readme-stats.sh`, architecture diagram in docs/, getting-started with build instructions). CHANGELOG covers both C# history and Rust 0.3.0 entries, `[Unreleased]` maintained. PROFILES.md reflects 54 entries accurately. 7.4 at 1/2 — core public API (decoders, pipeline, profile) has rustdoc; some modules (enc_helpers, photodb types) lack comprehensive doc comments. 7.5 at 2/2 — `docs/adr/` has 4 records covering AOT plugin boundary, SIMD dispatch, profile discovery, and quarterly audit protocol.

---

## Axis 8 — Observability (weight: low)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 8.1 | Structured logging | Consistent format, correlation tokens, log levels | Logs exist, no correlation | None |
| 8.2 | Metrics | Decode count, latency, error rate counters | One counter | No counters |
| 8.3 | Tracing | Correlation ID threaded through full pipeline | Activity ID at entry point | No tracing |
| 8.4 | Error telemetry | Stack trace + context captured for failures (typed DecodeError) | Message logged | Silent failures |

**Observability Score =** (sum of 8.1–8.4) / 8
**Current: 62.5% (5/8)** — 8.2 at 2/2 (7 per-format atomic counters behind `metrics` feature: decode count, nanoseconds per format). 8.4 at 2/2 (typed `DecodeError` with structured context, never silent failures). 8.1 at 1/2 — `ITHMB|component|EVENT|filename|details` convention in CLI, logs exist but no structured correlation IDs. 8.3 at 0/2 — no tracing or propagation system; cancellation is via AtomicBool, not via a trace context.

---

## Overall Score

```
Overall = (Axis1% + Axis2% + Axis3% + Axis4% + Axis5% + Axis6% + Axis7% + Axis8%) / 8
```

**Overall Score: 75.3% — Maturing** (70–84% band)

| Range | Rating | Meaning |
|-------|--------|---------|
| ≥95% | Elite | Benchmark-quality, few conceivable improvements |
| 85–94% | Production-grade | Safe to ship, targeted follow-up |
| 70–84% | Maturing | Ship with acknowledged debt, active remediation |
| 50–69% | Brittle | Ship only with high-urgency justification |
| <50% | Pre-production | Do not ship |

### Axis breakdown

| Axis | Score | Key strength | Key gap |
|------|-------|-------------|---------|
| 1. Structural Integrity | 90% | Module split by decoder, no cycles | simd.rs exceeds 250 SLOC |
| 2. Code Quality | 100% | `unsafe_code=deny`, typed errors | — |
| 3. Performance | 50% | LRU cache, zero-alloc raw decoders | No LTO, SIMD feature-gated, no bench regression gate |
| 4. Security | 50% | NUL guard, 32 MB limit | No gitleaks, no cargo audit in CI |
| 5. Testing | 90% | 489 tests, fuzz 1.2M, 0.45s | Coverage rate unconfirmed |
| 6. CI/CD | 70% | fmt+clippy+test all enforced | No coverage gate, no crates.io publish |
| 7. Documentation | 90% | README gate-verified, ADRs | Some modules lack rustdoc |
| 8. Observability | 62.5% | Per-format metrics, typed errors | No tracing, no correlation IDs |

### Priority remediation

| Priority | Item | Axis | Impact |
|----------|------|------|--------|
| P0 | Publish ithmb-core to crates.io | 6 | Unblocks downstream consumption |
| P0 | Add `[profile.release]` with LTO + codegen-units=1 | 3 | Free performance gain, ~15–20% decode speedup |
| P1 | Wire `cargo audit` / `cargo deny` into rust-ci.yml | 4 | Supply-chain vulnerability visibility |
| P1 | Add benchmark regression comparison to CI | 3 | Prevents silent performance regressions |
| P2 | Add coverage threshold gate (≥70%) to CI | 6 | Prevents coverage drift |
| P2 | Add `simd` feature to default | 3 | SIMD active for all users (YUV paths) |
| P3 | Add gitleaks or trufflehog to CI | 4 | Secret leak prevention |
| P3 | Add tracing instrumentation | 8 | Debugging production decode failures |

---

## Self-Assessment Checklist (for PRs)

Before marking a PR ready-for-review, the author SHOULD run through these
questions derived from the rubric:

- [ ] Does my change introduce a new file over 250 pure LOC?
- [ ] Does my change add or widen any escape hatch (`unsafe` without SAFETY comment, `#[allow]` silencing a real warning)?
- [ ] Does my change touch a hot decode path — does it have SIMD coverage (SSE2/AVX2/NEON for YUV, auto-vec for RGB)?
- [ ] Does my change create a per-frame allocation in a hot decode path?
- [ ] Does my change accept external input — is it validated at the boundary?
- [ ] Does my change add new behavior — is there a failing test that proves it was needed?
- [ ] Does my change introduce a new dependency — is it pinned in `Cargo.toml` and updated in `Cargo.lock`?
- [ ] Are public API methods documented with `///` rustdoc comments?
- [ ] Is the CHANGELOG updated under `[Unreleased]`?
- [ ] Do all existing tests still pass (`cargo test --workspace`)?
- [ ] Does the CLI still build (`cargo build -p ithmb-cli`)?
