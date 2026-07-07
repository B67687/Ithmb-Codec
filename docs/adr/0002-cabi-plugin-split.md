# ADR-0002: C ABI Plugin Split

**Status**: Accepted (2026-07-07)

**Context**: The original Rust workspace included a `cabi` crate (`ithmb-core-cabi`) as a first-class workspace member alongside `ithmb-core`, `ithmb-cli`, `ithmb-gen`, and `pymod`. This crate produced a `cdylib` shared library implementing the ImageGlass v10 native plugin ABI — a single exported function `ig_plugin_get_api` callable by the ImageGlass image viewer on Windows and by any language with C FFI support.

Co-locating the C ABI crate with the core library created several tensions:

1. **Dependency isolation** — The `cabi` crate required the ImageGlass SDK headers and Windows-specific build tooling. Every workspace build (including `cargo check` on Linux/macOS) inherited these constraints or required `--exclude` flags.
2. **Release cycle coupling** — A plugin bug fix (e.g., ABI struct layout, symbol visibility) forced a workspace-wide version bump even when the core codec was unchanged.
3. **ABI stability burden** — The plugin's public API surface (a single C function with a fixed struct layout) had to remain backward-compatible. Every change to the core library's internal types had to be evaluated for ABI impact on the plugin, even when the plugin didn't use those types.
4. **Review scope** — PRs touching `cabi/` required reviewers familiar with both Rust FFI and the ImageGlass plugin ABI. Only one person on the project had this context.
5. **CI matrix expansion** — The `cabi` crate needed Windows build + symbol export verification (`nm -D | grep ig_plugin_get_api`), adding a full Windows job to the already-growing CI matrix.

The C# prototype had the opposite problem: it was a single-purpose Native AOT plugin that could never become a standalone library. The Rust port was supposed to avoid that trap, but keeping the C ABI crate in the workspace risked a similar coupling — just in reverse (library-heavy, plugin secondary).

## Decision

Extract the C ABI plugin into its **own repository**: [Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin).

### What was moved

| Item | Old location (workspace) | New location |
|------|-------------------------|-------------|
| C ABI `cdylib` crate | `crates/ithmb-core-cabi/` | [Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin) |
| ImageGlass SDK bindings | `cabi/src/ig_sdk/` | Plugin repo (internal) |
| Plugin CI jobs | `./github/workflows/cabi.yml` | Plugin repo (separate workflow) |
| Symbol export test | `cabi/tests/symbol_export.rs` | Plugin repo |

### What stayed in the workspace

| Crate | Role |
|-------|------|
| `ithmb-core` | Core library (published to crates.io) |
| `ithmb-cli` | Standalone CLI binary |
| `ithmb-gen` | Synthetic sample generator |
| `pymod` | Python bindings (PyO3) |

### Dependency relationship

The plugin repository depends on `ithmb-core` as an **external crate** (pulled from crates.io or Git):

```toml
[dependencies]
ithmb-core = { git = "https://github.com/B67687/Ithmb-Codec" }
```

This is a one-way dependency. The core library has no knowledge of the plugin or its ABI. No `unsafe_code` allowance is needed in the core workspace.

### Release cycle independence

- **Core library** (`ithmb-core`): Ships on its own cadence. PRs, releases, version bumps are independent of the plugin.
- **Plugin** (`Imageglass-Ithmb-Plugin`): Ships when the ImageGlass plugin ABI changes or when a new core release is needed. The plugin pins a specific `ithmb-core` version (or Git revision) and can lag behind the latest core release.

### crates.io publishing implications

Crates.io publishing became a two-step process:

1. Publish `ithmb-core` to crates.io (via `cargo publish -p ithmb-core`).
2. Update the plugin's `Cargo.toml` to reference the new crates.io version and publish the plugin independently (if ever published — currently distributed via Git).

The plugin is not published to crates.io because its ImageGlass SDK dependency is Windows-only and has no crates.io presence. It is distributed as a pre-built `.dll`/`.so`/`.dylib` via GitHub Releases.

## Consequences

### Positive

- **Dependency isolation**: The workspace no longer needs ImageGlass SDK headers. `cargo build` on any platform works without `--exclude`. No Windows-specific toolchain is required for core development.
- **Focused CI**: Each repo has its own CI matrix. The plugin's CI (3 OS + clippy + cargo-deny + symbol export verification) runs independently of the core workspace's CI (6 platform/feature combinations).
- **ABI freedom**: The core library can refactor internal types without ABI stability concerns. The plugin wraps the public `ithmb-core` API only.
- **Review simplicity**: Plugin PRs are reviewed in the plugin repo by FFI-aware reviewers. Core library PRs stay focused on format decoding.
- **Separate versioning**: Plugin version tracks ImageGlass compatibility, not core library features. Core library version tracks format support and API evolution.
- **Clean workspace members**: Four crates instead of five. No `cdylib` in a library-focused workspace.

### Negative

- **Two-step releases**: Publishing a core fix that the plugin needs requires updating two repos. Mitigated by making the plugin pin a Git SHA rather than a crates.io version, allowing immediate consumption.
- **Duplicated CI infrastructure**: Both repos need build, test, and lint pipelines. Mitigated by shared CI templates.
- **Plugin lag risk**: The plugin may fall behind the latest core release if no one monitors compatibility. Mitigated by automated Dependabot-style PRs (currently manual).
- **Cross-repo changes**: A feature that touches both core and plugin (e.g., adding a new decoder variant) requires PRs in both repos. Mitigated by the fact that the plugin simply delegates to the core library — new decoders are picked up automatically with a version bump.
- **Lost symbolic export coverage**: The core workspace no longer tests `nm -D` symbol verification. Mitigated by the plugin repo's CI doing it.

## Alternatives Considered

| Approach | Why rejected |
|----------|-------------|
| **Keep `cabi` in workspace with `--exclude`** | Every developer and CI job must remember `--exclude ithmb-core-cabi` on non-Windows platforms. Workspace commands like `cargo test --workspace` would fail without it. |
| **Feature-gate the `cabi` crate** | Would require a workspace-level feature flag that default-members skip. Complex to maintain and still couples release cycles. |
| **Monorepo with workspace exclusion** | Same problem as `--exclude` — the crate would live in the repo but be excluded in most CI jobs, inviting bitrot. |
| **Keep as-is (co-located)** | The workspace was designed for library distribution. A cdylib plugin is a fundamentally different artifact with different build, test, and distribution requirements. Keeping them together forced compromises on both. |

## References

- C# Native AOT approach (superseded): [ADR-0001](0001-use-native-aot-plugin-boundary.md)
- Migration context: [EVOLUTION.md](../EVOLUTION.md#phase-3-c-abi-split)
- Plugin repository: [Imageglass-Ithmb-Plugin](https://github.com/B67687/Imageglass-Ithmb-Plugin)
- ABI integrity standard: [STANDARDS.md](../standards/STANDARDS.md) (C ABI release integrity)
- Workspace structure: [README.md](../../README.md#c-abi-shared-library)
