# Rust Engineering Standards

This file documents the Rust engineering practices applied in this workspace.
It is based on authoritative external sources (linked below) and our own experience.

## Authorities

| Source | Reference |
|--------|-----------|
| [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) | Library API design — ~60 `C-*` checklists |
| [Microsoft Rust Guidelines](https://microsoft.github.io/rust-guidelines/) | Must/should rules for libraries, FFI, performance, AI |
| [ANSSI Rust Security Guide](https://anssi-fr.github.io/rust-guide/) | Unsafe code policy, fuzzing, supply chain |
| [Rust Style Guide](https://doc.rust-lang.org/style-guide/) | rustfmt defaults, `style_edition = "2024"` |
| [Clippy Lint Docs](https://doc.rust-lang.org/stable/clippy/lints.html) | Tiers: `all = "deny"`, `pedantic = "deny"` |
| [Rust Design Patterns](https://rust-unofficial.github.io/patterns/) | Idioms, patterns, anti-patterns reference |
| [Arm Rust SIMD](https://learn.arm.com/learning-paths/cross-platform/simd-on-rust/) | Cross-platform SIMD best practices |
| [Rustonomicon](https://doc.rust-lang.org/nomicon/) | Unsafe code — required reading before writing `unsafe` |
| [Apollo Handbook](https://github.com/apollographql/rust-best-practices) | Error handling, testing, dispatch |

## Lint Configuration

```toml
[lints.rust]
unsafe_code = "deny"        # unsafe only in simd/ and cabi/

[lints.clippy]
all = "deny"
pedantic = "deny"
```

Individual modules may `#[allow(unsafe_code)]` with justification.

## Unsafe Code Policy

1. Read the [Rustonomicon](https://doc.rust-lang.org/nomicon/) before writing `unsafe`.
2. Every `unsafe fn` must have a `// SAFETY:` comment listing all invariants.
3. Every `#[allow(unsafe_code)]` must have a justification comment.
4. All `unsafe` code paths must pass Miri verification in CI.
5. Prefer `std::simd` (nightly) or `wide` crate over raw `core::arch` intrinsics where possible.
6. Performance: use platform-specific `#[target_feature]` intrinsics only for measured hot paths.

## Error Handling

| Context | Tool | Rule |
|---------|------|------|
| Library crate | `thiserror` | Typed error enum per crate. Never `Box<dyn Error>`. |
| Application binary | `anyhow` | `Result<T, anyhow::Error>` in `fn main()`. |
| Tests | `unwrap()` | Acceptable inside `#[cfg(test)]` and `#[test]` functions. |
| Production | `?` | Never `unwrap()` or `expect()` in production code. |

## Formatting

- `style_edition = "2024"` in `rustfmt.toml`
- `imports_granularity = "item"` (one import per line, grouped by crate)
- 4-space indent, 100-char width (rustfmt defaults)
- Enforced by `cargo fmt --check` in CI

## Testing

| Type | Requirement |
|------|-------------|
| Doc tests | Every public API. Runs in `cargo test`. |
| Unit tests | Per-module `#[cfg(test)]`. Covers happy + error paths. |
| Golden tests | Reference decode outputs for every format. |
| Fuzz targets | `cargo-fuzz` for all decode entry points. |
| Miri | `cargo +nightly miri test` for all `unsafe` code paths. |
| CI | All of the above + `cargo clippy -- -D warnings`. |

## SIMD

See [`STANDARDS.md`](STANDARDS.md) → Cross-Platform SIMD section for:
- Dispatch architecture (3-layer)
- Platform coverage table
- cfg gate patterns
- Lessons learned from cross-platform fixes
- CI matrix

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — `C-COMMON-TRAITS`, `C-NO-OUT`, etc.
- [Microsoft Rust Guidelines](https://microsoft.github.io/rust-guidelines/) — FFI, performance, resilience chapters
- [ANSSI Rust Security Guide](https://anssi-fr.github.io/rust-guide/) — Rules (must), Recommendations (should), Warnings (info)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/) — Builder, newtype, RAII patterns
- [Rustonomicon](https://doc.rust-lang.org/nomicon/) — unsafe code, variance, FFI
- [Clippy Lint Reference](https://doc.rust-lang.org/stable/clippy/lints.html) — 800+ lints
- [Arm SIMD on Rust](https://learn.arm.com/learning-paths/cross-platform/simd-on-rust/) — NEON intrinsics, portable SIMD
- [Google Highway](https://github.com/google/highway) — cross-platform SIMD dispatch (C++)
