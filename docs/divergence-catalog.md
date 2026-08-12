# Divergence Catalog — Rust `ithmb-codec` vs archived C# reference

**Date:** 2026-08-11 · **Author:** Sisyphus-Junior (analysis task, read-only)
**Rust:** `/home/nami/projects/dev/Ithmb-Codec` (branch `main`, crates/ithmb-core)
**C#:** `/home/nami/projects/dev/ithmb-codec-csharp` (archived, read-only — not built/run)

**Method:** module-by-module behavior diff of both source trees (not text diff), cross-checked against
prior session plans (`.omo/plans/rust-csharp-parity.md`, `rust-csharp-parity-fix.md`), ADRs
(`docs/adr/0003`, `0005`, `adr/csharp/*`), embedded profile tables, and deviation comments in the Rust source.

**Classification legend**
- **ALLOWLISTED-BUGFIX** — Rust intentionally fixes a C# bug/security hole (evidence cited).
- **ALLOWLISTED-IMPROVEMENT** — deliberate, documented better behavior.
- **EQUIVALENT** — same behavior, different code (dropped from the shortlist; listed once for traceability).
- **UNRESOLVED** — behavior differs with no recorded decision; needs user ratification.

> **Working-tree note:** the Rust tree was **not clean at session start** (6 pre-existing modified files:
> `c_api.rs, cache.rs, error.rs, metrics.rs, profile.rs, tests/properties.rs` — 647 insertions) and more
> files were modified/added by concurrent activity during analysis (17 modified + 5 untracked at write time,
> e.g. `tests/proptest.rs`, `fuzz/fuzz_targets/*`). **No write to any source file was made by this task**;
> the only artifact created is this report. Baseline `git status` snapshots at start/end are in `.omo/`.

---

