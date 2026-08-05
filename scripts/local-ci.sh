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

if command -v cargo-deny >/dev/null 2>&1; then
  run cargo deny check
else
  echo "--- cargo-deny not installed; skipping (install: cargo install cargo-deny --locked) ---"
fi

if command -v cargo-audit >/dev/null 2>&1; then
  run cargo audit
else
  echo "--- cargo-audit not installed; skipping (install: cargo install cargo-audit) ---"
fi

if command -v wasm-pack >/dev/null 2>&1; then
  run cargo build -p ithmb-wasm --target wasm32-unknown-unknown
else
  echo "--- wasm target/toolchain missing; skipping wasm build ---"
fi

# C API: build the cdylib with the c feature and run its test (fast, local-runnable)
run cargo build -p ithmb-core --features c
run cargo test -p ithmb-core --features c --test c_api_test

# Typos (pinned like CI)
if command -v typos >/dev/null 2>&1; then
  run typos -- ./README.md ./AGENTS.md ./ARCHITECTURE.md ./crates/ ./docs/
else
  echo "--- typos not installed; skipping (install: cargo install typos-cli --locked --version 1.42.3) ---"
fi

# Rustdoc -D warnings (matches pr-checks doc_check)
run env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

# CI pin enforcement (matches pr-checks check_ci_pins)
run bash scripts/check-ci-pins.sh

# Secrets scan (matches pr-checks secrets_scan)
if command -v gitleaks >/dev/null 2>&1; then
  run gitleaks detect --source .
else
  echo "--- gitleaks not installed; skipping (see https://github.com/gitleaks/gitleaks) ---"
fi

# Dependency updates — dev-first alternative to dependabot: run this, then
# commit upgrades on dev and ship them like any other change.
if command -v cargo-outdated >/dev/null 2>&1; then
  echo "--- dependency updates (cargo-outdated, informational) ---"
  cargo outdated -R 2>/dev/null | tail -n +2 | head -20 || true
else
  echo "--- cargo-outdated not installed; skipping (install: cargo install cargo-outdated) ---"
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
