# ADR-0002: SIMD Dispatch Strategy

**Status**: Revoked (2026-07-04) — replaced by Rust `core::arch` feature detection.

**Context**: .ithmb decoders need to process 8+ pixels per iteration for acceptable
performance. The C# prototype used Native AOT with x64 SSE2/SSSE3/AVX-512 and ARM64 NEON.

## Decision (Rust)

Use `core::arch` feature detection at the dispatch level, with scalar always
available as the verified reference:

```rust
if is_x86_feature_detected!("avx2") && width >= 16 → AVX2 path   (8 px/iter)
else if cfg!(target_arch = "aarch64")                → NEON path    (8 px/iter)
else if is_x86_feature_detected!("sse2") && width>=8 → SSE2 path   (4-8 px/iter)
else                                                   → scalar path (1 px/iter)
```

Key differences from the C# approach:

- **No AVX-512**. We had it, removed it — Intel frequency downclock made the
  256-bit AVX2 path 34× slower than scalar for simple pixel unpacking (RGB565,
  RGB555). Even YUV-conversion SIMD uses SSE2 only.
- **SIMD only for YUV-heavy decoders** (UYVY, YCbCr420, CL/CLCL, grayscale fill).
  RGB565/RGB555 use LLVM's auto-vectorized scalar loop which outperforms hand-written
  SSE2/AVX2 by avoiding function-call overhead and port-5 serialization.
- **No per-file ISA splitting**. All SIMD lives in `simd.rs` grouped by ISA
  module (`mod avx2`, `mod neon`, `mod sse2`), with dispatch functions that
  select the best available path at call time.

## Consequences

- **Positive**: Single-file SIMD surface — easier to audit (Miri on `unsafe` blocks).
- **Positive**: Scalar path is always the reference, verified by identity tests
  against SIMD output (15,625 nibble combos for CL, 65k values for RGB565).
- **Positive**: Neon on aarch64 is unconditional (no runtime detection needed).
- **Negative**: SIMD code duplication across ISAs for the same algorithm.
- **Negative**: Tail pixels (< 4/8) handled by scalar fallthrough for each ISA.
- **Negative**: AVX2 disabled on Intel CPUs due to frequency downclock — 128-bit
  SSE2 is the fastest x86 path for this workload.

## Historical Note

The original C# dispatch used `Avx512BW.IsSupported` with 32-pixel batches and
`Vector128`/`Vector256` for SSSE3-packed arithmetic. The Rust port initially
replicated this but discovered the Intel frequency downclock issue through
benchmarks. The AVX2 code remains in `simd.rs` behind `#[cfg(feature = "simd")]`
but is never dispatched at runtime.
