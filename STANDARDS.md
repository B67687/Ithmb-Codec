# Ithmb-Codec Engineering Standards

This file documents which automation and design standards apply to this specific project.
It is the applied version of the universal standards in the project-retrospective-methodology repo.

**Universal reference**: `github.com/B67687/project-retrospective-methodology`

---

## Automation Standards Applied

### Tier 0 — Day 1 (present at project creation)

| Item | Status | How |
|------|--------|-----|
| CI build + test | ✅ | `.github/workflows/build-linux.yml` — Release + Debug, push/PR to main |
| Static analysis (warnings-as-errors) | ✅ | `AnalysisLevel=latest-recommended`, `TreatWarningsAsErrors=true` in csproj |
| Dependency vulnerability scanning | ✅ | `dotnet list --vulnerable --include-transitive` in CI + Dependabot |
| Secret scanning | ✅ | `gitleaks/gitleaks-action@v3` on every push/PR |
| Signed commits | ✅ | `commit.gpgsign=true` (246/247 commits signed) |
| Reproducible builds | ✅ | `RestorePackagesWithLockFile=true` + SHA-pinned GitHub Actions |
| CHANGELOG | ✅ | Keep a Changelog format, `[Unreleased]` header present |
| README skeleton | ✅ | What, Why, How, Status, People sections present |

### Tier 1 — Within 10 Commits (present by v1.5.0)

| Item | Status | How |
|------|--------|-----|
| Conventional commits | ✅ | All commits follow Conventional Commits. Enforced by commitlint. |
|| CHANGELOG presence CI check | ✅ | `git diff | grep CHANGELOG` check in build-linux.yml |
| Stats gate (derive from source) | ✅ | `tools/check-readme-stats.sh` verifies profile count + test count in CI |
| Build provenance | ✅ | `AssemblyMetadata("CommitSha")` + `BuildTimestamp` embedded at compile time |
| Code coverage gate | ✅ | 72% minimum in CI (adjusted from 75%/73% due to PGO instability) |
| Formatter enforcement | ✅ | `dotnet format --verify-no-changes` in CI |
| EditorConfig | ✅ | `.editorconfig` with LF, UTF-8, indent 4/2, trim trailing whitespace |
| SDK/toolchain pinning | ✅ | `global.json` pins .NET 10.0.x, `rollForward: latestFeature` |
|| Signed release tags | ✅ | Tag signature validation in build-linux.yml. v1.6.0 tag pushed signed. |
| Concurrency-safe state | ✅ | Retrofitted in v1.6.0 (Lock, Interlocked, ConcurrentDictionary) |

### Tier 2 — Within First Release (present by v1.6.0)

| Item | Status | How |
|------|--------|-----|
| Performance regression gate | ✅ | `tools/check-benchmark-regression.sh` + `benchmark.yml` (manual dispatch) |
|| Scheduled fuzz testing | ✅ | Weekly CI in `.github/workflows/fuzz-weekly.yml` (Mon 06:00 UTC). Extended iterations for all fuzz test suites. |
| Production-grade rubric | ✅ | `PRODUCTION_GRADE_RUBRIC.md` — 8-axis, scored 86.6% baseline |
| Scheduled adversarial audit | ❌ **Missing** | The v1.5.0 and v1.6.0 audits were manual. No quarterly schedule. |
| Release artifact automation | ✅ | `.github/workflows/release-windows.yml` (tag → build → zip → upload) |
| Correlation tokens in logs | ✅ | `ITHMB\|component\|EVENT\|filename\|details` convention |
|| File size gate (250 LOC) | ✅ | CI gate in build-linux.yml via `tools/check-file-sizes.sh`. 4 files exempted with SIZE_OK comments. |
| Test quality gate | ✅ | Tautological assertions removed in v1.5.0 audit. Every test asserts behavior. |

### Tier 3 — Quality of Life

| Item | Status | How |
|------|--------|-----|
|| Design decision records | ✅ | `docs/adr/0001` (Native AOT), `0002` (SIMD dispatch), `0003` (profile resolution) |
|| Commit date alias | ✅ | `tools/git-commit-dated.sh` — preserves author+committer dates |
| Release notes from CHANGELOG | ⚠️ Manual | Notes are hand-crafted per release |
|| PR template | ✅ | `.github/PULL_REQUEST_TEMPLATE.md` with checklist |
|| Pre-commit hooks | ✅ | `.pre-commit-config.yaml` — trailing-whitespace, EOF-fixer, YAML/JSON lint, conventional commit check |
|| Multi-architecture CI | ✅ | x64 (build-linux.yml), ARM64 (test-neon.yml), macOS ARM64 (build-macos.yml) |

