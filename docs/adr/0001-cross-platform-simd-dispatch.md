# ADR-0001: Cross-platform SIMD Dispatch

**Status**: Accepted (2026-07-07)

**Context**: The .ithmb codec spends most of its CPU time in YUV→BGRA color conversion for four decoders (UYVY, YCbCr 4:2:0, CLCL nibble-chroma, and CL per-pixel chroma). These conversions apply BT.601 matrix arithmetic per pixel — an ALU-bound workload that benefits from SIMD throughput of 4–8 pixels per iteration. The codec must run on x86_64 (Linux, macOS Intel, Windows), aarch64 (Linux ARM, macOS ARM), and 32-bit x86, with a scalar reference path as the correctness baseline.

The C# prototype used a cascading dispatch ladder (`Avx512BW.IsSupported → Sse2.IsSupported → AdvSimd.IsSupported → scalar`) resolved at `JIT` compile time via `IsSupported` constants. Rust has no JIT; dispatch must happen either at compile time (cargo feature gates + `#[cfg]`) or at runtime (`is_x86_feature_detected!`).

Additionally, benchmarks revealed that hand-written SSE2/AVX2 for simple pixel-unpack formats (RGB565, RGB555) was **34× slower** than LLVM's auto-vectorized scalar loop on Intel CPUs due to AVX frequency downclock and port-5 serialization. SIMD would only benefit the YUV-heavy decoders — pixel-unpack formats should use auto-vectorized scalar.

## Decision

Use **cargo feature gates** (`--features simd`) as the primary on/off switch for SIMD, with runtime `is_x86_feature_detected!` dispatch inside the SIMD gate to select the best available ISA extension. ARM NEON is unconditional within the SIMD gate (ARMv8 guarantees NEON) except on macOS where it is gated by a known CI runner edge case.

The architecture has four layers:

### Layer 1: Feature gate (`Cargo.toml`)

```toml
[features]
simd = []
```

The `simd` feature is a pure flag — no code, no dependencies. All SIMD modules are behind `#[cfg(feature = "simd")]`.

### Layer 2: Dispatch functions (`simd/mod.rs`)

Platform-agnostic entry points that select the best ISA at call time:

```rust
pub fn uyvy_to_bgra(input: &[u8], output: &mut [u8], width: i32, height: i32) {
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    if is_x86_feature_detected!("avx2") && width >= 16 {
        return unsafe { avx2::uyvy_to_bgra(input, output, width, height); }
    }
    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    if is_x86_feature_detected!("sse2") && width >= 8 {
        return unsafe { sse2::uyvy_to_bgra(input, output, width, height); }
    }
    #[cfg(all(feature = "simd", target_arch = "aarch64", not(target_os = "macos")))]
    unsafe { neon::uyvy_to_bgra(input, output, width, height); }
    // Fallback: scalar
}
```

### Layer 3: Per-ISA modules (`simd/sse2.rs`, `simd/avx2.rs`, `simd/neon.rs`)

Each module contains the SIMD kernel for one ISA. AVX2 code remains in the repository (behind `#[cfg]`) but is **never dispatched at runtime** — it was removed from the dispatch ladder after benchmarking showed SSE2 was faster on Intel (see Consequences). The code is retained for future hardware where AVX2 downclocking may not apply.

### Layer 4: Scalar fallback (`simd/scalar.rs`)

Portable implementations used when either the `simd` feature is disabled or the target platform lacks the required ISA. Every dispatch function has a scalar fallback behind a `#[cfg(not(all(feature = "simd", ...)))]` guard to prevent `unreachable_code` errors.

### Platform coverage

| Platform                             | Default SIMD    | Our Dispatch                                     |
| ------------------------------------ | --------------- | ------------------------------------------------ |
| x86_64 (Linux, macOS Intel, Windows) | SSE2 guaranteed | SSE2 → scalar (AVX2 compiled but not dispatched) |
| aarch64 (Linux ARM)                  | NEON guaranteed | NEON → scalar                                    |
| aarch64 (macOS ARM)                  | NEON guaranteed | NEON → scalar                                    |
| x86 (32-bit)                         | SSE2            | SSE2 → scalar                                    |
| Other (RISC-V, etc.)                 | None            | Scalar only                                      |

### macOS ARM NEON (resolved)

