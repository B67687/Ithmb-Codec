# Shift Log

Learning shifts recorded per RULES.md section 5. Up to 5 shifts per project; after 5, consider a fresh cycle. Each shift is a documented discovery that changed direction, not a failure.

```
LEARNING SHIFT
  What we learned: The C# Native AOT plugin (B67687/Ithmb-Codec-CSharp) worked but was
    trapped inside the Windows-only ImageGlass plugin ABI and could not ship as a
    crates.io library, PyPI wheel, or WASM module.
  Decision: Rewrote the codec as a pure Rust workspace (ithmb-core + CLI, WASM, Python,
    C ABI wrappers) with the C# repo archived as the authoritative algorithm reference.
  Cost: A full rewrite plus a parity gap that took 3 waves of quality work to close
    (docs/EVOLUTION.md).
  What this enables: One codebase reaches crates.io, PyPI, the browser, and any C-FFI
    language, with SIMD control and zero-cost FFI that the C# path could not offer.
```

```
LEARNING SHIFT
  What we learned: .ithmb files are headerless blobs keyed by a 4-byte format prefix,
    and a single prefix can map to multiple resolutions and encodings. Guessing the
    format from bytes alone risks silent misdecodes.
  Decision: Replaced ad-hoc format detection with a static profile database of 53
    built-in profiles (derived from iOpenPod's empirically validated set), resolved by
    prefix with a data-size heuristic as a tiebreaker.
  Cost: A curated profile table that must be maintained as new formats surface.
  What this enables: Deterministic, hardware-validated decode for every known profile
    instead of best-effort guessing.
```

```
LEARNING SHIFT
  What we learned: The codec decodes untrusted files from photo libraries; the largest
    known real frame is 810 KB, but pathological input could force unbounded allocation.
  Decision: Added an 8 MB file size guard enforced before any allocation (ADR-0005),
    roughly a 10x safety margin over the largest observed frame.
  Cost: A hypothetical legitimate file over 8 MB would be rejected (theoretical: the
    iPod firmware caps files at ~500 MB).
  What this enables: OOM/DoS resistance on untrusted input without meaningful loss of
    real-world coverage.
```

```
LEARNING SHIFT
  What we learned: As a library meant for embedding (WASM, embedded, FFI), shipping with
    all dependencies enabled bloated the default build and the audit surface.
  Decision: Moved to an empty default feature set with optional features (cache, metrics,
    c, logging) that consumers enable explicitly.
  Cost: Feature-flag combinations add a small testing surface; consumers must opt in.
  What this enables: A lean default dependency footprint and a smaller cargo-deny and
    cargo-audit surface for the common case.
```

```
LEARNING SHIFT
  What we learned: Real .ithmb files do not always arrive standalone: they live inside
    Apple's PhotoDB/ArtworkDB containers (mhfd chunk trees), and users want to extract
    thumbnails from the container directly.
  Decision: Added the photodb/ module: a chunk-tree parser, writer, and integrity
    checker, exposed through open_ithmb and the CLI --open flag.
  Cost: A second parsing surface (chunk tree with endianness handling) plus its fuzz
    target (fuzz_parse_photodb).
  What this enables: Whole-library recovery from a PhotoDB in one command, not just
    single-file decode.
```