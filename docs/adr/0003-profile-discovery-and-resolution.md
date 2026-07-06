# ADR-0003: Profile Discovery and Resolution

**Status**: Accepted (2026-06-12)
**Context**: The .ithmb format has no central registry. Format IDs (prefixes) and
their dimensions/encodings must be discovered from 20+ open-source implementations.

## Decision

Three-tier profile architecture:

1. **Embedded canonical source**: 53 profiles compiled as a JSON string literal
   into the assembly (`ProfilesJson.cs`). Parsed at module init into a
   `FrozenDictionary<IthmbVariantProfile>`.
2. **External override**: `profiles.json` in the application directory loaded
   at runtime, merged into `KnownProfiles` via `Interlocked.Exchange`.
3. **Dynamic resolution**: Device-specific overrides, data-size heuristics,
   and Nano 7G workarounds in `ProfileSystem.cs`.

## Consequences

- **Positive**: No database, no network, no config files required for normal use.
- **Positive**: External profiles can be updated independently of the binary.
- **Positive**: Thread-safe reload via atomic dictionary swap.
- **Negative**: Profiles discovered by manual survey of 22 repositories.
  Each new device requires updating the embedded JSON and re-compiling.
- **Negative**: Embedded JSON is parsed by custom parser (no `System.Text.Json`
  in Native AOT), adding maintenance surface.
- **Negative**: No automated diff against upstream sources — profiles
  can drift from canonical implementations.

## Alternatives Considered

| Approach | Why rejected |
|----------|-------------|
| SQLite database | Increases deployment size, Native AOT SQLite interop complexity |
| Remote registry server | Offline use case, single-user tool |
| Load all from external JSON at startup | Boot failure if file missing. Embedded fallback required anyway. |

### Rust Port Notes

The Rust implementation ported this design as follows:
- `ProfilesJson.cs` → `src/profile_parser.rs` (JSON parsing)
- `FrozenDictionary<i32, IthmbVariantProfile>` → `HashMap<i32, Profile>` in `src/profile_db.rs`
- `ProfileSystem` → `src/profile_db.rs` (in-memory store + external override)
- `IthmbVariantProfile` → `src/profile.rs` (Profile struct)
- External profile override via `ProfileDb::load_external()`
- No runtime profile caching needed — `OnceLock<ProfileDb>` initialized on first use
- Profile data embedded via `include_str!("../data/profiles.json")` instead of C# resource files
- Custom JSON parser ported from C# Native AOT requirement (no `serde_json` dependency)
