# ADR-0003: Profile Resolution and Discovery

**Status**: Accepted (2026-07-07)

**Context**: The `.ithmb` format has no central registry. Format IDs (4-byte big-endian prefixes) and their corresponding dimensions, pixel encodings, byte orders, and post-processing flags must be discovered from 20+ open-source implementations across 15 years of iPod reverse-engineering. A profile database encodes this knowledge so the codec can identify and decode any `.ithmb` file without network access.

The codec must handle:

1. **Known formats** — 54 profiles covering iPod Photo 4G through iPhone 2G and iPod Nano 7G, sourced from iOpenPod (TheRealSavi, 50+ empirically validated entries), libgpod (PhotoDB chunk parser), and hardware validation (iPod Classic 6G samples from Reuhno).
2. **Unknown variants** — Files with a format prefix not in the database, or files whose prefix is known but whose actual encoding differs by device firmware.
3. **Runtime overrides** — Advanced users who discover a new format variant or need to tweak parameters (crop, rotation, channel swap) without recompiling.
4. **AOT compatibility** — The C# prototype could not use `System.Text.Json` in Native AOT and required a hand-written JSON parser. The Rust port inherits the same constraint: if serde_json were used, the `simd` dependency would pull in heavy code and increase compile times for an operation that runs once at startup.

The C# prototype used a three-tier architecture: embedded JSON (53 profiles as string literal), external `profiles.json` override, and dynamic device-specific resolution in `ProfileSystem.cs`.

## Decision

Use a **three-tier profile architecture** that embeds 54 profiles in the binary, allows external override via `profiles.json`, and chains through fallback encodings when the primary decode fails.

### Tier 1: Embedded binary profiles

Profile data is stored as `data/profiles.json` in the `ithmb-core` crate and embedded at compile time via `include_str!`:

```rust
// src/profile_db.rs
pub fn load_builtin() -> Result<Self, DecodeError> {
    let json = include_str!("../data/profiles.json");
    let profiles = parse_profiles_json(json)?;
    let map: HashMap<i32, Profile> = profiles.into_iter().map(|p| (p.prefix, p)).collect();
    Ok(Self { profiles: map })
}
```

The database is initialized once into a `OnceLock<ProfileDb>` global:

```rust
// src/pipeline/profile_loader.rs
static PROFILE_DB: OnceLock<ProfileDb> = OnceLock::new();

pub(crate) fn get_db() -> &'static ProfileDb {
    PROFILE_DB.get_or_init(|| ProfileDb::load_builtin().expect("built-in profile DB is valid"))
}
```

Each profile is a `Profile` struct with 21 fields covering:

- **Identification**: `prefix` (4-byte big-endian format ID)
- **Dimensions**: `width`, `height`, `swaps_dimensions`, `use_mhni_dimensions`
- **Encoding**: `encoding` (RGB565, RGB555, ReorderedRGB555, Yuv422, Ycbcr420, Jpeg), plus flags for chroma layout variants (`clcl_chroma`, `cl_chroma`, `swap_chroma_planes`)
- **Byte layout**: `frame_byte_length`, `little_endian`, `is_padded`, `slot_size`, `is_interlaced`
- **Post-processing**: `rotation`, `crop_x`, `crop_y`, `crop_width`, `crop_height`, `swap_rgb_channels`
- **Fallback**: `fallback_encodings` (ordered list of alternative encodings to attempt on primary failure)

### Tier 2: External `profiles.json` override

At runtime, `ProfileDb::load_external()` reads a `profiles.json` file from an application-specified path and merges its entries into the database, overriding existing profiles by matching prefix:

```rust
pub fn load_external<P: AsRef<Path>>(&mut self, path: P) -> Result<(), DecodeError> {
    let data = fs::read_to_string(path.as_ref())?;
    let profiles = parse_profiles_json(&data)?;
    for p in profiles {
        self.profiles.insert(p.prefix, p);
    }
    Ok(())
}
```

The merge is prefix-keyed: an external profile with the same prefix as a built-in one replaces it entirely. This lets advanced users:

- Add support for previously unknown format IDs
- Adjust crop/rotation parameters for specific devices
- Toggle channel-swap or chroma plane ordering
- Override frame dimensions for padded slot profiles where the slot size is device-specific

No restart is required — the CLI calls `load_external` at startup before the first decode.

### Tier 3: Fallback encoding chain

When a profile is found but its primary encoding fails to decode (e.g., a format ID is correct but the actual pixel data uses a different encoding than the database expects), the codec tries the profile's `fallback_encodings` list in order:

```rust
let mut ok = try_decode(raw, pixels, w, h, profile.encoding, profile);
if !ok && profile.fallback_encodings.is_some() {
    for &fallback_enc in profile.fallback_encodings.as_ref().unwrap() {
        ok = try_decode(raw, pixels, w, h, fallback_enc, profile);
        if ok { break; }
    }
}
```

If all fallbacks fail and JPEG is in the fallback list, the codec checks for JPEG markers (0xFF 0xD8) in the raw frame data:

```rust
if profile.fallback_encodings.contains(&Encoding::Jpeg)
    && raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xD8
{
    // Decode as JPEG
}
```

This handles the known ambiguity of format 1081, where libgpod says JPEG while iOpenPod says RGB565 — the fallback chain tries RGB565 first, then falls through to JPEG if the raw output is invalid.

When no profile matches at all, the codec falls back to JPEG carving (byte-level SOI marker scan of the full file) before reporting failure.

