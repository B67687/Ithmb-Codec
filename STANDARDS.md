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
| CHANGELOG presence CI check | ❌ **Still missing** | No `git diff | grep CHANGELOG` step |
| Stats gate (derive from source) | ✅ | `tools/check-readme-stats.sh` verifies profile count + test count in CI |
| Build provenance | ✅ | `AssemblyMetadata("CommitSha")` + `BuildTimestamp` embedded at compile time |
| Code coverage gate | ✅ | 72% minimum in CI (adjusted from 75%/73% due to PGO instability) |
| Formatter enforcement | ✅ | `dotnet format --verify-no-changes` in CI |
| EditorConfig | ✅ | `.editorconfig` with LF, UTF-8, indent 4/2, trim trailing whitespace |
| SDK/toolchain pinning | ✅ | `global.json` pins .NET 10.0.x, `rollForward: latestFeature` |
| Signed release tags | ❌ **Missing** | Tags v1.1.0–v1.5.0 not signed. CI does not validate. |
| Concurrency-safe state | ✅ | Retrofitted in v1.6.0 (Lock, Interlocked, ConcurrentDictionary) |

### Tier 2 — Within First Release (present by v1.6.0)

| Item | Status | How |
|------|--------|-----|
| Performance regression gate | ✅ | `tools/check-benchmark-regression.sh` + `benchmark.yml` (manual dispatch) |
| Scheduled fuzz testing | ❌ **Missing** | Fuzz tests run as unit tests (deterministic). No weekly long-running job. |
| Production-grade rubric | ✅ | `PRODUCTION_GRADE_RUBRIC.md` — 8-axis, scored 86.6% baseline |
| Scheduled adversarial audit | ❌ **Missing** | The v1.5.0 and v1.6.0 audits were manual. No quarterly schedule. |
| Release artifact automation | ✅ | `.github/workflows/release-windows.yml` (tag → build → zip → upload) |
| Correlation tokens in logs | ✅ | `ITHMB\|component\|EVENT\|filename\|details` convention |
| File size gate (250 LOC) | ⚠️ Partial | 4 files exceed 250 LOC with SIZE_OK comments. No CI gate. |
| Test quality gate | ✅ | Tautological assertions removed in v1.5.0 audit. Every test asserts behavior. |

### Tier 3 — Quality of Life

| Item | Status | How |
|------|--------|-----|
| Design decision records | ❌ **Missing** | No `docs/adr/` directory. Architecture decisions are implicit. |
| Commit date alias | ❌ **Missing** | `GIT_COMMITTER_DATE` dance repeated on every commit |
| Release notes from CHANGELOG | ⚠️ Manual | Notes are hand-crafted per release |
| PR template | ❌ **Missing** | No PR template in `.github/` |
| Pre-commit hooks | ❌ **Missing** | No `.pre-commit-config.yaml` or `.husky/` |
| Multi-architecture CI | ⚠️ Partial | x64 + ARM64 covered. No macOS (osx-arm64 supported but untested) |

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

| Gap | Effort | Impact | Why it matters |
|-----|--------|--------|---------------|
| CHANGELOG presence CI check | 1 line of CI config | Prevents next forgotten changelog | We keep forgetting. Both of us. |
| Signed tag enforcement | 2 lines of CI config | Tag integrity | Tags v1.1.0–v1.5.0 are unsigned |
| Commit date alias | 1 git config | Saves ~30s per commit | Multiple rebase rounds wasted |
| v1.6.0 tag creation | 1 git command | Release traceability | Release commit is not tagged |
| Scheduled fuzz CI | 1 workflow file | Catch overflow/OOB bugs | Unit-test fuzz is deterministic |
| Quarterly audit reminder | 1 calendar entry | Catch logic bugs | 28 bugs found in single manual pass |

---

## Version

This file is versioned with the project. Update when automation or design standards change.

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-06-30 | Initial: automation tiers 0-3 + design axioms applied |
