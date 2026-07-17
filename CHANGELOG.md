# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.9.4] - 2026-07-16

### Added

- **macOS ARM NEON acceleration**: The `not(target_os = "macos")` gate removed from all 19 dispatch points. NEON BRGA interleave bug fixed (vzip_s16 pairing corrected). Confirmed by all 581 tests passing on ARM64 CI runner. Apple Silicon now gets full NEON speed.
- **Branch protection enabled**: `main` requires `rust-ci` to pass before merge.
- **Dependabot auto-merge split**: Approval runs immediately, auto-merge waits for CI checks.

### Fixed

- **NEON BRGA interleave bug**: `vzip_s16` produced BRGA instead of BGRA in 4 interleave sites.
- **cargo-audit CI failure**: Replaced broken `actions-rust-audit/rust-audit@v1` with manual `cargo install cargo-audit && cargo audit`.
- **macOS-13 runner in release CI**: `macos-13` runner was fully deprecated (removed Dec 2025). Switched to `macos-15-intel`.
- **Benchmark baseline corruption**: Restored from commit history, updated to reflect encoder SIMD revert.

### Removed

- **Encoder SIMD**: Removed `simd/enc.rs` (was causing 3x regression compared to scalar).
- **Publish workflow**: Removed `publish.yml` (manual crates.io publishing preferred).
- **CONTRIBUTING.md**: Removed (no external contributions expected).
- **Unconditional cdylib**: Changed back to `["lib", "cdylib"]` — needed for C ABI checks.

### CI

- `cargo fmt --check` job added
- `cargo audit` job added
- Release workflow runs `cargo test` before building artifacts
- Python wheels built as GitHub Release artifacts (macOS/Windows/Linux)
- CLI binary builds for 5 targets (Linux x64/ARM64, macOS x64/ARM64, Windows x64)
- Benchmark regression threshold tightened from 5.0x to 1.25x
- macOS ARM NEON tested on `ubuntu-24.04-arm` runner
- README features table, SECURITY.md, ADR-0001 updated
- 609 tests passing, clippy clean

### Docs

- CHANGELOG: v1.9.4 entry added with full scope
- AGENTS.md: NEON gate status updated
- ADR-0001: macOS ARM NEON section re-written
- ithmb-core README: features table fixed, config.rs doc example fixed
- ithmb-cli README: created (was missing)
- SECURITY.md: unsafe code claim corrected

## [1.10.0-enterprise] — 2026-07-13

### Added

