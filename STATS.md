# Project Statistics

Canonical numbers for the Ithmb-Codec Rust workspace. All other documentation should reference this file rather than duplicating these values.

## Codec

| Stat | Value |
|------|-------|
| Decoders | 8 (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CL, CLCL, JPEG) |
| Encoders | 7 (same minus JPEG) |
| Built-in profiles | 54 (plus 1 speculative disabled) |
| Device profiles | 18 iPod/iPhone generations |
| Max frame size | 830 KB (480×864 RGB565) |

## Testing

| Stat | Value |
|------|-------|
| Unit tests | 489 |
| Test suites | 12 |
| Golden test vectors | 14 |
| Concurrency tests | 11 |
| libfuzzer targets | 2 |
| Fuzz iterations | 1.2M+ |
| Miri tests | 21 |
| Fuzz crashes | 0 |
| Miri UB found | 0 |

## Codebase

| Stat | Value |
|------|-------|
| Source modules | 21 (default) / 23 (with cache+metrics features) |
| Crates | 5 (ithmb-core, ithmb-cli, ithmb-core-cabi, ithmb-gen, pymod) |
| Git commits | 284+ |
| Signed tags | 7 |

## Performance

See [`BENCHMARKS.md`](BENCHMARKS.md) for full benchmark data across all decoders, resolutions, and input patterns.