### AOT-safe JSON parser

The custom parser at `src/profile_parser.rs` is a hand-written cursor-based JSON parser that understands only the field names and value types used by the profile database. It is not a general-purpose parser. Key properties:

- **No serde_json dependency** — avoids pulling in 100+ KB of serialization infrastructure for a ~5 KB JSON file parsed once at startup.
- **No `serde`** — avoids proc-macro compile time and binary bloat.
- **No `alloc`-only** — uses `Vec<Profile>` but avoids recursion (iterative object parsing).
- **DoS-limited** — caps at 100 objects to prevent CPU exhaustion from crafted JSON.
- **Unknown-field tolerant** — skips unrecognized keys via `skip_value()`, allowing schema evolution.
- **Error messages** — returns `DecodeError::Profile` with offset information for debugging.

The parser processes the embedded JSON in ~50 µs on modern hardware, well within startup budget.

### Profile data sources

The 54 profiles in `data/profiles.json` derive from:

| Source | Contribution |
|--------|-------------|
| iOpenPod (Savi) | 50+ format IDs, empirically validated across multiple iPod models (Nano, Classic, Touch). Primary dimension/encoding reference. |
| libgpod | PhotoDB format ID tables, padded slot handling, device-format mappings. |
| Keith's iPod Photo Reader (kebwi) | Original RE (2005), 13 decode methods, multi-frame confirmation, crop/rotation parameters. |
| clickwheel (dstaley) | C# ArtworkDB read/write, 40+ format IDs with byte order flags. |
| Hardware validation (Reuhno) | iPod Classic 6G samples (F1061, F1055, F1060) confirming BGR15 channel-swap and MSB replication. 30 reference PNGs. |

## Consequences

### Positive

- **Zero-config startup**: No database, no network, no config files required. The 54 profiles are always available.
- **Runtime extensibility**: External `profiles.json` can add or override profiles without recompiling — useful for community contributors who discover new variants.
- **Resilient decode**: The fallback encoding chain handles the known ambiguity cases (format 1081, swapped chroma planes) that would otherwise produce garbled output without explanation.
- **Compile-time validation**: The embedded JSON is parsed during `ProfileDb::load_builtin()`, tested by `load_builtin_has_54_profiles` — if the JSON is malformed, the test catches it immediately.
- **Thread-safe initialization**: `OnceLock` guarantees exactly-one initialization with no locks on the hot path. The database is read-only after init.
- **Stable across firmware**: The C# prototype proved that 53 (now 54) profiles cover all known iPod/iPhone firmware versions. The database is not expected to grow rapidly.
- **AOT-ready**: No reflection, no proc macros, no runtime code generation. The custom parser works identically in all Rust compilation modes.

### Negative

- **Manual survey process**: Each new device requires updating `profiles.json` and recompiling. There is no automated process to discover new profiles from upstream repositories.
- **No automated upstream diff**: Profiles can drift from iOpenPod/libgpod/clickwheel sources. A manual audit is needed periodically (see ADR-0004: Quarterly Audit Protocol).
- **Custom parser maintenance**: The hand-written JSON parser adds ~230 lines of maintenance surface that a library like `serde_json` would eliminate. However, removing the `serde` dependency saves ~50+ compile-time dependencies and reduces binary size.
- **External profile discovery path**: The `profiles.json` load path is application-specified. The CLI loads it from the current working directory, but library users must call `ProfileDb::load_external()` explicitly.
- **Frame size assumption**: The database assumes frame byte length is fixed per prefix. Multi-frame files where frames have different sizes are not supported (no such files are known to exist).

## Alternatives Considered

| Approach | Why rejected |
|----------|-------------|
| **SQLite database** | Increases deployment size, adds a C dependency via `rusqlite` (conflicts with `unsafe_code = "deny"` at workspace level), and requires complex setup for an embedded device database with 54 rows. |
| **Remote registry server** | Offline use case (iPod file recovery on an airplane, disconnected archival). Single-user tool that should not depend on network availability. |
| **Load all from external JSON at startup** | Boot failure if file is missing. An embedded fallback is required anyway — may as well make embedded the primary source. |
| **serde_json + serde** | Adds 50+ transitive dependencies, slows compile times (~3s for serde derive), and is unnecessary for a 5 KB fixed-schema JSON file parsed once. The custom parser is smaller, faster to compile, and safer (no proc macros). |
| **Compile-time profile generation via build.rs** | Equivalent to `include_str!` plus a custom parser, but introduces a build script that complicates cross-compilation and IDE support. `include_str!` is simpler and has no build-dependency footprint. |
| **Device-specific profile files per model** | Would require multiple files and device auto-detection logic. The single `profiles.json` merged with device-formats tables in `device_profiles.rs` achieves the same goal with less complexity. |

## References

- Profile database implementation: `crates/ithmb-core/src/profile_db.rs`
- Profile struct: `crates/ithmb-core/src/profile.rs`
- Custom JSON parser: `crates/ithmb-core/src/profile_parser.rs`
- Profile loader (OnceLock): `crates/ithmb-core/src/pipeline/profile_loader.rs`
- Device-specific overrides: `crates/ithmb-core/src/device_profiles.rs`
- External profiles mechanism: [README.md](../../README.md#profile-reference)
- Migration context: [EVOLUTION.md](../EVOLUTION.md#adr-3-54-built-in-profiles--external-profilesjson)
- C# profile architecture (predecessor): [ADR-0003](csharp/0003-profile-discovery-and-resolution.md)