---

## Design Standards Applied

This project follows the design hierarchy from `DESIGN_STANDARDS_HIERARCHY.md`.

### Axioms in practice

| Axiom | How Ithmb-Codec applies it |
|-------|---------------------------|
| **A1 Modularity** | 22 domain-partial files. Each decoder in its own file. 6 files extracted from god-classes in P5 refactoring. |
| **A2 Data Flow Direction** | Plugin ABI forces unidirectional flow: `IG_PluginGetApi` → `DecodePipeline` → per-format decoder → BGRA output. No back-edges. |
| **A3 Fail-Fast** | NUL-in-path guard. 32 MB file size guard. Frame index bounds check. Array length checks before SIMD processing. |
| **A4 Explicit Over Implicit** | Every decode logged with `ITHMB\|...` tokens. ArrayPool rent/return explicit. Lock scopes minimal. |
| **A5 Parse-Don't-Validate** | Embedded JSON profiles parsed at init into `FrozenDictionary`. Parse-time validation rejects malformed entries. |
| **A6 Layered Dependencies** | `PhotoDb/` → `IthmbCodecPlugin.*` → ImageGlass ABI. No module cycles. |

### Meso contracts in practice

| Contract | How Ithmb-Codec applies it |
|----------|---------------------------|
| **M1 Interface Surface** | Public API = `GetApi()`. Everything else is `internal` or `private`. Plugin ABI enforces minimal surface. |
| **M2 State Management** | `System.Threading.Lock` for cache, `ConcurrentDictionary` for live buffers, `Interlocked` for stats. |
| **M3 Resource Lifecycle** | `NativeMemory.Alloc/Free` with try/finally. ArrayPool rent/return in same method. |
| **M4 Error Domains** | Return `BGRA_ERR` codes (not exceptions). Log at failure point. No empty catch blocks. |
| **M5 Module Boundaries** | Decoders live in separate files, share only `IthmbCodecPlugin` namespace + `Helpers` utilities. |

### Micro rules in practice

| Domain | How Ithmb-Codec applies it |
|--------|---------------------------|
| **Naming** | `DecodeRawProfile`, `TryFindJpegSlice`, `IsPadded` — intent-revealing. No abbreviations except RGB/YUV. |
| **Branching** | Guard clauses for fail-fast (bounds checks). SIMD dispatch ladder (AVX-512 → SSE2 → NEON → scalar). |
| **Functions** | Most <40 lines. Exceptions: SIMD parameterized loops (ISA duplication inflates count). |
| **Concurrency** | No parameter mutation. Minimal lock scope. Interlocked for single-variable state. |

---

## Current Gaps (highest priority to close)

|| Gap | Effort | Impact | Why it matters |
||-----|--------|--------|---------------|
|| Quarterly audit reminder | 1 calendar entry | Catch logic bugs | 28 bugs found in single manual pass |

### Ithmb-Specific Gaps (not covered by universal tiers)

| Gap | Effort | Impact | Why it matters |
|-----|--------|--------|---------------|
|| Profile source diff script | 2h | Prevent profile drift | Formats discovered from 22 sources; no automatic diff against them |
|| NEON coverage collection | 1h | Accurate ARM64 coverage | 5% uncovered on x64 is NEON paths; never actually measured on ARM64 |
|| Profile integrity verification | 30m | Supply chain trust | FNV-1a hash logged but not verified against embedded expected hash |
|| README architecture table | 15m | Doc accuracy | Table still missing 6 extracted files |
|| Release notes from CHANGELOG | 30m | Clean releases | Notes still hand-crafted per release |
|| Benchmark Native AOT comparison | 2h | Accurate perf numbers | Benchmark runs JIT; production runs Native AOT with PGO |
|| Debug coverage collection | 15m | Complete coverage picture | Coverage only collected in Release mode |

---

## Version

This file is versioned with the project. Update when automation or design standards change.

| Version | Date | Changes |
|---------|------|---------|
|| 1.0 | 2026-06-30 | Initial: automation tiers 0-3 + design axioms applied |
|| 1.1 | 2026-06-30 | Wave 1: CHANGELOG CI check, signed tag CI, commit-date script, PR template, v1.6.0 tag |
|| 1.2 | 2026-06-30 | Wave 2+3: ADRs, file size gate, SETUP.md, macOS CI, pre-commit, fuzz CI, gaps table restructured |
