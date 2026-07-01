# Ithmb-Codec Development Setup

## Prerequisites

- .NET 10 SDK (`global.json` pins `10.0.x`)
- Git with commit signing configured (`commit.gpgsign = true`)
- (Optional) ARM64 runner for NEON test validation

## Quick Start

```bash
git clone https://github.com/B67687/Ithmb-Codec.git
cd Ithmb-Codec
dotnet restore src/IthmbCodec/test/IthmbCodec.Tests.csproj --locked-mode
dotnet build src/IthmbCodec/test/IthmbCodec.Tests.csproj -c Release
dotnet test src/IthmbCodec/test/IthmbCodec.Tests.csproj -c Release --no-build
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
- [ ] `dotnet format src/IthmbCodec/IthmbCodec.csproj --verify-no-changes` passes
- [ ] Tests pass: `dotnet test -c Release`
- [ ] Static analysis: `dotnet build -c Release` produces no warnings
- [ ] File sizes within 250 SLOC: `bash tools/check-file-sizes.sh`
- [ ] Commit signed

## CI Workflows

| Workflow | Trigger | What it does |
|----------|---------|-------------|
| `build-linux.yml` | push/PR to main | Build + test (Release+Debug), coverage gate, SAST, stats verification |
| `test-neon.yml` | push/PR to main | ARM64 NEON intrinsic validation |
| `release-windows.yml` | tag push `v*` | Build Native AOT binary, upload to GitHub Release |
| `benchmark.yml` | manual dispatch | Run benchmarks, check regression against baseline |

## Architecture

See `docs/adr/` for Architecture Decision Records covering key design choices:
- ADR-0001: Native AOT plugin boundary
- ADR-0002: SIMD dispatch strategy
- ADR-0003: Profile discovery and resolution
