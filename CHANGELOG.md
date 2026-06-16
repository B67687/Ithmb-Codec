# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Trailing bytes tolerance:** `DecodeRawProfile` now accepts files up to 256 bytes smaller than `FrameByteLength`, zero-padding undersized data. Handles real device alignment quirks where the encoder wrote fewer bytes than the expected frame size. Inspired by iOpenPod's `_resolve_packed_geometry` trailing-trim approach. (+2 tests)
- **JPEG carving fallback:** When a file has an unknown profile prefix, scan the entire file for embedded JPEG markers before giving up. Enables decoding of .ithmb files whose prefix is unknown but which contain JPEG data beyond the 4 MB peek buffer. Mimics File Juicer's byte-level carving approach. (+2 tests)
- **Centered crop infrastructure:** Added `CropX`/`CropY`/`CropWidth`/`CropHeight` fields to `IthmbVariantProfile` with full `profiles.json` parser support and post-decode cropping logic (applied after rotation). Ready for centered-padding photo formats (1007, 1015, 1024, 1093) once sample files validate exact crop dimensions. Based on iOpenPod's `_crop_visible_region`.
- **Quality pipeline:** `review.sh` unified 7-stage orchestrator (`editor`, `precommit`, `commitlint`, `test`, `ocr`, `codeql`, `links`) with `--list`, `--fix`, and stage-selection
- `AssertDeterminism` shared test helper — eliminates alloc×2+compare boilerplate in determinism tests
- Determinism tests for RGB555, YUV422, YCbCr420 decoders (+3 tests, 332 total)
- AI-assisted development disclosure in README (model, reasoning, platform, workflow)
- **ARM64 NEON SIMD:** full NEON (AdvSimd) implementations for RGB565, RGB555, and UYVY decoders — these previously fell to scalar on ARM64

### Changed

- `Property_Determinism_AllDecoders` expanded from 2→7 decoders via shared helper
- AI declaration updated to specify chain-of-thought reasoning, multi-agent delegation, and context budgeting
- README SIMD section: now documents ARM64 NEON alongside x64 SSE2/SSSE3

### Fixed

- Ghost heading `// ---- P4f: Determinism ----` removed from Exhaustive.cs (no test)

### SIMD Architecture

| Decoder  | x64                     | ARM64                                                         |
| -------- | ----------------------- | ------------------------------------------------------------- |
| RGB565   | SSE2 (`Sse2.*`)         | NEON (`AdvSimd.*`)                                            |
| RGB555   | SSE2 (`Sse2.*`)         | NEON (`AdvSimd.*`)                                            |
| UYVY     | SSSE3 (`Ssse3.Shuffle`) | NEON (`AdvSimd.Arm64.VectorTableLookup` + `ZipLow`/`ZipHigh`) |
| YCbCr420 | Generic `Vector128<T>`  | Generic `Vector128<T>` (already cross-platform)               |
| CLCL/CL  | Scalar                  | Scalar                                                        |

Dispatch pattern for all NEON-enabled decoders: `Sse2.IsSupported` → SSE2, `AdvSimd.IsSupported` → NEON, else scalar.

## [1.0.0] — 2026-06-14

### Added

#### Core codec

