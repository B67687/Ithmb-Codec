# Benchmarks

## Methodology

### Hardware & Environment

Benchmarks are run on the following reference hardware (CI may differ):

- **CPU**: AMD Ryzen AI 9 HX 370 (12C/24T, Zen 5)
- **Caches**: L1d 576 KiB, L2 12 MiB, L3 24 MiB
- **RAM**: 32 GiB LPDDR5x-7500 (28 GiB available, shared with iGPU)
- **OS**: Ubuntu 26.04 LTS, kernel 7.0.0

### CPU State Preparation

Run `scripts/bench-prep.sh` (with `sudo` if available) before taking measurements:

```
sudo ./scripts/bench-prep.sh
```

This script:
1. Sets `scaling_governor` to `performance` on all CPUs
2. Disables turbo boost (where supported) to reduce frequency variance
3. Prints before/after state so you can verify the environment

### Running Benchmarks

Use the convenience wrapper:

```
# Full benchmark run with perf stat counters
./scripts/run-bench-perf.sh

# With SIMD acceleration
./scripts/run-bench-perf.sh --features simd

# Quick run without perf counters
./scripts/run-bench-perf.sh --quick
```

Output is written to `target/bench/`:
- `divan-{timestamp}.json` — machine-readable benchmark results
- `perf-{timestamp}.txt` — `perf stat` counters (when available)
- `baseline.json` — latest baseline (overwritten each run)

### Framework

All benchmarks use [Divan](https://github.com/nvzqz/divan), an attribute-based
benchmarking framework for Rust that eliminates overhead from Bencher cloning
and provides automatic throughput reporting.

Key settings:
- **Minimum sample time**: Divan default (typically ~5 s per benchmark group)
- **Counter**: `BytesCount` — throughput measured in GB/s (GiB/s = 2³⁰ bytes/s)
- **Multi-size**: Each decoder is benchmarked at 4 resolutions:
  - 64×64 (L1 fit — measures pure dispatch/ALU)
  - 256×256 (L2 fit — representative decode)
  - 512×512 (L2 → L3 boundary)
  - 720×480 (typical thumbnail decode)

### Input Diversity

Each decoder benchmark cycles through 4 input patterns:
1. **Checkerboard** — alternating black/white, 2×2 tiles. Tests worst-case
   frequency transitions and full-range luminance.
2. **Random** — seeded LCG, deterministic across runs. Tests cache-miss
   behavior and average-case ALU utilization.
3. **Gradient** — horizontal+vertical linear ramps. Tests smooth-tone
   processing without abrupt transitions.
4. **Solid white** — all-0xFF. Tests bandwidth-only path (no ALU variance).

The bencher cycles through inputs with a round-robin atomic counter, so
each iteration gets a different pattern. Results are aggregate across all 4.

### Decoder Definitions

| Format | Encoding | BPP | Description |
|--------|----------|-----|-------------|
| RGB565 | `Rgb565` | 2 | 16-bit RGB (5R+6G+5B) |
| RGB555 | `Rgb555` | 2 | 16-bit RGB (5R+5G+5B) |
| ReorderedRGB555 | `ReorderedRgb555` | 2 | 16-bit RGB with Z-order Morton interleave |
| UYVY | `Yuv422` | 2 | 4:2:2 YCbCr, byte-interleaved |
| YCbCr 4:2:0 | `Ycbcr420` | 1.5 | 4:2:0 YCbCr, planar |
| CL | `Yuv422`+cl_chroma | 2 | Chroma-luma nibble interleave (per-pixel chroma) |
| CLCL | `Yuv422`+clcl_chroma | 2 | Chroma-luma nibble interleave (shared chroma pair) |
| JPEG | `Jpeg` | — | JPEG passthrough (limited to 64×64 fixture) |

### Output Formats

Decoded output is 32-bit BGRA (4 bytes per pixel) in channel order
Blue-Green-Red-Alpha, as used by Apple's `vImage` framework and ImageGlass.

### CI Baseline
### CI Baseline

The CI pipeline runs a benchmark regression check after each push (`continue-on-error: true`).
Results are compared against [`.github/baseline.json`](https://github.com/B67687/Ithmb-Codec/blob/main/.github/baseline.json) with a 1.25× threshold.
See [`tools/check-benchmark-regression.sh`](https://github.com/B67687/Ithmb-Codec/blob/main/tools/check-benchmark-regression.sh) for implementation.
## Decoder Throughput (Baseline)

Measured on AMD Ryzen AI 9 HX 370 with `--features simd` (SSE2/AVX2 for x64).
Decoders listed in standard order used across both Rust and C# repos.

| Decoder | 64×64 | 256×256 | 512×512 | 720×480 |
|---------|-------|---------|---------|---------|
| RGB565 | 0.52 µs | 7.5 µs | 33.3 µs | 45.3 µs |
| RGB555 | 0.55 µs | 7.9 µs | 35.3 µs | 47.2 µs |
| UYVY | 0.97 µs | 14.5 µs | 61.1 µs | 82.0 µs |
| UYVY (interlaced) | 2.05 µs | 30.8 µs | 60.8 µs | 81.6 µs |
| YCbCr 4:2:0 | 3.6 µs | 36.7 µs | 150 µs | 199 µs |
| CL | 1.3 µs | 20 µs | 84 µs | 112 µs |
| CLCL | 0.22 µs | 2.9 µs | 12.9 µs | 19.4 µs |
| Reordered RGB555 | — | 108 µs | 463 µs | 632 µs |

> Reordered RGB555 is square-only (w == h). 720×480 uses 512×512 as the nearest equivalent.
> For CI baseline, encoder throughput, and full data, see [`.github/baseline.json`](https://github.com/B67687/Ithmb-Codec/blob/main/.github/baseline.json).