NEON dispatch on macOS aarch64 was previously gated behind `#[cfg(not(target_os = "macos"))]` due to a BRGA vs BGRA channel-ordering bug in the `vzip_s16` interleave (green and red channels swapped). Fixed in v1.9.3 — confirmed by ARM64 CI run on ubuntu-24.04-arm with all 581 tests passing. macOS ARM now uses NEON acceleration like Linux ARM.

### RGB565/RGB555: No hand-written SIMD

Benchmarking showed hand-written SSE2/AVX2 RGB565→BGRA was 34× slower than LLVM's auto-vectorized scalar loop on Intel (AVX frequency downclock + port-5 bottleneck). These formats use plain scalar loops that LLVM auto-vectorizes to SSE2 or NEON automatically. See [BENCHMARKS.md](../benchmarks/BENCHMARKS.md) for data.

## Consequences

### Positive

- **Clean separation**: Feature gates keep SIMD code isolated. Building without `--features simd` produces a fully functional scalar-only binary.
- **Auditable unsafe**: All `unsafe` blocks are confined to `simd/*.rs` (per-module `#![allow(unsafe_code)]`). Miri verifies 21 SSE2 tests with zero UB.
- **Scalar reference**: The scalar path is always the confirmed-correct reference. All SIMD identity tests compare against it byte-for-byte (15,625 nibble combos for CL, 65,536 values for RGB565).
- **Zero-cost on unsupported platforms**: The `#[cfg]` guards ensure SIMD code is not even compiled on platforms where it cannot run.
- **Runtime flexibility**: Within the SIMD gate, `is_x86_feature_detected!` lets SSE2-only CPUs (older x86_64) use the SSE2 path without a separate binary.
- **CI coverage**: 6 platform/feature combinations tested (Linux x64 ±simd, macOS ARM ±simd, Windows x64 ±simd).

### Negative

- **Code duplication**: YUV conversion kernels are written three times (SSE2, NEON, scalar). The shuffle masks are identical between SSE2 and NEON, but the intrinsic names differ.
- **Tail pixel handling**: Widths not divisible by 8 (SSE2/NEON) or 4 (AVX2) require scalar fallthrough for the remaining pixels, adding per-format tail logic.
- **AVX2 dead code**: The AVX2 modules compile but are never dispatched. They require ongoing maintenance to keep compiling.
- **macOS ARM NEON gap**: macOS ARM users lose ~2–3× throughput on YUV decoders compared to Linux ARM NEON. Fix requires upstream CI runner resolution.
- **No AVX-512**: Unlike the C# prototype which used `Avx512BW.IsSupported`, the Rust port has no AVX-512 path — the Intel frequency downclock issue made 256-bit AVX2 already slower than SSE2, and 512-bit would be worse.

## Alternatives Considered

| Approach                                                         | Why rejected                                                                                                                                                                              |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Runtime CPUID dispatch only** (no feature gate)                | Requires all SIMD code to be compiled on all platforms, increasing binary size and forcing `#[cfg]` on every platform-specific import. Feature gate lets non-SIMD users opt out entirely. |
| **Pure runtime dispatch via `core::arch`** without feature gates | Would compile NEON on x86 and SSE2 on ARM, producing dead code that `rustc` cannot always eliminate. Feature gates prevent compilation entirely on irrelevant targets.                    |
| **Auto-vectorization for all decoders**                          | LLVM auto-vectorizes simple pixel unpack (RGB565) well but struggles with YUV BT.601 arithmetic (interleaved multiply-add-shuffle). Hand-written SIMD is 4–8× faster for YUV.             |
| **C# style per-file ISA splitting**                              | Each decoder file would need its own SIMD+scalar variants. Consolidating all SIMD in `simd/` makes audits simpler and ISA-specific logic easier to maintain.                              |
| **`cfg!(target_feature)` at compile time**                       | LTO or cross-compilation may not resolve correctly. Runtime `is_x86_feature_detected!` is the documented approach for portable SIMD dispatch.                                             |

## References

- Cross-platform SIMD lessons learned: [STANDARDS.md](../standards/STANDARDS.md#cross-platform-simd)
- C# SIMD strategy (superseded): [ADR-0002](csharp/0002-simd-dispatch-strategy.md)
- Migration context: [EVOLUTION.md](../EVOLUTION.md#adr-1-cross-platform-simd-dispatch)
- Benchmark data: [BENCHMARKS.md](../benchmarks/BENCHMARKS.md)
