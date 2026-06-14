# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Pre-commit hooks (.pre-commit-config.yaml) — trailing whitespace, JSON/YAML, markdown lint
- Broken link checker (`.github/workflows/links.yml`) — weekly scan of all docs
- Conventional commit enforcement (`.github/workflows/commits.yml`)
- `.commitlintrc.json` — custom type-enum matching project convention
- `review.sh` — 4-layer review pipeline (EditorConfig → Test → OCR → CodeQL)
- CHANGELOG.md and PR template with documentation checklist
- Test CI workflow with live status badge
- `.editorconfig` + Roslyn analyzer enforcement (`TreatWarningsAsErrors`)
- Standalone Acknowledgments section in README

### Changed

- README now uses live badge for test count instead of hardcoded number
- SVGs updated to avoid specific counts — diagrams stay current without edits
- DEVELOPMENT.md links to README for test count (single source of truth)
- pipeline.svg: "329 tests" → "✓" (change-agnostic)

### Fixed

- CodeQL useless-assignment-to-local: restructured JSON parser loop exit
- CodeQL path-combine: `Path.Combine` → `Path.Join`
- README line counts updated (~933→~1015, ~613→~660, added Encoding.cs)
- Architecture SVG: sizes, profile counts updated (47→48, 25→26)
- Pipeline SVG: "317 tests" → current

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
