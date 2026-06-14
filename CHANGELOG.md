# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **CI/CD infrastructure:** Test workflow with live badge, CodeQL security scanning (weekly + push/PR), conventional commit enforcement on PR, weekly broken-link check
- **Quality enforcement:** `.editorconfig` + Roslyn analyzer rules (`TreatWarningsAsErrors`), pre-commit hooks (whitespace, JSON/YAML/markdown lint, large-file guard), `.commitlintrc.json` with custom type-enum
- **Review pipeline:** `review.sh` — unified 7-stage orchestrator (`editor`, `precommit`, `commitlint`, `test`, `ocr`, `codeql`, `links`), runnable individually or in bulk, with `--list` and `--fix`
- CHANGELOG.md (Keep a Changelog format) and GitHub PR template with documentation checklist
- Standalone Acknowledgments section and AI-assisted development disclosure in README
- `AssertDeterminism` shared test helper — eliminates alloc×2+compare boilerplate pattern
- Determinism tests for RGB555, YUV422, YCbCr420 decoders (+3 tests, 332 total)
- `profiles.json` encoding descriptions added to existing profile table

### Changed

- **Docs as source of truth:** README now uses live badge for test count; SVG diagrams, DEVELOPMENT.md, and pipeline.svg all reference README instead of hardcoding counts — single source of truth
- **Pipeline unified:** `review.sh` rewritten as composable 7-stage orchestrator with `--list`, `--fix`, and stage-selection syntax; replaces ad-hoc 4-layer script
- **Determinism coverage:** `Property_Determinism_AllDecoders` now covers all 7 decoders (was 2); per-decoder determinism tests added for RGB555, YUV422, YCbCr420
- Monolithic test file (1908 lines) split into 7 partial class files
- SVGs: image sizes, profile counts (47→48, 25→26), test counts removed in favor of change-agnostic checkmark

### Cleaned

- Removed stale files: `RESEARCH.md`, `SOURCES.md`, `ACADEMIC.md`, `src/README.md`, `*.mmd` diagrams, `tools/decode-pipeline-test/`
- Scrubbed `REVIEW_PLAN.md` from all commit history via `git filter-repo`
- Removed `// ---- P4f: Determinism ----` ghost heading (had no test)

### Fixed

- **CodeQL:** useless-assignment-to-local (JSON parser loop exit restructuring) and path-combine (`Path.Combine` → `Path.Join`)
- README line counts synced with actual source sizes; profiles.json examples corrected (endianness)
- Architecture and pipeline SVGs refreshed to match current profile/test counts

## [1.0.0] — 2026-06-14

### Added

- Initial release of the ImageGlass v10 Native AOT codec plugin
- JPEG extraction from T-prefix .ithmb files with SIMD-accelerated SOI scan
- Raw decoders for 48 profiles (RGB565, RGB555, UYVY, YCbCr420, CLCL)
- Speculative decoders: CL per-pixel chroma, swapped chroma planes, post-decode rotation
- Synthetic encoder for roundtrip test generation
- 317 unit tests covering exhaustive roundtrip, fuzz, SIMD identity, parsers
- Verified against 956 iPhone 5 + 227 public Jakarade files (100%)
- HARDWARE_GUIDE.md for iPod hardware validation path
- Whitelist .gitignore, .editorconfig, .gitattributes
- CodeQL security scanning

### Fixed

- **CRITICAL**: Unaligned SSE — `Sse2.Store` 16-byte alignment and `Vector128.LoadUnsafe`/`StoreUnsafe`
- Size guard 100 MB → 50 MB (data-driven)
- Double-copy eliminated in JPEG decode path (halves peak memory)
- CLCL encoder byte offset bug
- CLI BMP output had R/B channels swapped
- Integer overflow in YCbCr bounds check
- Odd-width OOB read in YUV422 decoders
- Redundant determinism tests merged
- JPEG SOI false positive — validate FF prefix
- CLCL nibble scaling *17 → *16 (confirmed by upstream)
- TryAdd leak → indexer; volatile profile fix
- Defense-in-depth hardening from 5-lens review

[Unreleased]: https://github.com/B67687/ithmb-codec/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/B67687/ithmb-codec/releases/tag/v1.0.0
