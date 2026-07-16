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

# Quick run without perf counters
./scripts/run-bench-perf.sh --quick
```

Output is written to `target/bench/`:

- `divan-{timestamp}.json` — machine-readable benchmark results
- `perf-{timestamp}.txt` — `perf stat` counters (when available)
- `baseline.json` — latest baseline (overwritten each run)

### Benchmark Targets

All 6 targets are run by `run-bench-perf.sh`:

| Target | Description |
|--------|-------------|
| `decoders` | Per-format decode throughput at 4 resolutions (64, 256, 512, 720×480) |
| `encoders` | Per-format encode throughput at 3 resolutions (64, 256, 512) |
| `pipeline` | End-to-end decode + encode pipeline timing |
| `simd_compare` | SIMD vs scalar cross-validation at 512×512 |
| `memory` | Heap allocation count and bytes per decode (8 formats at 512×512) |
| `profiles` | Decode throughput for all 54 built-in profiles |

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

| Format          | Encoding             | BPP | Description                                        |
| --------------- | -------------------- | --- | -------------------------------------------------- |
| RGB565          | `Rgb565`             | 2   | 16-bit RGB (5R+6G+5B)                              |
| RGB555          | `Rgb555`             | 2   | 16-bit RGB (5R+5G+5B)                              |
| ReorderedRGB555 | `ReorderedRgb555`    | 2   | 16-bit RGB with Z-order Morton interleave          |
| UYVY            | `Yuv422`             | 2   | 4:2:2 YCbCr, byte-interleaved                      |
| YCbCr 4:2:0     | `Ycbcr420`           | 1.5 | 4:2:0 YCbCr, planar                                |
| CL              | `Yuv422`+cl_chroma   | 2   | Chroma-luma nibble interleave (per-pixel chroma)   |
| CLCL            | `Yuv422`+clcl_chroma | 2   | Chroma-luma nibble interleave (shared chroma pair) |
| JPEG            | `Jpeg`               | —   | JPEG passthrough (limited to 64×64 fixture)        |

### Output Formats

Decoded output is 32-bit BGRA (4 bytes per pixel) in channel order
Blue-Green-Red-Alpha, as used by Apple's `vImage` framework and ImageGlass.

### Latency Distribution Analysis

Divan reports mean and median (p50) decode latency per benchmark. For
deeper statistical insight (p95, p99, stddev), the `perf-stats.py` tool
captures and analyzes the full benchmark output.

#### Methodology

Divan's default output shows four timing aggregates per benchmark:
**fastest** (minimum per-iteration time), **slowest** (maximum),
**median** (p50), and **mean**. These are computed from ~100 samples,
each of which is itself an average of `sample_size` iterations (typically
100–819200 depending on the benchmark speed).

The `perf-stats.py` tool extracts these four values from the terminal
output and computes estimated p95 and p99 using nearest-rank estimation
on the 4-point summary. Since only 4 data points are available (not raw
per-iteration samples), p95 and p99 values should be treated as
indicative rather than statistically rigorous. They reliably converge
to the **slowest** (max) value when the tail is heavy.

For production-grade percentile analysis, raw per-iteration timing must
be captured at the harness level — either by adding per-iteration trace
output to the bench binary or by switching to a framework that exposes
raw samples (e.g., Criterion).

#### Usage

```bash
# Run benchmarks and capture results to JSON:
cargo bench -p ithmb-core --bench decoders -- --output-json=results.json

# Analyze latency distribution:
python3 tools/analysis/perf-stats.py results.json

# Or run directly with the convenience wrapper:
./scripts/run-bench-perf.sh  # JSON is saved to target/bench/
python3 tools/analysis/perf-stats.py target/bench/divan-*.json
```

The JSON file (produced via `--output-json=<path>`) contains the full
raw Divan terminal output in its `raw_output` field. The Python tool
parses this to extract per-benchmark statistics.

#### Output Fields

| Field      | Source                                     | Meaning                                            |
| ---------- | ------------------------------------------ | -------------------------------------------------- |
| **p50**    | Divan `median`                             | 50th percentile — typical decode latency           |
| **p95**    | Estimated from fastest/slowest/median/mean | 95th percentile — 1-in-20 slow decode              |
| **p99**    | Estimated from fastest/slowest/median/mean | 99th percentile — 1-in-100 slow decode             |
| **Mean**   | Divan `mean`                               | Average decode latency (may be skewed by outliers) |
| **Min**    | Divan `fastest`                            | Best-case decode latency (no contention/interrupt) |
| **Max**    | Divan `slowest`                            | Worst-case decode latency (includes outliers)      |
| **Stddev** | Computed from 4-point summary              | Estimated population standard deviation            |

**Note:** The estimated p95/p99 should not be used for SLO/SLA
guarantees without validation against raw per-iteration data. They
are useful for identifying decoders with heavy-tailed latency
distributions (where max >> mean).

### CI Baseline

The CI pipeline runs a benchmark regression check after each push (`continue-on-error: true`).
Results are compared against [`.github/baseline.json`](https://github.com/B67687/Ithmb-Codec/blob/main/.github/baseline.json) with a 1.25× threshold.
See [`tools/check-benchmark-regression.sh`](https://github.com/B67687/Ithmb-Codec/blob/main/tools/check-benchmark-regression.sh) for implementation.

## Decoder Throughput (Baseline)

Measured on AMD Ryzen AI 9 HX 370 with SIMD acceleration (SSE2/AVX2 for x64).
Decoders listed in standard order used across both Rust and C# repos.

| Decoder           | 64×64   | 256×256 | 512×512 | 720×480 |
| ----------------- | ------- | ------- | ------- | ------- |
| RGB565            | 0.55 µs | 7.5 µs  | 33.3 µs | 44.8 µs |
| RGB555            | 0.55 µs | 7.8 µs  | 34.8 µs | 46.7 µs |
| UYVY              | 0.94 µs | 14.0 µs | 60.0 µs | 80.4 µs |
| UYVY (interlaced) | 2.17 µs | 30.8 µs | 60.7 µs | 81.6 µs |
| YCbCr 4:2:0       | 3.5 µs  | 36.4 µs | 149 µs  | 198 µs  |
| CL                | 1.3 µs  | 19.6 µs | 82.9 µs | 110 µs  |
| CLCL              | 0.20 µs | 2.9 µs  | 12.6 µs | 18.8 µs |
| Reordered RGB555  | —       | 108 µs  | 458 µs  | 619 µs  |

> Reordered RGB555 is square-only (w == h). 720×480 uses 512×512 as the nearest equivalent.
> For CI baseline, encoder throughput, and full data, see [`.github/baseline.json`](https://github.com/B67687/Ithmb-Codec/blob/main/.github/baseline.json).
