# ADR-0008: CI optimization — strip dependency debug info, pinned prebuilt tools, full fuzz coverage

**Status:** Accepted (2026-08-19)

## Context

A strategic review of the codec CI pipeline (pr-checks.yml, ci-full.yml, release.yml) identified avoidable time spent recompiling unchanged state and source-building tooling on every run, without any corresponding quality gain. Three inefficiency classes:

1. **Dependency debug info is pure waste.** No `[profile]` section existed, so every dependency (zune-jpeg, clap, pyo3, png, …) was compiled with full `debug = 2` (DWARF) in dev/test. Backtraces never need to step *inside* a dependency's internals, yet every cold cache compile carried hundreds of MB of useless DWARF and slowed every dev/CI build. The workspace's own crates (ithmb-core etc.) were also at default, but for them the full debug is *required*: the codec has ~128 lines of `unsafe` (SIMD + C-API), and panic backtraces + gdb/lldb on that code must stay intact.

2. **`cargo install` recompiles tooling from source on every cold run.** typos-cli, cargo-deny, cargo-audit, and wasm-pack all ship official prebuilt release binaries. Source-compiling them cost ~1-3 min per cold run and re-opened the same drift risk the SHA/version pins exist to prevent.

3. **Fuzz coverage was undersized.** `fuzz/fuzz_targets/` defines 6 targets but CI ran only 3. The three never run (decode_pipeline, parse_photodb, parse_profile) include the central dispatch/cancellation path — arguably the most important for the unsafe SIMD code — and the two parsers. Miri was removed from CI (GitHub-hosted runners block its jailed child, rust-lang/miri#2711/#3233), so fuzz is now the **only** automated UB detector; leaving 3 targets permanently uncovered was an unacceptable gap.

A related correctness bug: cache keys hashed only `rust-toolchain.toml`, **not** `Cargo.lock`, so a dependency version bump did not invalidate the cache → stale-restore recompile.

## Decision

1. **Strip dependency debug info; keep full debug on own crates.** In the workspace root `Cargo.toml`, set `[profile.dev.package."*"] debug = false`, then explicitly re-enable `debug = true` for each own crate by exact name (ithmb-core, ithmb-cli, ithmb-gen, ithmb-wasm, ithmb-python). The `"*"` wildcard also matches workspace members, so the per-crate re-enable is mandatory — without it, backtrace line numbers silently vanish. `profile.test` inherits `profile.dev` automatically. The release profile is untouched (already `debug = 0`), and the fuzz crate is untouched (its profile must keep symbols for crash-reproducer symbolization).

2. **Use pinned prebuilt tool binaries instead of `cargo install`.** Replace the source-builds in pr-checks.yml, ci-full.yml, and release.yml with `curl` of the exact version-pinned GitHub release asset, extracting to `/tmp`:
   - typos v1.42.3 (musl tarball, binary at archive root)
   - cargo-deny 0.20.2 (musl tarball, needs `--strip-components=1`; standalone)
   - cargo-audit 0.22.2 (gnu `.tgz`, needs `--strip-components=1`; standalone, invoke explicit `audit` subcommand)
   - wasm-pack 0.12.1 (musl tarball, needs `--strip-components=1`; **keeps** the toolchain + wasm32 target because it shells out to cargo)
   - cargo-fuzz 0.13.2 has **no** prebuilt binary (crates.io source only) — it stays as `cargo install --debug --locked --version 0.13.2`.

3. **Close the fuzz coverage gap.** Add the 3 missing targets (`fuzz_decode_pipeline`, `fuzz_parse_photodb`, `fuzz_parse_profile`) to the fuzz job (each `-max_total_time=30`, same parallel job — no wall-time cost). Remove the phantom `cargo-fuzz` entry from `fuzz/rust-toolchain.toml`'s `components` (it is not a rustup component; listing it can fail the job on a fresh runner).

4. **Fix cache-key correctness.** Add `${{ hashFiles('**/Cargo.lock') }}` to every rust-cache key so dependency bumps invalidate caches. Drop the now-pointless `needs: [build]` serialization from benchmark_regression, fuzz, build_c_api, and wasm (build output was never shared between them — each has its own cache key).

5. **Enforce the new supply-chain pattern.** Extend `scripts/check-ci-pins.sh` with a section that rejects `releases/latest` URLs and any `curl`/`wget` line whose http(s) URL does not reference `/releases/download/` with an explicit version. Without this, a floating URL would silently reopen the drift the SHA/version pins prevent.

## Consequences

- **Positive**: ~15-30% faster codec build+test jobs and every local dev compile (smaller `target/`, no dep DWARF); no more 1-3 min tool-recompile tax on cold runs; all 6 fuzz targets now exercised (biggest remaining memory-safety gap closed); cache invalidation is now correct on dependency bumps; supply-chain drift on the new curl pattern is machine-enforced.
- **Negative**: dependency frames in a panic backtrace show bare symbols instead of file:line (never debugged anyway — acceptable); standalone `cargo-audit` needs the explicit `audit` subcommand.
- **Neutral**: none of these change where CI runs (all checks remain on GitHub Actions as enforced gates) or what checks exist — this is a speed/correctness optimization, not a coverage reduction. The checks' frequency is unchanged (all remain per-PR/per-push gates; lychee stays as-is).

## Related

- ADR-0006 (Dependency Management Policy) — the version/SHA-pin discipline this extends to prebuilt download URLs.
- `cargo` reference — `[profile.dev.package."*"]` wildcard semantics (matches workspace members; per-crate override required).
- `rust-lang/miri` #2711/#3233 — why miri cannot run on GitHub-hosted runners, making fuzz the sole automated UB detector.
