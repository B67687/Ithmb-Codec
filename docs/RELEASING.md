# Releasing

How to ship a new version of Ithmb-Codec. The dev/public dual-repo ritual lives in `AGENTS.md` — this file is the release-specific checklist.

## Prerequisites (every release)

- `./scripts/local-ci.sh` passes (fmt, clippy `-D warnings`, full tests, builds, deny, audit, C-API).
- For core-decoder changes: the wasm has been regenerated and shipped to `Ithmb-Codec-Web` (see AGENTS.md "WASM Regeneration").
- The public repo's CI (`pr-checks` + `ci-full`) is green on the last ship. (If only the private dev repo is red, that's the billing block — public is the gate.)

## Steps

1. **Bump the version** — `Cargo.toml` (workspace root, `version = "X.Y.Z"`). Run `cargo build` once so `Cargo.lock` package versions sync.
2. **CHANGELOG** — add `## [X.Y.Z] - YYYY-MM-DD` (Keep a Changelog). Match the user-facing tone of prior entries; group under `### Added` / `### Changed` / `### Fixed` / `### Security` / `### CI`.
3. **Commit on dev** — `docs: changelog X.Y.Z + version bump — <summary>` (or `fix:`/`feat:` if the version commit rides real changes). Push `origin/main`.
4. **Ship to public** — branch from `public/main`, cherry-pick the net commits (collapse split/revert pairs), verify `git diff --quiet <dev-head> <branch>` is empty, push `public <branch>:main`.
5. **Tag on the PUBLIC repo** — `git tag vX.Y.Z` + push. The tag gates `release.yml` (GitHub Enterprise Release + Python wheels). Do NOT skip this — untagged versions lose traceability (1.9.5–1.10.2 shipped untagged once).
6. **crates.io / PyPI** — manual, out-of-band: `cargo publish -p ithmb-core` (and `-p ithmb-cli`), `maturin publish` for wheels. Verify the published versions match `Cargo.toml`.
7. **Web side** — if the decoder changed, `Ithmb-Codec-Web` gets its own version bump (1.4.x → next) with a changelog entry referencing the core change.

## Baseline refresh (benchmark)

`.github/baseline.json` is compared by `tools/check-benchmark-regression.sh` at 1.25×. If runners change enough to false-alarm, refresh it:

```bash
./scripts/bench-prep.sh            # or: cargo bench -p ithmb-core
# update .github/baseline.json from the fresh divan output, then PR it
```

The CI gate fails on real regressions (and on parser drift / missing matches — it must never silently pass).
