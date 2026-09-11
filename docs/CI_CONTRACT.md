# CI Contract — ithmb projects (canonical, 2026-09-10)

Single place stating every CI's exact commands + env, incl. action defaults.
Verified against the workflow files; update THIS file when any workflow changes.
Rule of thumb: local gates must mirror CI EXACTLY (bare `cargo clippy` ≠ CI's
`clippy --workspace --all-targets`). Local-only gates are marked as such —
they do NOT run on GitHub.

## Codec (`Ithmb-Codec/.github/workflows/`)

Env: top-level `RUSTFLAGS: ""` (defeats setup-rust-toolchain's `-D warnings`
injection — see ADR + handover), toolchain pinned via rust-toolchain.toml,
all actions SHA-pinned (enforced by `check_ci_pins`).

**pr-checks.yml** — push to main + PRs to main. The fast gate (9 jobs):

| Job           | Exact command                                                                                                                                                                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| format_check  | `cargo fmt --check --all`                                                                                                                                                                                                                                  |
| verify_clippy | `cargo clippy --workspace --all-targets`                                                                                                                                                                                                                   |
| typos         | typos-cli v1.42.3 prebuilt musl, scoped `./README.md ./AGENTS.md ./ARCHITECTURE.md ./crates/ ./docs/`                                                                                                                                                      |
| check_links   | lychee v2, `README.md docs/` (excludes: buymeacoffee, home.fau.edu, imageglass.org, theiphonewiki.com, mybroadband, buy.stripe.com, mdsite.duckdns.org, forums.whirlpool.net.au, github.com/itsmichaelwest/classick, shnatsel.medium.com, hungrypoint.com) |
| verify_deny   | cargo-deny 0.20.2 prebuilt musl, `cargo-deny check`                                                                                                                                                                                                        |
| cargo_audit   | cargo-audit v0.22.2 prebuilt gnu, `cargo-audit audit`                                                                                                                                                                                                      |
| doc_check     | `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-W warnings"`                                                                                                                                                                                        |
| secrets_scan  | gitleaks v3, full history (`fetch-depth: 0`)                                                                                                                                                                                                               |
| check_ci_pins | `bash scripts/check-ci-pins.sh`                                                                                                                                                                                                                            |
| machete       | `cargo install cargo-machete --version 0.9.2 --locked` then `cargo machete`                                                                                                                                                                                |

**ci-full.yml** — push to main. The heavy suite (5 jobs): multi-OS `build`
(`cargo build --features logging -p ithmb-core`, …), `benchmark_regression`,
`fuzz` (cargo-fuzz 0.13.2, 6 targets × `-max_total_time=30`),
`build_c_api` (`nm -D` symbol check + `cargo nextest run -p ithmb-core --features c --test c_api_test`),
`wasm` (`wasm-pack build crates/ithmb-wasm/`).

**release.yml** — tags `v*` only. `test` (clippy + cargo audit + nextest 0.9.143
via taiki-e), `build` (multi-OS), `python-wheels` (maturin, 5 targets:
linux x86_64+aarch64, macOS x86_64+aarch64, Windows x86_64), `release`.

**mutants.yml** — `workflow_dispatch` ONLY (never automatic).

Local mirrors: `scripts/check.sh` (T1 pre-push gate: clippy + nextest + deny +
gitleaks + F-### anchors + LOC fitness), `scripts/local-ci.sh` (all
Linux-runnable CI checks; macOS/Windows legs stay on GitHub),
`scripts/verify-all.sh` (all build configs).
LOCAL-ONLY (no CI gate, see TECH_DEBT PYM-01): pymod —
`(cd pymod && uv run --frozen pytest && uv run --frozen ruff check . && uv run --frozen basedpyright)`.

## Plugin (`Imageglass-Ithmb-Plugin/.github/workflows/ci.yml`)

Trigger: push to main + tags `v*`, PRs to main. Env: `RUSTFLAGS: ""`,
toolchain 1.88.0, 3-OS build matrix (ubuntu/macos/windows → linux/macos/windows zips).

| Job                | Exact command                                                                                                                                                                                                                  |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| build              | `cargo build --release` + symbol-export verify (PE table on Windows, `nm -D` elsewhere) + ABI smoke on Linux (`python3 scripts/abi-smoke.py tests/fixtures/test1.ithmb`) + `bash scripts/package.sh <tag>` → uploaded artifact |
| verify_clippy      | `cargo clippy --all-features --all-targets`                                                                                                                                                                                    |
| (nextest)          | `cargo nextest run --all-features --all-targets` (nextest 0.9.143 via taiki-e)                                                                                                                                                 |
| verify_deny        | `cargo deny check`                                                                                                                                                                                                             |
| secrets            | gitleaks over history                                                                                                                                                                                                          |
| release (tag only) | awk-extract `## [VER]` section from CHANGELOG.md → `gh release create "$TAG" --draft --notes-file NOTES.md *.igplugin.zip`                                                                                                     |
| verify_machete     | machete 0.9.2                                                                                                                                                                                                                  |

Local mirror: `scripts/check-local.sh`. Release notes rule: current version's
section ONLY, draft first, human publishes (see RELEASE_TRAIN.md).

## Web (`Ithmb-Codec-Web/.github/workflows/ci.yml`)

Runs on BOTH dev + public repos (public CI = free fallback; dev has a
2,000-min private limit). Trigger: push to main + PRs, minus docs-only
(`docs/**`, `*.md`; PRs also minus `.github/**`). `workflow_dispatch` allowed.
`concurrency: ci-<ref>` with cancel-in-progress (rapid pushes don't stack matrices).

| Job                                      | Exact command                                                                                                                                                                                                                                                                                       |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| lint                                     | Node 22 + bun 1.3.14 (pinned) + bun-install cache; `bun install --frozen-lockfile`; biome 1.9.4 format gate (`npx --yes @biomejs/biome@1.9.4 format .`); `bun run lint:modules && git diff --exit-code` (typecheck + build + determinism: build must not mutate tracked files); `bun run test:unit` |
| test (matrix: chromium, firefox, webkit) | same installs + Playwright browsers cached (keyed on `browsers.json`, NOT the lockfile); `bun run build`; `npx playwright test --project=<browser>` (per-browser ports via `PLAYWRIGHT_BROWSER`; server lifecycle owned by playwright webServer config, ADR-0007)                                   |

Opt-outs: `[skip browsers]` in the push commit message skips the test job
(head_commit contains-match — naming the trailer in a body ALSO matches;
keep bodies clean or accept the skip). Doc-only pushes skip CI entirely.

Local mirrors: `scripts/check-local.sh` (step 1 = `bun scripts/check-audit.mts`
fail-closed audit, then `test:unit`, then Playwright at `BASE_URL=:8899`),
`npm run ci` (= lint:modules + lint:i18n + playwright).
LOCAL-ONLY (not in ci.yml): dependency audit (check-audit.mts + WAIVERS file,
see TECH_DEBT QS-01), i18n gates (hook + local).

## Cost controls (all repos)

- Web: docs paths-ignore + concurrency + `[skip browsers]` + free public minutes.
- Codec: mutants manual-only; fuzz/bench confined to ci-full (push-main, not PRs).
- Plugin: release job tag-gated; artifacts via upload-artifact, not releases.
- Never `[skip ci]` to dodge a red gate — skips are for cost, not for hiding failure.