- **WASM decoder** — Browser-based .ithmb decoder at [Ithmb-Codec-Dev](https://github.com/B67687/Ithmb-Codec-Dev)
- **Opt-in format metadata sharing** — GitHub issue via checkbox-gated opt-in
- **ADR-0006** — Zero-trust telemetry pattern documented
- **Enterprise page redesign** — Apple-style light theme, realistic support promises
- **Gen tool profile-aware defaults** — `ithmb-gen` generates at profile-matching dimensions

### Fixed

- **Plugin ABI rewrite** — IGCodecApi/IGCodecCapability matched to C# SDK
- **Pixel format enum** — Unknown(0) to Bgra8Unorm(1) for ImageGlass compat
- **Stack buffer overrun** — Removed monitor thread calling is_cancellation_requested(null)
- **Metadata dimension parsing** — Fixed comma-formatted profile descriptions

## [1.9.3] - 2026-07-16

### Added

- `#[non_exhaustive]` to DecodeConfig, Profile, DecodedImage for forward-compatibility
- `Display` impl for `Encoding` and `DecodeConfig`
- `PartialEq`/`Eq` for DecodeConfig and Profile
- Root-level re-exports for `PhotoDbEntry`, `PhotoDbMetadata`, `built_in_profiles()`

### Fixed

- Overflow safety: added `checked_mul` guards in dimension calculations
- Panic removed from profile DB initialization
- Truncated doc on `decode_with_profile_with_config`
- Duplicate `cache` feature entry in README
- SECURITY.md inaccurate unsafe code claim
- Config.rs doc example misplaced
- Unconditional cdylib removed from library crate-type

### CI

- Added `cargo fmt --check` workflow
- Added `cargo audit` security advisory check
- Tests now run before release artifact build
- Changed: FAIL_THRESHOLD 5.0→1.25
- Removed: encoder SIMD (was causing 3x regression)

## [1.9.0] — 2026-07-07

### Added

- **Initial Rust port** — Full C#→Rust conversion of Ithmb-Codec as a pure Rust workspace (5 crates: ithmb-core, ithmb-cli, ithmb-core-cabi, ithmb-gen, pymod)
- **8 pixel decoders** — RGB565, RGB555, ReorderedRGB555 (Morton Z-order), UYVY (YUV422), YCbCr420, CLCL (nibble-chroma), CL (per-pixel chroma), JPEG
- **7 synthetic encoders** — All raw formats with BuildIthmbFile dispatch, rotation, padding, interlacing
- **PhotoDB/ArtworkDB** — Binary chunk parser, writer, and integrity checker (9 chunk types: MHFD, MHSD, MHLI, MHII, MHNI, MHBA, MHIA, MHIF, MHOD)
- **DeviceProfiles** — 18-device iPod/iPhone format lookup tables
- **SIMD acceleration** — SSE2 for YUV conversion (UYVY, YCbCr420, CL, CLCL) with AVX2 and ARM NEON runtime dispatch behind `--features simd`
- **Python bindings** — PyO3/maturin crate (abi3-py312 stable ABI), expose decode/open_ithmb functions
- **LRU raw file cache** — RwLock<LruCache> backed CachedDecoder behind `cache` feature
- **Decode timing metrics** — 7 per-format atomic counters behind `metrics` feature
- **Cancellation polling** — AtomicBool parameter in all decoder functions, DecodeError::Canceled variant
- **Fuzz testing** — 2 libfuzzer targets (fuzz_decode_ithmb, fuzz_open_ithmb), 1.2M+ iterations, 0 crashes
- **Exhaustive roundtrip tests** — 65k RGB565 values, 32k RGB555, 15k+ CL nibble combos
- **Concurrency stress tests** — 11 threaded decode scenarios (Barrier sync, cancellation, repeated decode)
- **Statistical validation** — Decorrelation, entropy, histogram analysis of decoded output
- **Edge case coverage** — Zero-dimension, corruption, partial data, truncation, oversized inputs
- **Synthetic test vectors** — 36 programmatic test vector generation functions
- **Golden test vectors** — 15 reference files across all 7 encoding formats
- **Benchmark suite** — 10 divan benchmarks for decoders, encoders, and pipeline throughput
- **Format specification** — docs/FORMAT.md: complete .ithmb format reference (5,446 words, 46 sections)
- **CLI tool** — ithmb binary with decode, info, list-profiles, frame extraction, raw output, PNG output, benchmarks
- **Sample generator CLI** — ithmb-gen crate for generating test .ithmb files in all 7 formats
- **C ABI** — cdylib exposing ImageGlass v10 plugin ABI for FFI integration
- **STATS.md** — Canonical single-source-of-truth for test counts, suite breakdown, and project statistics (README now references STATS.md instead of hardcoding)
- **ADR-0005: 8 MB file size guard** — Systematic research-driven limit with 10× margin on largest known frame (see docs/adr/0005-file-size-guard-limit.md)
- **GLOSSARY.md, GUIDE.md** — Plain-english explainer for newcomers and iPod extraction walkthrough
- **ECOSYSTEM.md** — Documented original contributions to the .ithmb reverse-engineering space

### Changed

- **SIMD architecture** — RGB565/RGB555 use auto-vectorized scalar (hand-written SSE2/AVX2 was 34× slower on Intel due to AVX frequency downclock). SIMD kept only for YUV BT.601 arithmetic where it provides 1.5×+ speedup
- **CL/CLCL nibble scaling** — Corrected from ×17 to ×16 (<<4), matching SIMD expansion. 15,625 nibble combos verified
- **ReorderedRGB555 decoder** — Fixed row-major→Morton Z-order de-interleave for correct >2×2 decode
- **CI** — Added `--features simd` test matrix, aarch64 runner, clippy pedantic=deny enforcement
- **Docs reorganized**: docs/standards/, docs/benchmarks/, docs/guides/ under docs/ directory
- **README restructured**: Hardcoded test/profile counts replaced with STATS.md cross-refs; badge layout grouped; troubleshooting table pipe syntax fixed; unclosed code block fixed

### Fixed

- **CL/CLCL scalar nibble bug** — Cb/Cr variable swap and wrong expansion (×17 instead of <<4) corrected
- **ReorderedRGB555 Morton mismatch** — Decoder read row-major but encoder wrote Z-order Morton; now correctly de-interleaves
- **All clippy warnings** — 74+ warnings fixed across workspace, pedantic=deny enforced
- **macOS ARM NEON edge case** — NEON dispatch gated behind `not(target_os = "macos")`; macOS+simd uses scalar fallback. Documented in STANDARDS.md
- **File size guard (C# port gap)** — 8 MB limit with `DecodeError::FileTooLarge` variant, matching C# MaxDecodeFileSize
- **README URL casing** — Normalized ithmb-codec→Ithmb-Codec across all docs
- **CI checkout bumped** — actions/checkout v4→v7
- **Stale 32 MB references** — All README references updated to 8 MB (matching ADR-0005)

### Removed

- **C# CI workflow** — dotnet.yml and related .NET infrastructure removed (project is now pure Rust)
- **C# ADR documents** — Stale architecture decision records from C# era deleted
- **AVX-512 dispatch** — Removed after benchmarking showed 34× regression on Intel (frequency downclock + port-5 bottleneck)
- **tools/mcp-server/** — 27 MB node_modules deleted from repo
- **Stale v0.3.0 tag** — Orphan tag from squashed history, not on main

### Infrastructure

- **Rust workspace** — ithmb-core, ithmb-cli, ithmb-core-cabi, ithmb-gen, pymod (Python bindings)
- **CI/CD** — rust-ci.yml with build, test, clippy (pedantic=deny), fuzz build
- **.omo/** — Agent orchestration infrastructure for structured work tracking

### Docs

- **FORMAT.md**: Complete format specification covering all 6 known ithmb variants, header structures, profile tables, JPEG embedding, PhotoDB/ArtworkDB chunk format
- **STANDARDS.md**: Cross-platform SIMD lessons learned table; macOS NEON limitation documented; dispatch pseudocode updated with platform cfg
- **ADR-0004**: Quarterly audit protocol (removed stale synthesis reference)
- **ADR-0005**: 8 MB file size guard research and rationale

## [Unreleased]

### Added

_(placeholder for future changes)_

---

_The sections below record the C# era (v1.0 – v1.6). For Rust codec history, see [Unreleased] above._

## [1.6.0] — 2026-06-30

### Added

- **Thread safety: Lock wrapper for RawFileCache** — SetCachedFile, TryGetCachedFile, and ClearRawFileCache all wrapped in `System.Threading.Lock`
- **Observability: decode lifecycle logging** — every decode call emits START|{file}|{size} and END|{file}|{status}|{ms:F1}ms lines
- **Observability: periodic stats** — every 100 decode attempts a summary line (count, success, fail, avg ms)
- **Observability: shutdown stats** — `OnShutdown` logs cumulative decode attempts, successes, and fail rate
- **EncoderHelpers test suite** — 23 new tests: InterlaceFields (3 chroma modes), BT.601 known colors, scalar decoder fallback paths at sub-SIMD width w=6
- **AVX-512 decoder paths for RGB565/RGB555** — 32 pixels/iteration via Avx512BW, ~2× SSE2 throughput. Requires w%32==0 (falls back to SSE2).
- **SimdConstants shared class** — centralized all 8 shuffle masks and 7 coefficient vectors from UyvyYuv.cs, DecodeFormatClcl.cs, DecodeFormatCl.cs, DecodeFormatYcbcr420.cs. Eliminated ~70 lines of identical Vector128 definitions.
- **MHNI header parameterization** — `TryBuildPhotoDb` accepts optional mhniHeaderSize (default 76) and mhniPaddingSize (default 64) for non-Classic MHNI layouts.

### Changed

- **TryFindJpegSlice accepts ReadOnlySpan<byte>** — eliminates ArrayPool over-read bug from pooled array Length vs logical data size; callers pass exact span
- **\_rawFileCache LRU eviction** — Clear() replaced with oldest-LastAccess eviction, preventing cache thrashing under concurrent multi-image workloads
- **Peek buffer allocation** — new byte[4MB] replaced with ArrayPool<byte>.Shared.Rent/Return, reducing LOH pressure
- **KnownProfiles thread-safe publication** — Interlocked.Exchange replaces raw volatile field write
- **Log calls include correlation token** — all Log(4,...) in DecodePipeline.cs include Path.GetFileName(path) for traceability
- **Coverage gate raised 70% → 72%** — build-linux.yml threshold updated; Release+PGO yields ~74%, 72% provides stable buffer
- **Test count 571 → 594** — README updated
- **check-benchmark-regression.sh** — fixed: iterates all 7 report CSVs instead of `head -1`; correctly parses μs/ms/ns unit suffixes
- **Readme source layout 18 → 19 files** — added ProfilesJson.cs row to Architecture table

### Fixed

- **CI: gitleaks SHA pin** — updated to v3.0.0 actual commit `e0c47f4f8be36e29cdc102c57e68cb5cbf0e8d1e`
- **CI: YAML syntax error in SAST step** — `WARNING:` colon+space in plain scalar broke YAML. Quoted the `run:` value
- **CI: DecodePipeline indentation** — P4 try/finally left 4 spaces too-shallow indent; corrected by `dotnet format`
- **HasChildChunks false-positive** — header validation rejects fragments with hdrSize < 8, preventing spurious recursion into padding bytes matching known magic
- **Plugin shutdown memory leak** — FreePluginStrings() + FreePixelBufferCleanup() on OnShutdown() frees 8 AllocUtf16 native buffers
- **JPEG carving early bailout** — MaxCarvingFileSize = 8 MB prevents full-file scanning of unknown raw files with no JPEG markers
- **profiles.json parse failures logged** — skipped entries (prefix=0, validation failure) now log entry details instead of silent omission
- **Culture-sensitive SkipJsonValue** — char.IsWhiteSpace() replaced with explicit ASCII whitespace comparison
- **frameSize == 0 guard** — division-by-zero in multi-frame decode rejected with clear log message
- **Trailing-padding boundary** — inclusive semantics documented; exact boundary test (frameSize - 256) added
- **Orphaned /// tag** — duplicate WalkEntries doc comment removed from PhotoDb/Core.cs
- **Indentation fix** — return jpegSlice at DecodePipeline.cs line 167 realigned with surrounding block
- **CA1508 rationale** — comment explains why NativeMemory.Free(rotated) in finally block is safe after early return
- **iOpenPod issue link** — ProfilesJson.cs disabled profile comments reference issue #81

### Refactored

- **SIMD constants centralized** — per-method UyvySimdConstants struct instances replaced by shared SimdConstants static class
- **P5: DecodeInfrastructure extracted** — RawFileCache, DecodeMetrics, MaxCarvingFileSize from DecodePipeline.cs → new DecodeInfrastructure.cs
- **P5: Strings extracted** — AllocUtf16 buffers + InitStrings/FreePluginStrings from IthmbCodecPlugin.cs → new Strings.cs
- **P5: Helpers extracted** — MaxDecodeFileSize, TrailingPaddingTolerance, utility helpers from IthmbCodecPlugin.cs → new Helpers.cs
- **P5: PhotoDb/Types extracted** — chunk constants, data structs, read helpers from PhotoDb/Core.cs → PhotoDb/Types.cs
- **P5: JsonParser extracted** — JSON tokenizer + depth-limited parser from ProfileSystem.cs → new JsonParser.cs

### Security

- **NUL-in-path guard** — decode entry rejects paths containing embedded NUL bytes, preventing truncated-path decode in Native AOT
- **profiles.json integrity logging** — FNV-1a CRC logged on external profile load for tamper detection
- **Catch blocks log ex.Message** — all 3 bare `catch(Exception) { return; }` in ProfileSystem.cs now include error detail
- **AssemblyMetadata build provenance** — CommitSha and BuildTimestamp embedded at compile time via MSBuild target
- **SAST + secret scanning** — gitleaks-action@v3.0.0 added to build-linux.yml CI workflow
- **CLSCompliant(false)** — explicit opt-out via Properties/AssemblyInfo.cs, suppressing CS3000/CS3016 warnings from Native AOT unsafe patterns

### Performance

- **Two-phase peek buffer** — 512 KB first probe, extend to full 4 MB only if JPEG SOI found within window. Reduces peak I/O for common small thumbnails.
- **Decode metrics infrastructure** — Interlocked counters (_decodeCount, _decodeSuccessCount, _decodeTotalTicks) + GetDecodeStats()/ResetDecodeStats()

### CI/CD

- **dotnet format --verify-no-changes** — added as CI step in build-linux.yml, enforcing consistent formatting
- **Tag validation** — release-windows.yml validates tag matches `v*` before proceeding
- **Code coverage gate** — build-linux.yml collects XPlat Code Coverage with 72% threshold (raised from 70%)
- **Coverage report artifact upload** — build-linux.yml uploads coverage report XML for manual inspection
- **SAST + secret scanning** — gitleaks-action@v3.0.0 scans every push/PR
- **Benchmark comparison note** — benchmark.yml documents comparison against prior run artifact
- **Benchmark regression script fixed** — check-benchmark-regression.sh iterates all 7 report CSVs; handles ns/μs/ms unit suffixes
- **benchmark.yml deduplicated** — removed duplicate "Display dotnet info" step

### Testing

- **EncoderHelpers: 23 new tests** — InterlaceFields (2Bpp, YCbCr420, edge dims), BT.601 known colors, scalar decoder fallback at sub-SIMD width w=6
- **DecodePipeline coverage push** — UnknownPrefix_NoJpeg + UnknownPrefix_EmbeddedJpeg tests (+2)
- **JpegDecode coverage push** — real iPhone JPEG slice decode + early bailout paths (+2)
- **Metrics test** — DecodeInternal increments counters, ResetDecodeStats clears, GetDecodeStats returns cumulative values
- **PhotoDB roundtrip tests** — 3+ MHNI param combos (classic 76/64, Apple TV alternate, edge) verified build→parse→match
- **AVX-512 tail fuzz** — 8 width variants (1,31,32,33,47,63,64,65) verifying SSE2 fallback matches AVX-512 output
- **ArrayPool regression guard** — rent/return correctness verified; ensures no pool corruption or double-return
- **Cancellation stress test** — 100 iterations of mid-decode cancel on multi-frame raw decode; 0 memory leaks
- **PhotoDB chunk parser fuzz** — 20+ malformed MHFD/MHSD/MHNI inputs (corrupt sizes, negative offsets, truncation) verified graceful failure
- **Cache concurrency test** — 1000 iterations of 2+ concurrent writers/readers on _rawFileCache; no race or corruption
- **GetFormatIdName thread safety** — 1000 concurrent calls while KnownProfiles updates; no torn reads
- **Trailing-padding boundary tests** — exact (frameSize-256) succeeds, one byte beyond fails
- **TreatWarningsAsErrors** — enabled in test csproj; coverlet.collector added for coverage reporting

## [1.5.0] — 2026-06-29

### Added

- **Architecture SVG updated to match current profile counts** — 54 profiles (was 49), 25 photo (was 22), 29 cover art (was 27). Pipeline inner boxes vertically centered. Long decoder label shortened (RGB565/RGB555→RGB565/555). EXIF box widened to match JPEG Path. Font-size reduced on crowded decoder line.
- **README Contributions to ecosystem section updated** — surveyed implementations 4→22, dimension discrepancies 9→15. Added Steee29 iPhone 2G real-device validation, clickwheel 1062 discovery, gnupod/OrgZ profile corrections.
- **Format 1062 (56×56 RGB565, frameBytes=6272) added** — from clickwheel (dstaley) SysInfoExtended table. Not in any prior device profile. (+2 test assertions). Profiles count: 53 active + 1 speculative disabled (54 total).
- **REC_RGB555 decoder (quad-tree / Morton Z-order) for iPhone/Touch cover art:** Apple's recursive-ordered dither format used by profiles 3001/3002/3003 (256×256, 128×128, 64×64). Pixels stored in Morton Z-order (interleaved bit pattern) rather than raster scanline. Decoder de-deranges via MortonInterleave, then decodes as standard RGB555→BGRA8. Encoder (`EncodeReorderedRgb555`) reorders BGRA8→RGB555 via Morton Z-order for writing iPhone-compatible .ithmb files.
- **Format 3004 SlotSize:8192 added** — libgpod's Itdb_ArtworkFormat `padding` field confirmed 8192 bytes slot padding for iPhone/Touch photo thumbnails (profile 3004, 56×55 Rgb555).
- **Format 3005 (320×320 Rgb555) added** — iPhone/Touch cover art variant from libgpod's `ipod_touch_1_cover_art_info` table. (53→54 active).
- **Full libgpod comparison completed:** All 42 overlapping format IDs verified identical. REC_RGB555 decoder for 3001-3003, 3004 SlotSize, 3005 profile added.
- **linux-x64 CI build workflow:** `.github/workflows/build-linux.yml` — validates both Release and Debug builds on push/PR.
- **NuGet dependency lockfile:** `packages.lock.json` enabled for the test project — locked restore (`--locked-mode`) ensures reproducible builds.
- **Dependabot configuration:** `.github/dependabot.yml` for weekly NuGet and GitHub Actions dependency updates.
- **SSSE3/ARM64 NEON SIMD for CLCL nibble-chroma and CL per-pixel chroma decoders** — 8 pixels/iteration via pshufb (SSSE3) / VectorTableLookup (NEON), matching the existing UYVY SIMD pattern. CLCL: ~454→192 µs (2.4×), CL: ~589→196 µs (3.0×). Both dispatch only when w%8==0 (fallback to scalar).
- **Reproducible decoder benchmarks** — `tools/IthmbCodec.Benchmark/` with BenchmarkDotNet, all 7 decoders at 720×480, MemoryDiagnoser, 3 warmup + 10 measurement iterations. CI workflow at `.github/workflows/benchmark.yml` (manual dispatch).
- **CI gate for README stats accuracy** — `tools/check-readme-stats.sh` verifies profile count (54), test count (547), and decoder count (7) match the codebase. Runs in build-linux.yml on every push.
- **Zero-dimension edge-case tests** — all 7 decoder formats tested on 0×4, 4×0, and 0×0 (all return false). Null outBuf guard tested for DecodeRawProfile (returns OK). (+8 tests)

### Changed

- **Profile system: Nano 7G override data deduplicated** — shared `Nano7GOverrides` field powers both `BuildProfileAlternates()` and `BuildDeviceOverrides()` instead of hardcoding the same values twice.
- **ProfilesJson.cs sorted by prefix ascending** — easier maintenance, no functional change.
- **Profile flexibility system:** Added `UseMhniDimensions` flag (use actual Width/Height from MHNI chunk instead of profile's fixed dimensions) and `FallbackEncodings[]` array (ordered list of alternative encodings on primary decode failure). Enabled on profile 1061 to resolve dimension disagreement.
- **PhotoDb/Core.cs tuple expanded:** WalkEntries and TryParsePhotoDb output now includes Width, Height from MHNI header (6-element tuple).
- **PROFILES.md updated:** 3001/3002/3003 encoding changed from RGB555 to Reordered RGB555. UseMhniDimensions and FallbackEncodings documented.
- **BT.601 YUV coefficients relocated:** `YuvRCoef`, `YuvGCoefCb`, `YuvGCoefCr`, `YuvBCoef` moved from `Rgb565Rgb555.cs` to `YuvUtils.cs`.
- **README sections reordered:** Performance promoted to after Architecture, Development/Contributions moved after Troubleshooting. Added decoder format reference table. Benchmark table updated with real CLCL/CL SIMD results.
- **BGR15 naming normalized:** All `BGR;15` occurrences (code comments, docs, README) changed to `BGR15` for consistency.
- **README intro restructured:** Removed duplicate Goal paragraph, replaced with Key Features bullet list (54 profiles, 7 decoders, SIMD, PhotoDB, cross-platform).

### Fixed

- **Padded-profile short-file blind spot** — when `IsPadded=true` and raw data was slightly shorter than `validSize`, neither trim path nor zero-pad path ran. Now zero-pads within tolerance.
- **Defense-in-depth: padded-profile guard `>`→`>=`** — ensures exact-size files don't slip through trim check.
- **Nano 5G/6G device profiles inverted** — Our Nano 5G had Nano 6G photo formats (1092/1093) and vice versa. Corrected per OrgZ IPodCapabilities table.
- **Nano 3G profile entirely wrong** — had 6 Nano 4G+ formats instead of 1060/1055/1061 per gnupod. Replaced.
- **Nano 4G missing 1055 and 1068** — gnupod confirmed both 128×128 variants required. Added. Removed 6 extra formats belonging to Nano 5G/6G.
- **Nano 1G/2G missing 1031 (42×42)** — gnupod/pygpod confirmed this album art thumbnail. Added to both.
- **Format 3009 dimensions swapped (160×120→120×160)** — Steee29/ithmb_converter from real iPhone 2G iOS 1.1.4 shows portrait. Added isPadded:true and slotSize:40960.
- **Mini 1G/2G artwork support removed** — Both iOpenPod and iOpenPod creator confirm `supports_artwork=False`. Removed 1024 and 1027 from Mini profiles.
- **Documentation audit: stale profile/test counts updated across all docs.** README, CHANGELOG, ACKNOWLEDGMENTS, what-is-this.md corrected: 49→53 profiles, 528→530 tests, 35+→33 implementations.
- **CLCL benchmark buffer size corrected:** Was `w*h+uvSize` (too small), changed to `w*h*2`.
- **Benchmark CI added `--filter "*"`** — BenchmarkDotNet entered interactive TUI mode without it, hanging the CI run.
- **Release-windows publish path:** Output was `native/` folder, publish step pointed to `publish/`.
- **build-linux.yml upload-artifact:** `path:` field was empty. Fixed to list output directories.
- **.gitignore purge (history-rewritten):** Removed 6 leaky blacklist entries (`.ruff_cache/`, `BenchmarkDotNet.Artifacts/`, `.omo/`, `HANDOVER.md`, `TestResults/`, `.codegraph/`) — all already covered by the whitelist pattern.

### Documentation

- **Stale counts/docs updated** — README/CHANGELOG/PROFILES/what-is-this.md test count 530→538, profiles 53→55 active. PROFILES.md 3009 dimension corrected. 1062 added to profile table.

### Test

- **Tautological assertions fixed:** 7 assertions in Fuzz.cs that always passed due to wrong variables. Extracted `MutateBuffer` helper, reduced iteration count 1000→300.
- **Memory leaks fixed:** `outBuf->Data` null-guarded free added in 18 `finally` blocks across 5 roundtrip test files.
- **Redundant SIMD test removed:** `Rgb565_Exhaustive_SIMD_Redundant` was a no-op copy of the scalar exhaustive test.
- **Empty test shells deleted:** `Roundtrip.cs` and `Statistical.cs` (empty partial stubs). Constants merged into `Statistical.Core.cs`.
- **Weak assertions hardened:** CL/CLCL/SwapChroma roundtrip now checks actual pixel values within ±8-16 tolerance. `DecodeYuv422_KnownColor` B assertion corrected (B≈0, was >220). `DecodeYcbcr420_NeutralChroma` now checks all channels + alpha for all 4 pixels.
- **Source bugs fixed (C-3, H-2, H-3, H-4):** DecodePipeline.cs crop bounds negative check, YCbCr420 pointer math overflow (`int`→`nint`), RotateBgra OOM corrupting output dimensions, Morton de-derange `uint`→`int` overflow.
- **CI configs fixed:** benchmark.yml lockfile restore, upload `if: always()`, test-neon.yml locked-mode, release-windows.yml if syntax. IthmbCodec.csproj `AnalysisLevel` case.
- **Fuzz_Corruption_RandomByteMutations assertions added:** Pixel validity checks gated on decoder return (void decoders always check; bool decoders skip when returning false for early exits).
- **Photo/cover art split corrected:** README claimed 22+32, actual is 25+29. All 547 tests pass.

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

- **`swapRgbChannels` support completed for RGB565:** Scalar and Tail decode paths now accept the `swapRgbChannels` parameter (matching existing SSE2/NEON coverage). Completes BGR15 support for all 5 DecodeRgb565 paths. (+0 new tests)
- **Padded row stride tests:** 3 new tests verify correct decode for padded (55×55 nominal, 56-pixel rows), unpadded square, and non-square padded dimensions through both SSE2 and Scalar paths.
- **Format IDs 1042, 1043, 3006, 3007:** Four additional built-in profiles — 1042 (320×240 RGB565, Classic photo alias for 1024), 1043 (130×88 RGB565, alias for 1015), 3006 (56×56 RGB555, iPod Touch cover art, slot-padded), 3007 (88×88 RGB555, slot-padded). Adds `SlotSize` field to `IthmbVariantProfile` for padded profiles. (+2 tests, 530 total)
- **CLI `--extract-all-pd` and `--list-devices` flags:** `--extract-all-pd` batch-decodes all entries in a PhotoDB/ArtworkDB to individual BMPs; `--list-devices` prints the 18-device format table to stdout.
- **PhotoDB inline JPEG blob detection:** `TryParsePhotoDb` post-processing detects entries with unknown format_id + FF D8 prefix and dispatches to `DecodeJpegSlice`. Covers PhotoDB entries where Apple stored JPEG data instead of raw pixel data. (+1 test)
- **Byte-level corruption fuzz test:** `Fuzz_Corruption_RandomByteMutations` — 1000 iterations, fixed seed 42, all 7 decoders at random 4-128 dims. Random mutations: 10% bit flip, 5% byte swap, 5% truncate, 80% clean. NativeMemory alloc/free, no try/catch. (+1 test, 530 total)
- **Reuhno CC0 synthetic test vectors:** 10-entry ArtworkDB + F1061/F1055/F1060 multi-frame .ithmb files + 30 reference PNGs + manifest.csv with SHA256 checksums. All 30 SHA256s verified against manifest. Committed to `samples/reuhno-reference/`. (+6 tests)
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

- **BGR15 channel-swapped RGB555 (`SwapRgbChannels`):** Added `SwapRgbChannels` bool parameter to `IthmbVariantProfile` and `profiles.json` parser, new `swapRgbChannels` JSON field. When true, the RGB555 decoder reads `xBBBBBGGGGGRRRRR` (BGR15) layout for iPhone 2G thumbnail compatibility. Applied to all 5 decoder paths (Tail, Scalar, SSE2, NEON, public DecodeRgb555 entry point) and encoder (`EncodeRgb555`, `BuildIthmbFile`). SIMD uses a conditional branch outside the pixel loop (zero overhead on the hot path). (+3 tests, 517 total)
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

[Unreleased]: https://github.com/B67687/Ithmb-Codec/compare/v1.9.4...HEAD
[1.9.4]: https://github.com/B67687/Ithmb-Codec/compare/v1.9.3...v1.9.4
[1.9.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.6.0...v1.9.0
[1.6.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.1.0...v1.3.0
[1.1.0]: https://github.com/B67687/Ithmb-Codec/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/B67687/Ithmb-Codec/releases/tag/v1.0.0
