# ADR-0009: Explicit Empty RUSTFLAGS (Action-Injection Contract)

**Status:** Accepted (2026-09-07)

## Context

After the clippy cherry-pick (`all = deny` plus ~1754 new pedantic/nursery/
cargo warns), both Codec CI suites failed en masse. Root cause was not the
lints: `actions-rust-lang/setup-rust-toolchain` injects `RUSTFLAGS=-D warnings`
when the variable is unset (visible in logs as `_srt_NEW_RUSTFLAGS`), turning
every warn into a hard error outside Cargo.toml's control. The public clippy
section had been EMPTY, so pedantic had never fired before — the entire
failure surface was action-injected flags meeting new warns.

## Decision

Set explicit top-level `RUSTFLAGS: ""` in all workflows. The action skips
injection when the variable is set, so `Cargo.toml` `[workspace.lints]` alone
governs lint severity. This is documented configuration, not warning-gaming:
the same lints still fire, at the severity the repo declares.

## Consequences

- CI lint behavior is reproducible locally with bare `cargo clippy`
  (previously it was not — local runs lacked the injected `-D warnings`).
- Standing rule: local gates must mirror the EXACT CI command, and action
  defaults must be verified, not assumed. Any new action gets its defaults
  audited before adoption.
