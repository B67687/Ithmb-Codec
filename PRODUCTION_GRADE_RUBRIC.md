# Production-Grade Assessment Rubric — Ithmb Codec

This document defines the 8-axis maturity rubric used to evaluate the Ithmb Codec
plugin.  Every PR, release, or major refactor SHOULD self-assess against it so we
never regenerate the framework from scratch.

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
| 1.3 | Dependency hygiene | Minimal deps, all pinned, AOT-compatible | Some unpinned, no SBOM | Unnecessary or risky deps |
| 1.4 | Cyclic dependency check | No cycles, documented architecture | No cycles proven by tool | Undocumented or cycles present |
| 1.5 | Partial-class discipline | Logical split, no cross-file coupling | Some cross-file coupling | Chaotic split |

**Structural Integrity Score =** (sum of 1.1–1.5) / 10
**Current: 90% (9/10)** — 1.1–1.3, 1.5 at 2/2 (P5 extraction resolved god-classes). 1.4 at 1/2 (no cyclic-dep tooling documented, though no cycles known).

---

## Axis 2 — Code Quality (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 2.1 | Type safety | No escape hatches (`as any`, `unsafe`-to-skip) | 1–3 justified escapes | Broad suppression |
| 2.2 | Error handling | Every catch logs, no empty blocks | Majority catch, some gaps | Bare catch or empty blocks |
| 2.3 | Edge-case coverage | Known edges guarded (NUL, bounds, zero) | Major edges covered | Reactive only |
| 2.4 | Locale/explicit culture | All string ops invariant or explicit | Most ops safe, 1–2 gaps | Turkish-i style bugs |
| 2.5 | Code-smell discipline | No negative naming, >3 params, redundant verify | ≤2 smells | 3+ or systematic |

**Code Quality Score =** (sum of 2.1–2.5) / 10
**Current: 100% (10/10)** — All five criteria at 2/2. NUL guard, ex.Message in all catches, explicit ASCII whitespace, no code smells detected in audit.

---

## Axis 3 — Performance (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 3.1 | SIMD coverage | All hot paths have SIMD (SSE2 + AVX-512 + NEON) | SSE2 + NEON, no AVX-512 | Scalar-only paths in hot loop |
| 3.2 | Memory discipline | Pooled buffers, LRU cache, no LOH hot allocs | Mostly pooled, 1 LOH risk | New byte[] in hot path |
| 3.3 | Zero-alloc hot path | Hot decode path has zero managed allocations | Once-per-decode alloc | Alloc per frame/pixel |
| 3.4 | Benchmark regression gate | CI compares artifact, fails on >10% regression | CI runs benchmarks, no gate | No benchmark CI |
| 3.5 | Profile-guided optimization | PGO enabled in csproj for Native AOT | PGO considered | Not configured |

**Performance Score =** (sum of 3.1–3.5) / 10
**Current: 90% (9/10)** — SIMD 2/2 (SSE2+AVX-512+NEON), memory 2/2 (ArrayPool+LRU), benchmark gate 2/2, PGO 2/2 (just enabled). 3.3 at 1/2 — unavoidable managed allocs (JPEG slice, synthetic buffers) prevent zero-alloc decode.

---

## Axis 4 — Security (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 4.1 | Input validation at boundary | Every untrusted source validated (NUL, size, bounds) | Major paths validated | Minimal validation |
| 4.2 | Supply-chain integrity | All GH actions SHA-pinned, Dependabot for all ecosystems | Pinned, Dependabot for NuGet only | Unpinned actions |
| 4.3 | SAST in CI | `dotnet list vulnerable` + CodeQL or equivalent | One of the two | None |
| 4.4 | Secret scanning | gitleaks or trufflehog in CI | Manual scanning | None |
| 4.5 | Profiles integrity | External profiles verified by hash before use | CRC logged but not verified | Loaded with trust |

**Security Score =** (sum of 4.1–4.5) / 10
**Current: 90% (9/10)** — 4.1–4.4 all at 2/2 (NUL guard, SHA-pinned actions, CodeQL+gitleaks). 4.5 at 1/2 — CRC logged on profiles.json load but not verified against a trusted hash.

---

## Axis 5 — Testing (weight: high)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 5.1 | Unit coverage | ≥85% line rate, every public function tested | ≥70% line rate | <70% or no gate |
| 5.2 | Integration tests | Real roundtrip (build → parse → match) | Partial roundtrip | Happy-path only |
| 5.3 | Stress/concurrency tests | Concurrent read/write, cancellation, race detection | Some concurrency tests | None |
| 5.4 | Fuzz / property-based | Fuzz for all parsers, property tests for decoders | Fuzz for one parser | None |
| 5.5 | Regression suite runtime | <30 s | <60 s | >60 s |