- ImageGlass v10 Native AOT codec plugin with `ig_plugin_get_api()` ABI entry point
- JPEG extraction from T-prefix .ithmb files with SIMD-accelerated SOI scan (Span.IndexOf), JFIF/Exif validation within 512 bytes, StbImageSharp decoding, and EXIF orientation tag (0x0112) parsing
- Raw decoders for 48 profiles (22 photo + 26 cover art): RGB565, RGB555, UYVY (YUV422), YCbCr 4:2:0 (planar, padded and unpadded)
- Speculative decoders: CL per-pixel nibble chroma (Keith's Methods 3/4), CLCL shared nibble chroma, post-decode rotation (90/180/270 CW), swapped chroma planes for YCbCr
- Synthetic encoder (`BuildIthmbFile`) for all raw formats — drives exhaustive roundtrip tests
- `profiles.json` with 48 known profiles plus runtime extensibility
- CLI decoder tool (`tools/IthmbDecoder`) with BMP output

#### SIMD acceleration

- RGB565 → SSE2 (4-6× gain), 8 pixels per iteration
- RGB555 → SSE2 (identical pipeline to RGB565 with 5-bit green)
- UYVY → SSSE3+SSE2 (2-3× gain), pshufb deinterleave + 32-bit BT.601 arithmetic
- YCbCr 4:2:0 → cross-platform Vector128 (SSE2 on x64, NEON on ARM64, 3-5× gain), 2×2 blocks
- CLCL and CL decoders are scalar-only

#### Testing

- 317 unit tests across roundtrip (RGB565: 65,536 values, RGB555: 32,768, UYVY/YCbCr420 gradients), fuzz (250 inputs × 5 decoders), SIMD identity vs scalar, YUV tolerance, JSON parser, EXIF, and speculative paths
- Real-device validation: 956 iPhone 5 T-prefix files (100% extraction), 227 Jakarade public files (100% JPEG+EXIF detection)
- Size guard: 50 MB ceiling (32× over largest known 1.55 MB file)

### Changed

- Size guard 100 MB → 50 MB (data-driven)
- Double-copy eliminated in JPEG decode path (halves peak memory)
- SDK switched from local project reference → NuGet PackageReference v1.0.100266
- CL/CLCL encode order: moved before interlace to avoid wasteful intermediate encodes
- Monolithic test file (1908 lines) split into 7 partial class files by concern
- .gitignore switched to whitelist pattern (ignore all, un-ignore explicitly)
- SVG diagrams, DEVELOPMENT.md all refactored to draw test/profile counts from README (single source of truth) instead of hardcoding

### Fixed

- **CRITICAL:** Unaligned SSE — `Sse2.Store` 16-byte alignment → `Vector128.LoadUnsafe`/`StoreUnsafe` (movdqu)
- CLCL encoder byte offset bug (line 178: chroma written past block boundary)
- CLI BMP output had R/B channels swapped; added `--help`/`-h` flag
- CLCL nibble scaling ×17 → ×16 (confirmed against andrewmalta/ithmb and wrinklykong/pyithmb)
- Integer overflow in YCbCr bounds check (`requiredSize` calculation)
- Odd-width OOB reads in YUV422, Interlaced, and CLCL decoders (even-width guards)
- JPEG SOI false positive — validate FF prefix
- Redundant determinism tests merged into `Property_Determinism_AllDecoders`
- CodeQL alerts: missed-ternary-operator in RotateBgra, useless-assignment-to-local (JSON parser), path-combine (`Path.Combine` → `Path.Join`)
- 5 OCR review findings: CLI extension validation, dead dst allocation, redundant if guard, fuzz seed diversity, RgbToYuv fixed-point accuracy
- TryAdd dictionary leak → indexer; volatile profile field fix
- `profiles.json` endianness example corrected (`littleEndian: false` was missing)
- .gitignore whitelist entry for CHANGELOG.md

### Infrastructure

- CI: CodeQL analysis (weekly + push/PR), test workflow with live badge
- Quality: `.editorconfig` + Roslyn analyzer enforcement (`TreatWarningsAsErrors`)
- Pre-commit hooks: trailing-whitespace, end-of-file-fixer, JSON/YAML lint, markdown, large-file guard
- Conventional commit enforcement with `.commitlintrc.json` (type-enum: feat/fix/docs/refactor/test/chore/cleanup)
- Weekly broken-link check via lychee
- `.gitattributes` for consistent line endings

### Documentation

- README restructured 368→157 lines with GitHub alerts, badges, cross-platform table
- SVG architecture diagram: decode pipeline with 48 profiles, 50 MB guard, SIMD, rotation
- SVG pipeline diagram: Dev → Review → CI → Release workflow
- Standalone Acknowledgments section (25 surveyed implementations)
- CHANGELOG.md (Keep a Changelog format), GitHub PR template with doc checklist
- HARDWARE_GUIDE.md: iPod hardware validation plan
- Stale files removed: RESEARCH.md, SOURCES.md, ACADEMIC.md, src/README.md, .mmd files, decode-pipeline-test/
- REVIEW_PLAN.md scrubbed from all commit history

[Unreleased]: https://github.com/B67687/ithmb-codec/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/B67687/ithmb-codec/releases/tag/v1.0.0
