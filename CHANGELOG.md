# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Test CI workflow with live status badge (#5518550)

### Changed

- README now uses live badge for test count instead of hardcoded number

## [1.0.0] — 2026-06-13

### Added

- Initial release of the ImageGlass v10 Native AOT codec plugin
- JPEG extraction from T-prefix .ithmb files with SIMD-accelerated SOI scan
- Raw decoders for 48 profiles (RGB565, RGB555, UYVY, YCbCr420, CLCL)
- Speculative decoders: CL per-pixel chroma, swapped chroma planes, post-decode rotation
- Synthetic encoder for roundtrip test generation
- 329 unit tests covering exhaustive roundtrip, fuzz, SIMD identity, parsers
- Verified against 956 iPhone 5 + 227 public Jakarade files (100%)
- HARDWARE_GUIDE.md for iPod hardware validation path
- Whitetlist .gitignore, .editorconfig, .gitattributes
- CodeQL security scanning

### Fixed

- Size guard 100 MB → 50 MB (data-driven)
- Double-copy eliminated in JPEG decode path (halves peak memory)
- CLCL encoder byte offset bug
- CLI BMP output had R/B channels swapped
- Integer overflow in YCbCr bounds check
- Odd-width OOB read in YUV422 decoders
- Redundant determinism tests merged
