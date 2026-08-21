#!/usr/bin/env bash
# Local CI — runs every CI check that can run on this machine, so you
# don't have to wait for (or trust) GitHub CI for the Linux-runnable set.
# The macOS/Windows legs cannot run here (no Mac/Windows) — those stay
# on GitHub CI, which still runs on every push.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== local-ci: ithmb-codec ==="
fail=0

run() {
  echo ""
  echo "--- $* ---"
  if ! "$@"; then
    echo "!!! FAILED: $*"
    fail=1
  fi
}

run cargo fmt --check
run cargo clippy --workspace --all-targets -- -D warnings
run cargo test --workspace --tests
run cargo build --workspace
run cargo build --features logging -p ithmb-core

# Tool-availability gates: a missing tool is a FAIL, not a skip — the
# no-false-pass constitution (spec rule 1) forbids silently weakening the
# gate set. CI runs all of these, so parity requires them locally too.
# (REVIEW warning fix: was silent-skip.)
require_tool() {
  local tool=$1
  local hint=$2
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "!!! FAILED: required tool '$tool' missing — $hint"
    fail=1
    return 1
  fi
  return 0
}

if require_tool cargo-deny 'install: cargo install cargo-deny --locked'; then
  run cargo deny check
fi

if require_tool cargo-audit 'install: cargo install cargo-audit'; then
  run cargo audit
fi

if require_tool wasm-pack 'install: rustup target add wasm32-unknown-unknown && cargo install wasm-pack'; then
  run cargo build -p ithmb-wasm --target wasm32-unknown-unknown
fi

# C API: build the cdylib with the c feature and run its test (fast, local-runnable)
run cargo build -p ithmb-core --features c
run cargo test -p ithmb-core --features c --test c_api_test

# Typos (pinned like CI)
if require_tool typos 'install: cargo install typos-cli --locked --version 1.42.3'; then
  run typos -- ./README.md ./AGENTS.md ./ARCHITECTURE.md ./crates/ ./docs/
fi

# Rustdoc -D warnings (matches pr-checks doc_check)
run env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

# CI pin enforcement (matches pr-checks check_ci_pins)
run bash scripts/check-ci-pins.sh

# Secrets scan (matches pr-checks secrets_scan)
if require_tool gitleaks 'see https://github.com/gitleaks/gitleaks'; then
  run gitleaks detect --source .
fi

# Dependency updates — dev-first alternative to dependabot: run this, then
# commit upgrades on dev and ship them like any other change. Informational only.
if command -v cargo-outdated >/dev/null 2>&1; then
  echo "--- dependency updates (cargo-outdated, informational) ---"
  cargo outdated -R 2>/dev/null | tail -n +2 | head -20 || true
else
  echo "--- cargo-outdated not installed; skipping (informational only, not a gate) ---"
fi

# Fuzz is slow (minutes) — only run when explicitly requested.
if [ "${1:-}" = "--fuzz" ]; then
  echo "--- fuzz: bounded runs (15s per target; pinned nightly via fuzz/rust-toolchain.toml) ---"
  (cd fuzz && cargo fuzz run fuzz_decode_ithmb -- -max_total_time=15 -print_final_stats=1 2>/dev/null | grep -E 'Done|crash') || fail=1
  (cd fuzz && cargo fuzz run fuzz_open_ithmb -- -max_total_time=15 -print_final_stats=1 2>/dev/null | grep -E 'Done|crash') || fail=1
  (cd fuzz && cargo fuzz run fuzz_encode_roundtrip -- -max_total_time=15 -print_final_stats=1 2>/dev/null | grep -E 'Done|crash') || fail=1
else
  echo "--- fuzz skipped (run ./scripts/local-ci.sh --fuzz to include) ---"
fi

echo ""
if [ "$fail" -eq 0 ]; then
  echo "=== local-ci: ALL CHECKS PASSED ==="
else
  echo "=== local-ci: SOME CHECKS FAILED (see above) ==="
  exit 1
fi