## 1. Profile database (data/profiles.json vs `IthmbCodecPlugin.ProfilesJson.cs`)

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Profile count | **54 active** profiles, incl. prefix **1044** (128×128 RGB565, 32768 B) | **53 active**; 1044 present but **commented out** — disabled because writing it to iPod Classic corrupts cover art (iOpenPod #81) | **RESOLVED** | Rust `data/profiles.json:22` vs C# `ProfilesJson.cs:60-62` | Rust re-enabled a format C# deliberately disabled. **RESOLVED:** 1044 filtered in `profile_db.rs::load_builtin` via `DISABLED_PREFIXES`; 53 active; JSON entry kept. |
| Remaining 53 entries | Identical prefix/width/height/encoding/frame_byte_length + flags (1013 BE+rot90, 1019 interlaced, 1020 swapsDim+BE, 1023 BE, 1061 useMhni, 1067 padded ycbcr420, 1081 fallback jpeg, 2002/2003 BE, 3004/3006/3007/3009 padded+slot, 3001-3003 reordered) | Same values | EQUIVALENT | `profiles.json:2-56` vs `ProfilesJson.cs:20-130` | Field names differ (`frame_byte_length` vs `frameBytes`) — same semantics. |
| Default byte order | `little_endian: true` default | `LittleEndian = true` default | EQUIVALENT | `profile.rs:128` vs `VariantProfile.cs:30` | Both little-endian by default; only 1013/1020/1023/2002/2003 override. |
| Profile fields | Prefix, Width, Height, Encoding, FrameByteLength, SwapsDimensions, LittleEndian, IsPadded, IsInterlaced, ClclChroma, SwapChromaPlanes, ClChroma, SwapRgbChannels, Rotation, CropX/Y/W/H, SlotSize, UseMhniDimensions, FallbackEncodings | Same 19 fields | EQUIVALENT | `profile.rs:53-117` vs `VariantProfile.cs:27-39` | Crop representation identical (x/y/w/h), Rust as 4 i32s. |
| JSON parser DoS caps | No object-count cap found; ADR claims "caps at 100 objects" | `objectsRead++ > 100` (JsonParser.cs:28) + nesting depth cap 32 (:178) | **UNRESOLVED** | `profile_parser.rs` (no cap constant) vs `JsonParser.cs:28,178`; ADR-0003:113 | External `profiles.json` is user-controlled; Rust parser is unbounded (minor DoS surface). |

## 2. Profile resolution / overrides (`ProfileSystem.cs` vs `pipeline/mod.rs`, `pipeline/open.rs`)

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Resolution cascade | prefix lookup → **global** data-size heuristic (any profile, unknown prefix only) → JPEG carve → Unsupported | device override → **per-prefix** data-size heuristic (alternates) → global → JPEG carve | **UNRESOLVED** | `pipeline/mod.rs:190-229` vs `ProfileSystem.cs:123-158` | Rust heuristic matches ANY profile by frame size for an unknown prefix → can mis-decode an unknown file as a different-format profile. C# heuristic only considers alternates of the same prefix. |
| Nano 7G device overrides (1013→50×50, 1015→58×58, 1016→57×57) | **Not implemented** — prefix known → global profile (220×176 / 130×88 / 140×140) → decode fails on 5000/6728/6498-byte frames | Implemented via device override + alternates (5000/6728/6498 B) | **RESOLVED** | `profile.rs`/`profiles.json:6,8,12` vs `ProfileSystem.cs:88-98,163-182` | Real Nano 7G cover-art files decode in C# but fail in Rust. **RESOLVED:** `ProfileDb.alternates` + `resolve(prefix, data_len)` (256 B tolerance) wired into `decode_ithmb_inner` + `open.rs`. |
| `device_name` semantics | Filters PhotoDB entries by device's format-ID list; **unknown device ⇒ empty allowlist ⇒ skips all entries** (docstring says the opposite) | Used only for profile-override lookup (Nano 7G) | **UNRESOLVED** | `open.rs:80-95` vs `ProfileSystem.cs:126-132` | Rust doc-vs-code contradiction at open.rs:28-32; C# never filters by device. |
| Fallback encodings chain | Wired (retries each fallback; JPEG fallback inside `jpeg::decode`) | Wired (TryDecode loop + explicit JPEG-marker check) | EQUIVALENT | `pipeline/mod.rs:300-318` vs `DecodeRawProfile.cs:105-127` | Added by parity-fix P1. |
| `use_mhni_dimensions` | Overrides w/h from MHNI entry | Same (overrideW/overrideH) | EQUIVALENT | `open.rs:107-116` vs `DecodePipeline.cs:203-211` | Added by parity-fix P1. |
| External profiles.json loading | Explicit `ProfileDb::load_external()` (library API); no auto-discovery | Auto-loads `<AppDir>/profiles.json` at init, FNV-1a + SHA-256 logged, merged built-in-first | ALLOWLISTED-IMPROVEMENT | `profile_db.rs:43` + ADR-0003:148 vs `ProfileSystem.cs:32-78` | Documented deliberate difference (ADR-0003 negative consequence). |

## 3. PhotoDB / ArtworkDB parsing (`PhotoDb/Core.cs`, `Types.cs` vs `photodb/parser.rs`, `types.rs`)

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Chunk magics + endianness | LE-canonical u32 magics; LE/BE detected from MHFD magic; endian-dispatched readers | Identical magics; `DetectEndianness` 0/1/-1; same readers | EQUIVALENT | `photodb/types.rs:10-58` vs `PhotoDb/Types.cs:15-72` | Both canonical-LE u32; BE files byte-swap to the same canonical value. |
| Tree-walk depth limit | 64 | 64 | EQUIVALENT | `photodb/parser.rs:21` vs `Core.cs:140` | |
| MHNI inline/external split | `ithmb_offset>=0 && image_size>0` → inline; `ithmb_offset==-1` → external | Same logic | EQUIVALENT | `parser.rs:276-284` vs `Types.cs:256-277` | Added by parity-fix P4 (external-ref API). |
| MHNI width/height offsets | width @ +34, height @ +32 (LE u16) | Same (iPod Classic 76B) + extra **Apple TV/Animal packed w/h @ +20** variant detection | **UNRESOLVED (low confidence)** | `parser.rs:587-588` vs `Types.cs:275-276` | Rust MHNI struct not verified to include the packed Apple TV variant; no real sample exists either way. |
| JPEG auto-trim of non-profile entries | No parse-time trim; JPEG fallback in open.rs passes full blob to jpeg-decoder (stops at EOI) | Trims to EOI during parse for entries not in KnownProfiles | EQUIVALENT | `open.rs:131-139` vs `Core.cs:99-117` | Net behavior same (trailing bytes ignored). |

## 4. Row-format decoders

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| RGB565 / RGB555 | 16/15-bit unpack, LE/BE via `little_endian`, `swap_rgb_channels`; MSB replication 5/6-bit; masks 0x1F/0x3F | Identical shifts/masks/MSB replication (`rShift=11,gShift=5`, `(v<<3)\|(v>>2)` etc.) | EQUIVALENT | `pixel_utils.rs:14-24`, `simd/rgb565.rs` vs `Rgb565Rgb555.cs:35,215-229,320,388-397` | Auto-vectorized scalar in Rust vs hand-SIMD in C# — same math. |
| ReorderedRGB555 decode | Little-endian default + Morton Z-order de-interleave (`morton_interleave`/`morton_inc_x`); LE read | Requires square power-of-2; Morton de-derange then `DecodeRgb555` | EQUIVALENT | `reordered_rgb555.rs:1-100` vs `Rgb565Rgb555.cs:415-457` | |
| UYVY | 4:2:2 packed, BT.601, interlaced `half=((h+1)/2)*rowStride`; rowStride = len/h | Identical | EQUIVALENT | `uyvy.rs` vs `UyvyYuv.cs:35,222-292` | |
| YCbCr 4:2:0 | Planar `w*h + ((w+1)/2)*((h+1)/2)*2`; nearest-neighbour upsampling; `swap_chroma_planes` | Same sizes/formula + swap | EQUIVALENT | `ycbcr420.rs:43-61` vs `DecodeFormatYcbcr420.cs:34-73` | Rust padded-slot trim happens implicitly (reads only `expected` bytes). |
| **CL** | **Planar `[Y…][CbCr…]`; byte = (Cr<<4)\|Cb** (high nibble = **Cr**) | **Interleaved `[CbCr][Y]` per pixel; high nibble = Cb, low = Cr** | **RESOLVED** | `cl.rs:1-24` + `enc/cl.rs:32` vs `DecodeFormatCl.cs:15-17,63-67` | Opposite layout AND swapped nibble roles. Both speculative; self-consistent roundtrips in each impl, but cross-impl incompatible. **RESOLVED:** Rust planar layout ratified as canonical; PIN test `u5_cl_planar_layout_pin` locks it. |
| **CLCL** | **Planar `[Y…][Cb…][Cr…]`, 1 nibble/px, odd pixel in high nibble** | **Interleaved `[CbCr][Y0][CbCr][Y1]` (4 B/2 px), shared macropixel chroma** | **RESOLVED** | `clcl.rs:3-28` + `enc/clcl.rs:11-23` vs `DecodeFormatClcl.cs:19-25,57-62` | Fundamental layout divergence; C# self-marks CLCL "SPECULATIVE". **RESOLVED:** Rust planar layout ratified as canonical; PIN test `u6_clcl_planar_layout_pin` locks it. |
| CL/CLCL encoders | Planar, match Rust decoders | Interleaved, match C# decoders | EQUIVALENT (per impl) | `enc/cl.rs`, `enc/clcl.rs` vs `Encoding.cs:192-257` | Roundtrips pass within each impl; divergence is cross-impl (see above). |

## 5. YUV conversion (BT.601)

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Coefficients + rounding | R=359, G_Cb=88, G_Cr=183, B=454; division = arithmetic `>>8` (rounds toward −∞) — **documented bit-exact vs C#** | Same constants; `>>8` | EQUIVALENT | `yuv.rs:3-4,14-15,20-30,62-67` vs `YuvUtils.cs:11-14`, `UyvyYuv.cs:123-133` | Deliberate alignment, documented in Rust doc-comments. |
| Forward transform (encoders) | Y=77/150/29, Cb=−43/−85/128, Cr=128/−107/−21 (>>8, +128) | Identical | EQUIVALENT | `enc/helpers.rs:17-40` vs `EncoderHelpers.cs:89-112` | |

## 6. JPEG embedded extraction

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Decoder library | `jpeg-decoder` crate (RGB→BGRA, A=255) | StbImageSharp (RGBA→BGRA) | EQUIVALENT | `jpeg.rs:59-111` vs `JpegDecode.cs:78-121` | |
| **Dimension cap** | **CWE-400 pre-check: `read_info()` then reject `w*h*11 > 256 MiB`** + `set_max_decoding_buffer_size` | **No cap at all** — `result.Width/Height` used directly | **ALLOWLISTED-BUGFIX** | `jpeg.rs:15-21,61-90` vs `JpegDecode.cs:92` | 166-byte SOF2-65535×65535 stream once aborted the process (~8 GiB alloc). Rust-only hardening. |
| **EXIF orientation** | Parses APP1→`Exif\0\0`→TIFF II/MM→0x002A→IFD0 tag 0x0112, then **rotates the pixels** (90/180/270 CW) | Same EXIF parser, but orientation goes into image-info **metadata only**; pixels never rotated | **ALLOWLISTED-IMPROVEMENT** | `jpeg.rs:113-122,138-239` vs `Helpers.cs:135-197`, `JpegDecode.cs:94` | **Output pixels differ**: Rust returns rotated, C# returns raw. Rust is standalone (no host); deliberate. |
| Carving validation | Requires SOI **and** JFIF/Exif within 512 B window; scan bounded by `jpeg_scan_limit` (4 MiB default) | Requires SOI + 3rd byte 0xFF + JFIF/Exif in 512 window; carving skipped > 8 MB (`MaxCarvingFileSize`) | ALLOWLISTED-IMPROVEMENT (window); **UNRESOLVED** (4 vs 8 MiB) | `pipeline/mod.rs:471-518` vs `JpegDecode.cs:45-67`, `DecodePipeline.cs:249-255` | JFIF/Exif validation = deliberate parity-fix P3 (stricter, avoids false positives). Carving extent 4 MiB (Rust) vs 8 MiB (C#) differs. |

## 7. Pipeline, guards, post-processing

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| File-size guard | `max_raw_file_size` default **8 MiB** (reject `FileTooLarge`) | `MaxDecodeFileSize` = **32 MB** | ALLOWLISTED-IMPROVEMENT | `config.rs:30`, `pipeline/mod.rs:176-181` vs `Helpers.cs:28`; ADR-0005 | Deliberate, documented (ADR-0005: 8 MB ≈ 10× margin on 810 KB max frame). |
| JPEG scan limit | `jpeg_scan_limit` 4 MiB | `PeekBufferSize` 4 MB (two-phase probe) | EQUIVALENT | `config.rs:31` vs `DecodePipeline.cs:99-126` | |
| Trailing padding tolerance | 256 B, zero-pad up to deficit; threaded via thread-local | 256 B, zero-pad up to deficit | EQUIVALENT | `decoder_helpers.rs:19-20,100-137` vs `Helpers.cs:36`, `DecodeRawProfile.cs:99-104` | parity-fix P4. |
| Cancellation interval | 65 536 bytes | `(i & 0xFFFF) == 0` (every 64 KiB) | EQUIVALENT | `config.rs:32` vs `JpegDecode.cs:43` | |
| **Post-process order** | swap → **crop → rotate** | **rotate → crop** (crop region references final orientation); swap applied to dims separately | **UNRESOLVED** | `pipeline/mod.rs:326-337` vs `DecodeRawProfile.cs:132-165`, `DecodePipeline.cs:205` | No built-in profile has both crop+rotation, so no current-data impact; differs for external profiles. |
| Rotation direction | 90/180/270 **CW**; unknown angles no-op | Same CW | EQUIVALENT | `pixel_utils.rs:46-104` vs `Helpers.cs:42` | |
| Multi-frame slicing | Core `decode_ithmb` = frame 0 only (tolerates extra bytes); CLI slices with `frame_size()` (slot override), `offset = 4 + n*slot` | Core `DecodeRawProfile` has `frameIndex` param, `frameStart = 4 + n*SlotSize`; pipeline counts frames via `FrameByteLength` | EQUIVALENT (capability); C# has internal FBL-vs-Slot inconsistency | `ithmb-cli/src/main.rs:144-176`, `profile.rs:153-159` vs `DecodeRawProfile.cs:24,52`, `DecodePipeline.cs:232-239` | Same stride math; C# computes count with FBL, slices with SlotSize (never undercounts in practice). |
| F/T prefix byte guard | None (prefix = raw BE i32; JPEG detected via SOI) | None — comment explains guard removed because it blocked own encoder output | EQUIVALENT | `pipeline/mod.rs:183-186` vs `DecodePipeline.cs:214-220` | |

## 8. C API vs ImageGlass plugin

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| ABI surface | Standalone C ABI (`c_api.rs`) with **CWE-787 undersized-buffer guard** (area-based w×h check before copy) | ImageGlass v10 Native-AOT plugin ABI; no equivalent generic guard | ALLOWLISTED-BUGFIX (no C# counterpart) | `c_api.rs:130-135` vs `IthmbCodecPlugin.cs:41-160` | Rust-only hardening; the C# host (ImageGlass) sizes buffers itself. |

## 9. Encoders (`Encoding.cs` vs `enc/`)

| Area | Rust behavior | C# behavior | Classification | Evidence | Notes |
|---|---|---|---|---|---|
| Reverse rotation in `build_ithmb_file` | `rev = (360 − rotation) % 360`; dims swap for 90/270 | `revRotation = (360 − Rotation) % 360`; dims from RotateBgra output | EQUIVALENT | `enc/mod.rs:76-104` vs `Encoding.cs:260-336` | parity-fix P2. |
| **`swaps_dimensions` in encoder** | **Not applied** (only rotation-based swap) | Applied (`fw=h, fh=w`) before encode | **RESOLVED** | `enc/mod.rs:76-104` vs `Encoding.cs:263` | Profile 1020 (swapsDim, rot 0) encodes differently. **RESOLVED:** `build_ithmb_file` swaps dims first (then rotation) per C# order; test `u8_encoder_honors_swaps_dimensions_profile_1020`. |
| **Reordered encoder byte order** | **Hardcodes `big_endian=true`**; Rust decoder reads **little-endian** → Rust encode→decode roundtrip byte-swapped | Passes `!LittleEndian` (=LE for built-ins), consistent with its decoder | **RESOLVED** | `enc/mod.rs:133` vs `Encoding.cs:304`; decoder LE at `reordered_rgb555.rs:45,78-97` | Golden vectors (C# encoder → Rust decoder) pass because both are LE; Rust's own reordered encoder was inconsistent with its own decoder. **RESOLVED:** `encode_bgra` passes `!profile.little_endian`; byte-exact roundtrip tests for both endiannesses. |
| Interlace / padding / BE prefix | Post-encode interlace; pad to FBL; BE prefix | Same | EQUIVALENT | `enc/mod.rs:144-159` vs `Encoding.cs:311-333` | |

---

## SUMMARY

### Counts by classification
| Classification | Count | Items |
|---|---|---|
| **ALLOWLISTED-BUGFIX** | 2 | JPEG CWE-400 dimension cap (§6); C API CWE-787 buffer guard (§8) |
| **ALLOWLISTED-IMPROVEMENT** | 4 | 8 MiB file guard vs 32 MB (ADR-0005); EXIF self-rotation (§6); stricter carve JFIF/Exif validation (§6); explicit external-profiles model (ADR-0003) |
| **EQUIVALENT** | ~24 | listed once in tables (profiles, photodb, RGB565/555, reordered decode, UYVY, YCbCr420, YUV math, tolerance, cancellation, rotation direction, interlacing, etc.) |
| **UNRESOLVED** | **6** | U3, U4, U7, U10, U11, U12 (see shortlist) |
| **RESOLVED** | **6** | U1, U2, U5, U6, U8, U9 (impl task 2026-08-12) |

### Divergence shortlist — status as of 2026-08-12 (✅ = RESOLVED)

| # | Divergence | Why it matters | Suggested default |
|---|---|---|---|
| U1 | Profile **1044 active in Rust** vs disabled in C# (iOpenPod #81) | Rust decodes files C# would carve/reject | ✅ **RESOLVED** — 1044 filtered in `profile_db.rs::load_builtin` (`DISABLED_PREFIXES`); 53 active; JSON entry kept |
| U2 | **Nano 7G overrides (1013/1015/1016 → 50×50/58×58/57×57) missing** | Real Nano 7G cover-art files fail to decode in Rust, decode in C# | ✅ **RESOLVED** — `ProfileDb.alternates` + `resolve(prefix, data_len)` (256 B tolerance); wired into `decode_ithmb_inner` + `open.rs` |
| U3 | Rust **global data-size heuristic** for unknown prefixes | Can mis-decode unknown file as wrong-format profile (C# is per-prefix only) | Ratify or restrict to prefix-alternates |
| U4 | `device_name` semantics: Rust filters entries (and skips all on unknown device — doc contradicts code); C# uses it for overrides | PhotoDB extraction differs by device-name input | Fix open.rs doc-vs-code; ratify filter design |
| U5 | **CL byte layout + nibble roles inverted** (Rust planar, high=Cr; C# interleaved, high=Cb) | Speculative format; outputs incompatible | ✅ **RESOLVED** — Rust planar layout ratified; PIN test `u5_cl_planar_layout_pin` locks it |
| U6 | **CLCL byte layout inverted** (Rust planar vs C# interleaved macropixel) | Speculative; C# self-marked untested | ✅ **RESOLVED** — Rust planar layout ratified; PIN test `u6_clcl_planar_layout_pin` locks it |
| U7 | **Crop vs rotation order** (Rust crop→rotate; C# rotate→crop) | No built-in profile affected; external profiles differ | Ratify Rust order |
| U8 | Rust encoder **ignores `swaps_dimensions`** | Profile 1020 encoding differs | ✅ **RESOLVED** — `build_ithmb_file` swaps dims first (then rotation); test `u8_encoder_honors_swaps_dimensions_profile_1020` |
| U9 | Rust reordered encoder **hardcodes big-endian** (decoder is LE) | Rust's own reordered roundtrip is byte-swapped | ✅ **RESOLVED** — reordered encoder passes `!little_endian`; byte-exact roundtrip tests for both endiannesses |
| U10 | Rust profile JSON parser **lacks the 100-object cap** ADR-0003 claims | External profiles.json DoS surface | Add cap or correct ADR |
| U11 | JPEG **carving extent 4 MiB vs 8 MiB** | Large unknown files with late JPEG carve in C#, not Rust | Ratify tighter limit |
| U12 | Apple TV **packed MHNI w/h variant** present in C#, unverified in Rust | No real sample; low impact | Verify or note as accepted gap |

### Recommended decision order
1. Ratify **ALLOWLISTED** items (6 total) as-is — all are documented, deliberate, security-positive.
2. ✅ **U5/U6** resolved — Rust planar layout ratified as canonical; locked by PIN tests `u5_cl_planar_layout_pin` / `u6_clcl_planar_layout_pin`.
3. ✅ **U1, U2, U9** resolved (silent behavioral differences on real data / self-roundtrip) — see shortlist notes. U8 also resolved (encoder parity).
4. **U3, U4, U7, U10, U11, U12** remain — polish/ratification items, no current-data impact.

### Sources consulted
Rust: `crates/ithmb-core/src/{pipeline/mod.rs, pipeline/open.rs, profile.rs, profile_db.rs, profile_parser.rs, decoder_helpers.rs, pixel_utils.rs, yuv.rs, rgb565.rs, rgb555.rs, reordered_rgb555.rs, uyvy.rs, ycbcr420.rs, cl.rs, clcl.rs, jpeg.rs, c_api.rs, config.rs, photodb/{parser.rs,types.rs}, enc/{mod.rs,cl.rs,clcl.rs,reordered.rs,helpers.rs}, data/profiles.json}`, `crates/ithmb-cli/src/main.rs`, ADRs `docs/adr/0003, 0005, adr/csharp/*`, `.omo/plans/rust-csharp-parity*.md`.
C#: `src/IthmbCodec/*.cs` (ProfilesJson, ProfileSystem, VariantProfile, DecodeRawProfile, DecodePipeline, DecodeInfrastructure, Rgb565Rgb555, UyvyYuv, YuvUtils, DecodeFormatCl, DecodeFormatClcl, DecodeFormatYcbcr420, JpegDecode, Helpers, Encoding, EncoderHelpers, SimdConstants, JsonParser, PhotoDb/{Core,Types,Serialization}).
