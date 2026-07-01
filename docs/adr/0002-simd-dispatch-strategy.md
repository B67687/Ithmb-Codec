# ADR-0002: SIMD Dispatch Strategy

**Status**: Accepted (2026-06-10), updated v1.6.0 (AVX-512)
**Context**: .ithmb decoders need to process 8+ pixels per iteration for acceptable
performance. C# Native AOT supports x64 SSE2/SSSE3/AVX-512 and ARM64 NEON.

## Decision

Use a cascading dispatch ladder at the per-decoder level:

```
if Avx512BW.IsSupported && width >= 32 → AVX-512 path  (32 px/iter)
else if Sse2.IsSupported && width >= 8  → SSE2 path      (8 px/iter)
else if AdvSimd.IsSupported && width>=8 → NEON path       (8 px/iter)
else                                     → scalar path    (1 px/iter)
```

Each decoder file contains all ISA specializations for that format.
The scalar path is always the reference implementation.

## Consequences

- **Positive**: Each ISA's code is isolated. Adding a new ISA doesn't touch existing paths.
- **Positive**: Scalar path serves as the verified reference — SIMD identity tests
  compare against it byte-for-byte.
- **Positive**: Compile-time conditionals (`IsSupported`) are resolved to constants
  by Native AOT ILC, so dead branches are removed.
- **Negative**: Code duplication across ISAs for the same algorithm.
  Mitigated by SIMDConstants.cs and parameterized inner loops where possible.
- **Negative**: Tail (width < 8/32) must be handled separately for each ISA.

## Alternatives Considered

| Approach | Why rejected |
|----------|-------------|
| Single SIMD loop with `Vector<T>` | Inconsistent width across platforms, no AVX-512 access |
| Intrinsics per file, shared via interface | Interface dispatch cost in hot loop defeats SIMD purpose |
| C++/CLI SIMD via P/Invoke | Native AOT cannot load mixed-mode assemblies |
