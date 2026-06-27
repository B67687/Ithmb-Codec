# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]
### Added
- **REC_RGB555 decoder (quad-tree / Morton Z-order) for iPhone/Touch cover art:** Apple's recursive-ordered dither format used by profiles 3001/3002/3003 (256×256, 128×128, 64×64). Pixels are stored in Morton Z-order (interleaved bit pattern) rather than raster scanline. Decoder de-deranges via MortonInterleave, then decodes as standard RGB555→BGRA8. Encoder (`EncodeReorderedRgb555`) reorders BGRA8→RGB555 via Morton Z-order for writing iPhone-compatible .ithmb files.
- **Format 3004 SlotSize:8192 added** — libgpod's Itdb_ArtworkFormat `padding` field confirmed 8192 bytes slot padding for iPhone/Touch photo thumbnails (profile 3004, 56×55 Rgb555).
- **Format 3005 (320×320 Rgb555) added** — iPhone/Touch cover art variant from libgpod's `ipod_touch_1_cover_art_info` table.
- **Profiles count: 53→54 active** (3005 added). Total known: 54 active + 1 speculative disabled (F1064).
- **Full libgpod comparison completed:** All 42 overlapping format IDs verified identical. REC_RGB555 decoder for 3001-3003, 3004 SlotSize, 3005 profile added.
### Changed
- **Profile flexibility system:** Added `UseMhniDimensions` flag (use actual Width/Height from MHNI chunk instead of profile's fixed dimensions) and `FallbackEncodings[]` array (ordered list of alternative encodings on primary decode failure). Enabled `UseMhniDimensions:true` on profile 1061 to resolve libgpod (56×56) vs Reuhno (55×55) dimension disagreement.
- **PhotoDb/Core.cs tuple expanded:** WalkEntries and TryParsePhotoDb output now includes Width, Height from MHNI header (6-element tuple).
- **PROFILES.md updated:** 3001/3002/3003 encoding changed from RGB555 to Reordered RGB555. Profile count 53→54. UseMhniDimensions and FallbackEncodings documented in advanced flags table.

### Fixed
- **Documentation audit: stale profile/test counts updated across all docs.** README, CHANGELOG, ACKNOWLEDGMENTS, and what-is-this.md corrected: 49→53 built-in profiles, 528→530 tests, 35+→33 surveyed implementations, stale LOC in source layout table. v1.4.0 comparison URL added to CHANGELOG.

### Added
- **linux-x64 CI build workflow:** `.github/workflows/build-linux.yml` — validates both Release and Debug builds on push/PR for the primary development platform.
- **NuGet dependency lockfile:** `packages.lock.json` enabled for the test project — locked restore (`--locked-mode`) ensures reproducible builds.
- **Dependabot configuration:** `.github/dependabot.yml` for weekly NuGet and GitHub Actions dependency updates.

### Changed
- **BT.601 YUV coefficients relocated:** `YuvRCoef`, `YuvGCoefCb`, `YuvGCoefCr`, `YuvBCoef` moved from `Rgb565Rgb555.cs` to `YuvUtils.cs` where they are used. Functional no-op (same partial class).

## [1.4.0] — 2026-06-26

### Fixed

- **F1061 profile dimensions corrected:** Width/Height changed from 56×56 to 55×55, FrameByteLength changed from 6272 to 6160 (56×55×2) to match real data from Reuhno's iPod Classic 6G. The slot is 56-pixel rows × 55 rows, not 56×56. The stride fix (src.Length/h = 112) correctly reads 55 pixels from each 112-byte row.
- **Input row stride computed from actual data size instead of declared width:** All decoders (RGB565/RGB555 SSE2, NEON, Scalar, Tail × UYVY SIMD, NEON, Scalar, interlaced SIMD/NEON/Scalar × CLCL, CL) now compute input row stride as `src.Length / h` instead of `w * 2`. This fixes padded formats like F1061 (55×55 nominal, 56-pixel rows = 112-byte stride vs the old 110-byte stride that misaligned every row past row 0). Discovered via Reuhno's real iPod Classic 6G samples. (+0 tests, behavior-preserving for unpadded formats)
- **Tail destination offset in SSE2/NEON paths:** `DecodeRgb565_Tail` and `DecodeRgb555_Tail` wrote to `pDstRow` instead of `pDstRow + xStart * 4`. Pre-existing bug that only manifests when `w % 8 != 0` with the SIMD path (e.g., F1061 at w=55). All previous test widths were powers of 2 or <8 (falling through to scalar). (+3 stride tests, 530 total)
- **Multi-frame decode span slicing:** `DecodeRawProfile` sliced the source buffer with `data.AsSpan(frameStart)`, passing the entire remaining buffer tail instead of just the current frame. Exposed by the stride fix (old `w*2` stride ignored `src.Length`). Now correctly trims to `frameSize` bytes.
- **Interlaced UYVY field offset:** `DecodeYuv422Interlaced` methods computed the second-field offset using `(h+1)/2 * w * 2` instead of `(h+1)/2 * rowStride`. Fixed to use data-derived stride for consistency.

### Validation

- **First real F-prefix .ithmb samples decoded successfully:** 5/5 frames from Reuhno's iPod Classic 6G (F1061 at 4 offsets + F1055 at offset 0) confirmed working. Validates the stride fix, tail destination fix, and F1061 profile correction against real hardware data for the first time.

### Added

- **`swapRgbChannels` support completed for RGB565:** Scalar and Tail decode paths now accept the `swapRgbChannels` parameter (matching existing SSE2/NEON coverage). Completes BGR;15 support for all 5 DecodeRgb565 paths. (+0 new tests)
- **Padded row stride tests:** 3 new tests verify correct decode for padded (55×55 nominal, 56-pixel rows), unpadded square, and non-square padded dimensions through both SSE2 and Scalar paths.
- **Format IDs 1042, 1043, 3006, 3007:** Four additional built-in profiles — 1042 (320×240 RGB565, Classic photo alias for 1024), 1043 (130×88 RGB565, alias for 1015), 3006 (56×56 RGB555, iPod Touch cover art, slot-padded), 3007 (88×88 RGB555, slot-padded). Adds `SlotSize` field to `IthmbVariantProfile` for padded profiles. (+2 tests, 530 total)
- **CLI `--extract-all-pd` and `--list-devices` flags:** `--extract-all-pd` batch-decodes all entries in a PhotoDB/ArtworkDB to individual BMPs; `--list-devices` prints the 18-device format table to stdout.
- **PhotoDB inline JPEG blob detection:** `TryParsePhotoDb` post-processing detects entries with unknown format_id + FF D8 prefix and dispatches to `DecodeJpegSlice`. Covers PhotoDB entries where Apple stored JPEG data instead of raw pixel data. (+1 test)
- **Byte-level corruption fuzz test:** `Fuzz_Corruption_RandomByteMutations` — 1000 iterations, fixed seed 42, all 7 decoders at random 4-128 dims. Random mutations: 10% bit flip, 5% byte swap, 5% truncate, 80% clean. NativeMemory alloc/free, no try/catch. (+1 test, 530 total)
- **Reuhno CC0 synthetic test vectors:** 10-entry ArtworkDB + F1061/F1055/F1060 multi-frame .ithmb files + 30 reference PNGs + manifest.csv with SHA256 checksums. All 30 SHA256s verified against manifest. Committed to `samples/reuhno-synthetic/`. (+6 tests)
- **ImageMagick delegate registration:** `tools/ithmb-delegate.xml` registers ITHMB format in ImageMagick's delegate system — runs `IthmbDecoder` behind the scenes for `magick ithmb:file.ithmb out.png`. Installer script at `tools/install-ithmb-magick.sh`.
- **Benchmark summary in README:** 7-row benchmark table added under Tooling (RGB565=64µs, RGB555=66µs, UYVY=190µs, YCbCr420=221µs, YUV422I=190µs, CLCL=457µs, CL=591µs, all zero allocs).

### Changed

- **MaxDecodeFileSize 8 MB → 32 MB:** Increased file size guard from 8 MB to 32 MB after systematic research. All public real-world .ithmb files are under 1 MB (max: 852 KB). The 32 MB limit covers ~40 max-size (P1007) raw frames — far beyond any realistic thumbnail cache. Ratio-naled from scratch: 32 MB is a power of 2, covers all known data, and does not borrow authority from libgpod's commonly-repeated but uncorroborated 256 MB limit. Researched via profile frameBytes analysis (53 profiles, max 829 KB), multi-frame concatenation limits from 5 RE tools (Keith/ithmbrdr/iOpenPod/libgpod/clickwheel), and a public .ithmb file size survey (GitHub has zero .ithmb binary files; largest local sample = 852 KB).
- **README restructured:** "How it works" updated with PhotoDB chunk parser path (#4) and JPEG carving fallback; Performance promoted to standalone section with benchmark table; Testing section fixes (FAU.edu demoted); Acknowledgments extracted to own top-level section (duplicate bottom stub removed). F-prefix warning softened from "best-effort" to "validated."
- **ACKNOWLEDGMENTS reorganized:** "Primary references" (~13 projects that shaped decoder architecture) and "Additional references" (~14 surveyed projects) replace flat alphabetical list. Reuhno entry condensed.

### Security

- **PhotoDB chunk walker recursion depth limit:** `IntegrityWalkTree` and `WalkEntries` now hard-stop at depth 64, preventing stack overflow from crafted PhotoDB files (CWE-674).
- **`_rawFileCache` LRU eviction:** Cache limited to 16 entries. Oldest entry evicted when limit exceeded. Prevents unbounded memory growth from repeated decode requests (CWE-770).
- **Carve path bounds guard:** `carveOffset` clamped to file bounds before carving scan, preventing OOB read from truncated files (CWE-125).
- **JSON parser nesting depth limit:** `SkipJsonValue` stops at depth 32, preventing CPU DoS from deeply nested JSON (CWE-674).
- **Zero-byte file guard:** `DecodeInternal` rejects zero-length files before any processing, preventing division-by-zero in stride computation.

### Maintenance

- **Reuhno credit updated:** ACKNOWLEDGMENTS.md now links to github.com/reuhno instead of reuhno.fr.
- **libgpod URL corrected:** libgpod org moved from `github.com/libgpod/libgpod` to SourceForge (`sourceforge.net/p/gtkpod/libgpod/ci/master/tree/`). Updated in README and ACKNOWLEDGMENTS.
- **Badge SVGs updated:** tests 517→530, commits 139→190, profiles 49→53.
- **.gitignore expanded:** Blacklists for `.ruff_cache`, `BenchmarkDotNet.Artifacts`, `.omo`, ``. Whitelist for synthetic sample `.ithmb` files.
- **packages.lock.json enabled:** Deterministic restore via `RestorePackagesWithLockFile` for supply-chain integrity.
- **CI workflow SHA pinning:** `test-neon.yml` actions pinned by commit SHA instead of mutable tags.
- **CI permissions restricted:** `actions: read`, `contents: read` (default least-privilege).

### Refactored

- **PhotoDB parser extracted to own namespace:** `IthmbCodecPlugin.PhotoDb.cs` split into `PhotoDb/Core.cs` + `PhotoDb/Serialization.cs` under `IthmbCodec.PhotoDb` namespace. No longer a partial class of `IthmbCodecPlugin`. (-0 LOC, cleaner separation).
- **ClclCl.cs split by format:** Single 256-LOC file doing 4 things (CLCL, CL, YCbCr420, WriteYuvPixel) split into `DecodeFormatClcl.cs`, `DecodeFormatCl.cs`, `DecodeFormatYcbcr420.cs`, and `YuvUtils.cs`. Each file owns one decoder. (-0 LOC, single responsibility).
- **Duplicate endian readers consolidated:** Five private helpers in Plugin.cs (ReadU16LE/BE, ReadU32LE/BE, ReadInt32BigEndian) replaced with `System.Buffers.Binary.BinaryPrimitives` calls. (-13 LOC).
- **ProcessUyvyRow parameter count reduced:** 12-parameter signature packed into `UyvySimdConstants` readonly record struct. Method body unchanged. (-9 params, cleaner API).
- **RGB565/RGB555 SSE2/NEON deduplicated:** 4 near-identical SIMD methods (340 LOC) merged into 2 parameterized `DecodeRgbX_Sse2`/`DecodeRgbX_Neon` methods with mask/shift constants. All 4 public wrappers are `[MethodImpl(AggressiveInlining)]` one-liners that JIT to the same assembly. (-260 LOC, -63% file size).

## [1.3.0] — 2026-06-23

### Added

- **BGR;15 channel-swapped RGB555 (`SwapRgbChannels`):** Added `SwapRgbChannels` bool parameter to `IthmbVariantProfile` and `profiles.json` parser, new `swapRgbChannels` JSON field. When true, the RGB555 decoder reads `xBBBBBGGGGGRRRRR` (BGR;15) layout for iPhone 2G thumbnail compatibility. Applied to all 5 decoder paths (Tail, Scalar, SSE2, NEON, public DecodeRgb555 entry point) and encoder (`EncodeRgb555`, `BuildIthmbFile`). SIMD uses a conditional branch outside the pixel loop (zero overhead on the hot path). (+3 tests, 517 total)
- **PhotoDB/ArtworkDB writer (`TryBuildPhotoDb`):** Added `TryBuildPhotoDb` to `IthmbCodecPlugin.PhotoDb.cs` — builds complete ArtworkDB binary from a list of (format_id, pixel_data, width, height) entries. Writes MHFD header → MHSD section → MHNI entries → pixel data (all entries first, then all pixel data for correct multi-entry roundtrip). Enables artwork sync to iPod without external tools. (+3 tests)
- **PhotoDB integrity checker (`IntegrityCheckPhotoDb`):** Added `IntegrityCheckPhotoDb` + `IntegrityWalkTree` to PhotoDb.cs — validates chunk structure sanity, MHNI overlapping ranges, known format ID checks, trailing garbage detection. CLI `--check-pd` flag for stand-alone verification. (+3 tests)
- **Device-specific format tables (`DeviceProfiles.cs`):** New `IthmbCodecPlugin.DeviceProfiles.cs` with static format tables for 18 iPod generations: Classic (5G, 5.5G, 6G), Nano (1G-7G), iPod Touch (1G-4G), iPhone (1G-2G), iPod Mini (1G-2G), iPod Photo (4G), iPod Video (5G), and iPod Mobile (Motorola). Each entry lists the format IDs required by that device for thumbnail display and cover art. (+5 tests)
- **Format 1081 (640×480 RGB565):** New built-in profile for iPod Classic/Nano cover art large variant, documented in the consolidated format table from multiple sources. (+0 tests, 53 profiles total)

### Changed

- **Refactored monolithic source files into domain-focused partial classes.** 6 oversized files (all 900-1200 LOC) split into 15 targeted files. Plugin.cs → Plugin.cs + DecodePipeline.cs + JpegDecode.cs + ProfileSystem.cs. Decoding.cs → Rgb565Rgb555.cs + UyvyYuv.cs + ClclCl.cs. Roundtrip.cs → 3 specialized test files. Statistical.cs → 2 focused test files. Fuzz.cs → base + SimdTail.cs. EncoderHelpers.cs extracted from Encoding.cs. Build clean, 498 tests pass. Total source lines unchanged; no behavioral change.

## [1.1.0] — 2026-06-22

### Added

- **Multi-frame raw decode:** F-prefix `.ithmb` files may contain multiple concatenated raw frames. Added `_rawFileCache` (ConcurrentDictionary) for read-once decode-many access, `DecodeRawProfile` frame slicing (frameStart = 4 + frameIndex * frameSize), `FillImageInfo` FrameCount propagation, and `frameIndex >= 0` acceptance in `CodecDecodeStaticRaster`. 3 multi-frame tests (+7 tests total). Confirmed by Keith's iPod Photo Reader, ithmbrdr, libgpod, and iOpenPod.
- **Rotation roundtrip tests:** Added `RotateBgra_90_Correctness`, `RotateBgra_270_Correctness`, and `RotateBgra_90_Roundtrip_Identity` — verify rotated output pixel correctness and encode→decode→rotate identity. (+3 tests)
- **ARM64 NEON CI:** `.github/workflows/test-neon.yml` runs all tests on native `ubuntu-24.04-arm` GitHub Actions runners, exercising `AdvSimd.IsSupported` code paths. `scripts/test-neon-locally.sh` for local QEMU user-mode NEON validation.

### Fixed

- **CRITICAL — SSE2 buffer overrun:** Removed overly conservative `(w & 3) == 0` guard from RGB565/RGB555 SIMD dispatchers. The `x + 7 < w` loop bound plus scalar tail handler (`DecodeRgb565_Tail`, `DecodeRgb555_Tail`) already prevent any buffer overrun. The old guard masked a phantom bug and blocked SIMD for widths like 10, 14, 18. (`IthmbCodecPlugin.Decoding.cs` lines 29, 214)
- **CRITICAL — RotateBgra heap buffer overflow:** 90° CW formula `y * dstW + (srcW - 1 - x)` wrote past the allocated buffer for non-square images (h > w). 270° CW formula had the same bug for w > h. Corrected to: 90° CW = `x * srcH + (srcH - 1 - y)`, 270° CW = `(srcW - 1 - x) * srcH + y`. Both produce indices strictly within `[0, srcH*srcW - 1]`. Confirmed crash in isolation via `malloc(): unaligned tcache chunk detected`. (`IthmbCodecPlugin.cs:742`)
- **SECURITY — Integer overflow in crop bounds check:** `profile.CropX + profile.CropWidth` used unchecked int addition. A crafted `profiles.json` with max-value offsets could bypass the `<= w` guard and cause OOB heap read. Fixed with `(long)` cast. (`IthmbCodecPlugin.cs:674-675`)
- **YCbCr420 interlaced encoder:** `InterlaceFields` only copied Y luminance for YCbCr420, producing green-tinted output. Changed to 3-plane interlace (Y + Cb + Cr) independently. Using ceiling division for chroma dimensions to match encoder plane size. (`IthmbCodecPlugin.Encoding.cs:326-368`)
- **Error code consistency:** `DecodeRawProfile` returned `DecodeFailed` for out-of-range frameIndex, while the cache-path in `DecodeInternal` returned `InvalidArg`. Unified to `InvalidArg`. (`IthmbCodecPlugin.cs:578`)

### Removed

- **F1064 speculative profile:** Commented out — no real-world sample has been found across iOpenPod, Keith's iPod Photo Reader, libgpod, or any public iPod Photo Cache dump. Asserting test updated. (`IthmbCodecPlugin.cs:116-120`, `IthmbCodecTests.Exhaustive.cs:232-233`)

### Changed

- **Documentation updated:** README.md test count (456→466), profile count (49→48), multi-frame statement, review cycles count; PROFILES.md F1064 marked disabled, count corrected; HARDWARE_GUIDE.md multi-frame support note; badge SVGs (tests.svg, showcase.svg) test count 460→466; InterlaceFields uses ceiling division for odd-dimension chroma compatibility; BuildIthmbFile comment step numbering fixed.

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

[Unreleased]: https://github.com/B67687/ithmb-codec/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/B67687/ithmb-codec/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/B67687/ithmb-codec/compare/v1.1.0...v1.3.0
[1.1.0]: https://github.com/B67687/ithmb-codec/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/B67687/ithmb-codec/releases/tag/v1.0.0