**Testing Score =** (sum of 5.1–5.5) / 10
**Current: 90% (9/10)** — 5.2–5.5 at 2/2 (real roundtrip+concurrency+fuzz+7s runtime). 5.1 at 1/2 — 75.3% coverage below 85% bar; ~5% gap is NEON paths unreachable on x64 CI.

---

## Axis 6 — CI/CD (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 6.1 | Format gate | `dotnet format --verify-no-changes` enforced | Exists but not enforced | None |
| 6.2 | Build gate | 0 errors, 0 warnings (TreatWarningsAsErrors) | 0 errors, warnings tolerated | Errors in CI |
| 6.3 | Test gate | All tests pass on every push | Most pass, known failures tolerated | No test stage |
| 6.4 | Coverage gate | ≥85% enforced, report published | ≥70% enforced | No coverage check |
| 6.5 | Release validation | Tag pattern check, CHANGELOG diff, release notes | Tag check only | Manual release |

**CI/CD Score =** (sum of 6.1–6.5) / 10
**Current: 90% (9/10)** — Format+build+test+release gates all at 2/2. 6.4 at 1/2 — 72% gate enforced but below 85% target; coverage report published.

---

## Axis 7 — Documentation (weight: medium)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 7.1 | README | Stats gate-verified, architecture diagram, getting-started | Comprehensive but not gate-verified | Minimal or stale |
| 7.2 | CHANGELOG | `[Unreleased]` kept current, categorized, no duplication | Exists, occasionally stale | None |
| 7.3 | Profiles documentation | PROFILES.md reflects actual code, complete | Mostly accurate | Not maintained |
| 7.4 | XML doc comments | All public API documented | Key methods documented | None |
| 7.5 | Architecture decision records | Rationale for major design choices | Inline comments only | No rationale |

**Documentation Score =** (sum of 7.1–7.5) / 10
**Current: 80% (8/10)** — 7.1–7.3 at 2/2 (README gate-verified, CHANGELOG current, PROFILES.md accurate). 7.4 at 1/2 — DecodePipeline+EncoderHelpers documented, but Encoding.cs and JpegDecode.cs still lack XML docs. 7.5 at 1/2 — inline rationale comments exist but no formal ADR process.

---

## Axis 8 — Observability (weight: low)

| # | Criterion | 2 (pass) | 1 (partial) | 0 (fail) |
|---|-----------|----------|-------------|----------|
| 8.1 | Structured logging | Consistent format, correlation tokens, log levels | Logs exist, no correlation | None |
| 8.2 | Metrics | Decode count, latency, error rate counters | One counter | No counters |
| 8.3 | Tracing | Correlation ID threaded through full pipeline | Activity ID at entry point | No tracing |
| 8.4 | Error telemetry | Stack trace + context captured for failures | Message logged | Silent failures |

**Observability Score =** (sum of 8.1–8.4) / 8
**Current: 62.5% (5/8)** — 8.2+8.4 at 2/2 (decode metrics, error capture). 8.1 at 1/2 — consistent format+filename tokens but no structured correlation IDs. 8.3 at 0/2 — no tracing/propagation system.

---

## Overall Score

```
Overall = (Axis1% + Axis2% + Axis3% + Axis4% + Axis5% + Axis6% + Axis7% + Axis8%) / 8
## Overall Score: **86.6% — Production-grade** (85–94% band)

| Range | Rating | Meaning |
|-------|--------|---------|
| ≥95% | Elite | Benchmark-quality, few conceivable improvements |
| 85–94% | Production-grade | Safe to ship, targeted follow-up |
| 70–84% | Maturing | Ship with acknowledged debt, active remediation |
| 50–69% | Brittle | Ship only with high-urgency justification |
| <50% | Pre-production | Do not ship |

---

## Self-Assessment Checklist (for PRs)

Before marking a PR ready-for-review, the author SHOULD run through these
questions derived from the rubric:

- [ ] Does my change introduce a new file over 250 pure LOC?
- [ ] Does my change add or widen any escape hatch (`unsafe` without SAFETY comment, `#pragma`, CA suppression)?
- [ ] Does my change touch a hot decode path — does it have SIMD coverage?
- [ ] Does my change create a new `new byte[]` in a per-frame path?
- [ ] Does my change accept external input — is it validated at the boundary?
- [ ] Does my change add new behavior — is there a failing test that proves it was needed?
- [ ] Does my change introduce a new dependency — is it SHA-pinned in CI?
- [ ] Are public API methods documented with XML doc comments?
- [ ] Is the CHANGELOG updated under `[Unreleased]`?
- [ ] Do all existing tests still pass?
