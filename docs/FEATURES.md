# FEATURES.md: Standing Feature & Behavior Inventory

> **Backfill note:** this inventory was created at the REVIEW gate (August 2026) for a codebase that shipped before the inventory discipline landed. Every feature below is `applied` (implemented, tested, spec-synced). Tests anchor to features via the Test Anchoring tables; per-test F-### tags are the forward convention. FR-## traceability added to SPECIFICATION.md (§16).

## Lifecycle

```
proposed -> approved -> applied -> archived
```

| Status | Meaning | Can be shipped? |
| --- | --- | --- |
| `proposed` | Intended, not yet ratified into V1 | No |
| `approved` | In V1 scope (IN SCOPE, RULES section 5) | No: needs `applied` |
| `applied` | Implemented, tests anchored, spec-synced | Yes |
| `archived` | Removed/superseded; entry kept for history | No |

## Features

### F-001: Format identification by 4-byte prefix

- **Status:** applied
- **Reviewed:** 2026-08-27 (review cadence: 6 months)

**Behavior Contract:** Preconditions: input is a complete file buffer. Postconditions: the first 4 bytes (big-endian) resolve to a known profile or a typed Unsupported error. Invariants: unknown prefixes never silently decode. Error cases: unknown prefix with no JPEG returns `DecodeError::Unsupported`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/edge_cases.rs` | unknown-prefix error path |
| `crates/ithmb-core/tests/proptest.rs` | prefix parsing properties |
| `fuzz/fuzz_targets/fuzz_decode_ithmb.rs` | arbitrary-byte prefix handling |

### F-002: RGB565 decode

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | exhaustive 65,536-value roundtrip |
| `crates/ithmb-core/tests/golden_comparison.rs` | C# reference parity |

### F-003: RGB555 decode

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | exhaustive 32,768-value roundtrip |
| `crates/ithmb-core/tests/golden_comparison.rs` | C# reference parity |

### F-004: Reordered RGB555 decode

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | roundtrip |
| `crates/ithmb-core/tests/synthetic_vectors.rs` | synthetic vector decode |

### F-005: UYVY decode (including interlaced)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4; SIMD and scalar paths agree. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | roundtrip |
| `crates/ithmb-core/tests/simd_tail.rs` | SIMD/scalar tail agreement |

### F-006: YCbCr420 decode

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4; SIMD and scalar paths agree. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | roundtrip |
| `crates/ithmb-core/tests/simd_tail.rs` | SIMD/scalar tail agreement |

### F-007: CLCL decode (nibble chroma)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4; SIMD and scalar paths agree. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | exhaustive 15,625 nibble-combo roundtrip |
| `crates/ithmb-core/tests/simd_tail.rs` | SIMD/scalar tail agreement |

### F-008: CL decode (per-pixel chroma)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: frame payload with prefix stripped. Postconditions: BGRA8 pixels at profile dimensions. Invariants: output length == width x height x 4; SIMD and scalar paths agree. Error cases: truncated payload returns `BufferTooShort`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | roundtrip |
| `crates/ithmb-core/tests/simd_tail.rs` | SIMD/scalar tail agreement |

### F-009: JPEG-embedded (T-prefix) decode

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: file starts with JPEG SOI (FF D8) followed by JFIF or Exif within 512 bytes. Postconditions: JPEG payload extracted (SOI to EOI), decoded via zune-jpeg, EXIF orientation (0x0112) applied. Invariants: JPEG SOI must be within the first 4 MB. Error cases: corrupt JPEG returns `DecodeError::Jpeg`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/golden_comparison.rs` | T-prefix golden vectors |
| `crates/ithmb-core/tests/edge_cases.rs` | corrupt JPEG error path |

### F-010: Embedded JPEG carving fallback

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: unknown prefix, no leading JPEG. Postconditions: byte-level SOI to EOI scan recovers an embedded JPEG if present. Invariants: carving never runs before the size guard. Error cases: no JPEG found returns `DecodeError::Unsupported`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/edge_cases.rs` | carving fallback |
| `crates/ithmb-core/tests/properties.rs` | carving properties |

### F-011: PhotoDB/ArtworkDB container open

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: input starts with `mhfd` magic. Postconditions: chunk tree (MHFD, MHSD, MHNI) parsed; entries extracted with inline pixel data or external file references. Invariants: chunk offsets validated against buffer length before reads. Error cases: invalid chunk structure returns `DecodeError::InvalidFormat`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | PhotoDB roundtrip write and integrity |
| `fuzz/fuzz_targets/fuzz_parse_photodb.rs` | chunk-tree parsing fuzz |

### F-012: Device profile resolution

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: a device name is supplied to `open_ithmb`. Postconditions: the 18-device lookup table filters or selects the matching profile set. Invariants: unknown device names fall back to the full profile set. Error cases: none (fallback is silent by design).

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/profile_validation.rs` | device-specific format tables |
| `crates/ithmb-core/tests/edge_cases.rs` | unknown device fallback |

### F-013: Post-process transforms (swap, crop, rotation)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: a decoded frame plus a `TransformConfig`. Postconditions: channel swap, crop, and rotation applied in fixed order (swap, crop, rotate). Invariants: transform order never changes. Error cases: invalid crop geometry returns `DecodeError::Profile`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | rotation roundtrip |
| `crates/ithmb-core/tests/properties.rs` | transform properties |
| `fuzz/fuzz_targets/fuzz_decode_pipeline.rs` | transform-path fuzz |

