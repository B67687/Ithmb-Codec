# Ithmb-Codec Development Setup

## Prerequisites

- **Rust toolchain** — install via [rustup](https://rustup.rs). Edition 2024 requires Rust 1.85+.
- **Git** with commit signing configured (`commit.gpgsign = true`)
- **Pre-commit hooks** — run once: `git config core.hooksPath .githooks`
- **LSP** — `.opencode/lsp.json` configures rust-analyzer with clippy as the check command
- (Optional) ARM64 runner for NEON test validation

## Quick Start

```bash
git clone https://github.com/B67687/Ithmb-Codec.git
cd Ithmb-Codec
cargo build --workspace
cargo test --workspace
```

## Committing

Always sign commits with the correct author date:

```bash
# Using the helper script (recommended):
bash tools/git-commit-dated.sh -m "feat: my change"

# Manual (preserve date):
GIT_COMMITTER_DATE="$(git log -1 --format=%aD)" git commit -S --date="$(git log -1 --format=%aD)" -m "feat: my change"
```

Commit messages follow Conventional Commits:
`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `perf:`, `ci:`, `chore:`, `chore(deps):`.

## Standards

This project follows the standards documented in `STANDARDS.md` and
the methodology in `docs/synthesis/v1.6.0.md`.

### Before committing

- [ ] CHANGELOG.md updated under `[Unreleased]`
- [ ] `cargo fmt --check` passes
- [ ] Tests pass: `cargo test --workspace`
- [ ] Clippy clean: `cargo clippy --workspace -- -D warnings`
- [ ] File sizes within 250 SLOC: `bash tools/check-file-sizes.sh`
- [ ] Commit signed
- [ ] (Nightly) Miri: `cargo +nightly miri test --features simd -p ithmb-core`

## CI Workflows

| Workflow | Trigger | What it does |
|----------|---------|-------------|
| `rust-ci.yml` | push/PR to main | Build, test, clippy, fmt, cabi build + symbol-export check |

## Architecture

See `docs/adr/` for Architecture Decision Records covering key design choices:
- ADR-0001: Native AOT plugin boundary
- ADR-0002: SIMD dispatch strategy
- ADR-0003: Profile discovery and resolution
