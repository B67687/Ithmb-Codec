# Ithmb-Codec Engineering Standards

This file documents which automation and design standards apply to this specific project.
It is the applied version of the universal standards in the project-retrospective-methodology repo.

**Universal reference**: `github.com/B67687/project-retrospective-methodology`
>
> **⚠️ Retrospective**: This file documents the current state after retrospective application. These standards were **not** present from Day 1 — they were applied after the fact. See the [Version](#version) table for when each item was introduced.

---

## Automation Standards Applied

### Tier 0 — Core Infrastructure (retrospectively applied)

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

### Tier 1 — Development Discipline (retrospectively applied)

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

### Tier 2 — Production Readiness (retrospectively applied)

| Item | Status | How |
|------|--------|-----|
| Performance regression gate | ✅ | `tools/check-benchmark-regression.sh` + `benchmark.yml` (manual dispatch) |
| Scheduled fuzz testing | ✅ | Weekly CI in `.github/workflows/fuzz-weekly.yml` (Mon 06:00 UTC) |
| Production-grade rubric | ✅ | `PRODUCTION_GRADE_RUBRIC.md` — 8-axis, scored 86.6% |
| Scheduled adversarial audit | ✅ | Quarterly protocol in `docs/adr/0004`, issue template in `.github/ISSUE_TEMPLATE/` |
| Release artifact automation | ✅ | `.github/workflows/release-windows.yml`: build, notes, upload |
| Correlation tokens in logs | ✅ | `ITHMB|component|EVENT|filename|details` convention |
| File size gate (250 LOC) | ✅ | CI gate via `tools/check-file-sizes.sh`. SIZE_OK exemptions. |
| Test quality gate | ✅ | Tautological assertions removed in v1.5.0 audit |
| Debug coverage gate | ✅ | 60% min in build-linux.yml |
| NEON coverage collection | ✅ | Coverage gate in test-neon.yml (ARM64, 60% min) |
| Native AOT benchmark | ✅ | `.github/workflows/benchmark-nativeaot.yml` |
| Post-release baseline update | ✅ | `.github/workflows/post-release-benchmark.yml` |
| Profile integrity verification | ✅ | SHA-256 hash logged in ProfileSystem.cs |
| Profile source diff script | ✅ | `tools/profile-diff.sh` |

### Tier 3 — Quality of Life

| Item | Status | How |
|------|--------|-----|
|| Design decision records | ✅ | `docs/adr/0001` (Native AOT), `0002` (SIMD dispatch), `0003` (profile resolution) |
|| Commit date alias | ✅ | `tools/git-commit-dated.sh` — preserves author+committer dates |
|| Release notes from CHANGELOG | ✅ | Auto-generated from CHANGELOG section in `release-windows.yml` |
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

No remaining gaps from the 11 items in the automation plan.
See `.omo/plans/ithmbcodec-automation-gaps.md` for complete execution history.

Items closed across 4 phases (commit `0943504`):
- Phase 0: File size gate exemptions, macOS flaky test fix
- Phase 1: README arch table (24 files), Debug coverage gate, release notes automation
- Phase 2: NEON coverage, quarterly audit protocol ADR + issue template, profile SHA-256 integrity
- Phase 3: Profile diff script, Native AOT benchmark workflow, post-release benchmark baseline


---

## Version

This file is versioned with the project. Update when automation or design standards change.

| Version | Date | Changes |
|---------|------|---------|
|| 1.0 | 2026-06-30 | Initial: automation tiers 0-3 + design axioms applied |
|| 1.1 | 2026-06-30 | Wave 1: CHANGELOG CI check, signed tag CI, commit-date script, PR template, v1.6.0 tag |
|| 1.2 | 2026-06-30 | Wave 2+3: ADRs, file size gate, SETUP.md, macOS CI, pre-commit, fuzz CI, gaps table restructured |
|| 1.3 | 2026-07-01 | Phase 0-3: size gate fix, macOS fix, README table, Debug/NEON coverage, release notes, audit protocol, profile integrity, profile diff, Native AOT benchmark, post-release baseline |
|| 1.3 | 2026-07-01 | Phase 0-3: size gate fix, macOS fix, README table, Debug/NEON coverage, release notes, audit protocol, profile integrity, profile diff, Native AOT benchmark, baseline update |
