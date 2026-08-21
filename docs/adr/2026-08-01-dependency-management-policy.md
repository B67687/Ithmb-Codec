# ADR-0006: Dependency Management Policy

**Status:** Accepted (2026-08-01)

## Context

The repository previously relied on Dependabot (`.github/dependabot.yml` with weekly schedules for both `cargo` and `github-actions` ecosystems) plus two auto-merge workflows (`dependabot-auto-merge.yml`, `dependabot-auto-merge-checks.yml`) to keep dependencies current. Several factors made this automation a poor fit:

1. **Contribution policy**: This project intentionally accepts **issues only** — no external contributions, including automated bot PRs. Dependabot's weekly PR stream directly contradicts that stance (every PR must be reviewed and closed or merged by the owner).
2. **Configuration burden**: The dependabot + auto-merge stack is itself a non-trivial, occasionally misconfigured piece of infrastructure to maintain — more moving parts than the dependencies it manages.
3. **Tiny, stable dependency surface**: The workspace depends on a small set of crates (`zune-jpeg`, `lru`, `thiserror`, `log`, `clap`, `anyhow`, `png`, `wasm-bindgen`, `pyo3`). An audit on 2026-08-01 showed everything current except trivial patch releases — there is no high-churn dependency problem to automate away.
4. **Local tooling is complete**: `cargo audit` (RustSec advisory DB) and `cargo outdated` provide the same information Dependabot would, on demand, with zero background infrastructure.

## Decision

**Manage dependencies locally; remove all automated dependency tooling.**

- Delete `.github/dependabot.yml` and both dependabot auto-merge workflows.
- Keep `pr-checks.yml` (fast checks on PRs + pushes: fmt, clippy, typos, links, deny, audit, doc) and `ci-full.yml` (expensive suite on push to main only: 3-OS build matrix, fuzz, benchmarks, wasm, C-API) — correctness gates, unrelated to dependency churn. Undefined-behavior detection is handled by the `fuzz` job (see ADR-0008); the former `miri.yml` was removed because GitHub-hosted runners block miri's jailed child (rust-lang/miri#2711/#3233).
- Keep `release.yml` (cross-platform build matrix + packaging). It runs only on `v*` tags and is the compatibility gate that guarantees the codec + CLI + Python wheels build on Linux x86_64/ARM64, macOS Intel/ARM, and Windows x86_64 — a check that cannot be replicated locally on a single machine.

### Local dependency ritual

| When | Command | Purpose |
|---|---|---|
| Before every release | `cargo audit` | Vulnerability check against RustSec advisory DB — **mandatory** |
| Monthly / before release work | `cargo outdated` | See what is behind |
| As needed | `cargo update` | Apply safe patch-level bumps |
| Before release | `cargo test --workspace` | Confirm updates broke nothing (already part of release.yml `test` job) |

The one non-negotiable discipline is **`cargo audit` before each release**. It covers the security-sensitive dependencies (`zune-jpeg`, `png`) at the moment code ships to users. The RustSec DB is fetched in seconds; a scheduled job adds infrastructure without adding protection.

## Consequences

- **Positive**: No bot PRs to triage; no misconfigured automation to debug; fewer `.github/` files; dependency state is fully visible and owner-controlled.
- **Negative**: A CVE could go unnoticed between releases if the pre-release `cargo audit` is skipped. Mitigated by making it part of the release checklist and by the small, stable dependency surface.
- **Neutral**: Patch-level bumps (`cargo update`) happen opportunistically rather than on a fixed weekly schedule — acceptable for a solo-maintained project.

## Related

- ADR-0004 (Quarterly Audit Protocol) — the periodic manual review cadence this policy complements.
- `cargo audit` documentation: https://rustsec.org — advisory database consumed by the audit tool.