### F-014: File size guard

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: input buffer length known. Postconditions: files over 8 MB are rejected before any allocation. Invariants: the guard never shrinks below the largest known frame (810 KB). Error cases: oversized input returns `DecodeError::FileTooLarge { size, limit }`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/edge_cases.rs` | oversized-input rejection |
| `crates/ithmb-core/tests/alloc_contract.rs` | allocation contract |

### F-015: Cooperative cancellation

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: a valid `AtomicBool` is passed. Postconditions: decode stops at the next checkpoint (every 64 KiB) and returns `DecodeError::Canceled`. Invariants: cancellation never corrupts state (decode is pure). Error cases: canceled returns `DecodeError::Canceled`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/cancellation.rs` | cancellation behavior |
| `crates/ithmb-core/tests/concurrency.rs` | cancellation under concurrency |

### F-016: Synthetic encoders (7 formats)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: BGRA input at known dimensions. Postconditions: valid .ithmb frame bytes for the target format. Invariants: encoder output roundtrips through the matching decoder. Error cases: invalid dimensions return an encoder error.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/roundtrip.rs` | encode/decode roundtrip per format |
| `fuzz/fuzz_targets/fuzz_encode_roundtrip.rs` | encode roundtrip fuzz |

### F-017: C ABI

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: valid pointers and lengths from the C caller (feature `c`). Postconditions: decoded BGRA buffer or error code returned. Invariants: no panics cross the FFI boundary; `ithmb_decode` and `ithmb_prefix_to_profile` keep their names and signatures. Error cases: invalid input returns an error code.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/c_api_test.rs` | FFI contract |
| `.github/workflows/ci-full.yml` (build_c_api job) | `nm -D` symbol verification |

### F-018: WASM bindings

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: valid input bytes from JavaScript. Postconditions: decoded image data returned to JS (`decode_ithmb`, `peek_prefix`, `get_encoding_name`). Invariants: no panics cross the wasm-bindgen boundary. Error cases: invalid input returns an error string.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-wasm/tests/wasm_runtime.rs` | wasm-pack runtime smoke tests |

### F-019: Python bindings

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: valid bytes input (PyO3, abi3-py312). Postconditions: dict with width, height, BGRA data, format, rotation returned. Invariants: no panics cross the PyO3 boundary. Error cases: invalid input raises an exception carrying the DecodeError detail.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `pymod/tests/test_basic.py` | basic decode from Python |
| `pymod/tests/test_runtime.py` | runtime behavior |

### F-020: CLI tool

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: valid input path. Postconditions: decoded output file or metadata printed (decode, `--info`, `--list-profiles`, `--frame`, `--raw`, `--open`, `--frame-count`, `--extract-all`). Invariants: `anyhow::Result` at the boundary; DecodeError converted to a message. Error cases: unreadable input prints an error and exits non-zero.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-cli/tests/cli.rs` | end-to-end binary behavior |

### F-021: SIMD acceleration

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: feature detection has selected the active ISA (SSE2/AVX2/NEON). Postconditions: correct BGRA output for the selected ISA. Invariants: scalar fallback always available; unsafe confined to `simd/`. Error cases: none (fallback is silent).

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/simd_constants.rs` | SIMD constant correctness |
| `crates/ithmb-core/tests/simd_tail.rs` | SIMD/scalar tail agreement |
| Miri (21 tests) | unsafe-kernel UB verification |

### F-022: Multi-frame F-prefix support

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: an F-prefix file with concatenated raw frames. Postconditions: frame count detected from file size; individual frames accessible by index. Invariants: out-of-range indices return an error. Error cases: out-of-range frame returns `DecodeError::InvalidFormat`.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/edge_cases.rs` | multi-frame decode |
| `crates/ithmb-core/tests/synthetic_vectors.rs` | multi-frame synthetic vectors |

### F-023: LRU raw file cache (feature `cache`)

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: feature `cache` enabled. Postconditions: decoded raw files cached with bounded size. Invariants: cache never returns stale or corrupt entries. Error cases: cache misses fall through to decode.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/cache_concurrency.rs` | cache behavior under concurrency |

### F-024: Golden-vector parity with the C# reference

- **Status:** applied
- **Reviewed:** 2026-08-27

**Behavior Contract:** Preconditions: a golden vector file and its reference PNG. Postconditions: decoder output matches the reference pixel for pixel. Invariants: a golden-vector regression blocks the release (circuit breaker). Error cases: mismatch fails the test with a diff report.

**Test Anchoring:**

| Test file / name | Covers |
|---|---|
| `crates/ithmb-core/tests/golden_comparison.rs` | 14 golden vectors |
| `crates/ithmb-core/tests/ratified_divergences.rs` | documented, ratified divergences |

## Relationship to other artifacts

| Artifact | Role | Static or living? |
| --- | --- | --- |
| **SPECIFICATION.md** | The plan-IS-spec: how the system is built | Static (locked) |
| **FEATURES.md** | The reference of what exists + how it behaves | **Living** |
| **docs/PROJECT_MODEL.md** | Whole-project state machine (valid transitions, invariants) | Living |
| **EXPLAINER.md** | Code explainer for the owner | Living |
| **Tests** | Prove the contracts | Living |

SPECIFICATION says how it is built; FEATURES says what it does and what "working" means. They describe the same system at different levels: FEATURES is the differential that stays true as the code evolves.