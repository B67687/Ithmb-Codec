#!/usr/bin/env bash
# scripts/verify-all.sh — comprehensive verification across all build configs.
# Exits with non-zero if ANY gate fails.
set -euo pipefail

OMIT_FMT="${OMIT_FMT:-}"
RAPID="${RAPID:-}"  # if set, skip full-feature combinations and doc

echo "=== verify-all.sh ==="
echo ""

# 1. Format
if [ -z "${OMIT_FMT}" ]; then
    echo "--- 1. cargo fmt --check ---"
    cargo fmt --check
    echo "OK"
else
    echo "--- 1. cargo fmt --check (skipped) ---"
fi

# 2. Build (default — warns allowed per cherry-pick)
echo "--- 2. cargo build --workspace ---"
cargo build --workspace
echo "OK"
# 3. Clippy (default, all-targets)
echo "--- 3. cargo clippy --workspace --all-targets ---"
cargo clippy --workspace --all-targets
echo "OK"

# 4. Clippy (all-features, strict — warns allowed per cherry-pick; hard denies still fail)
echo "--- 4. cargo clippy --workspace --all-features --all-targets ---"
cargo clippy --workspace --all-features --all-targets
echo "OK"
# 5. Test (nextest, per TOOLCHAIN inventory — nextest 0.9.143)
echo "--- 5. cargo nextest run --workspace ---"
cargo nextest run --workspace
echo "OK"
# 6. Doc (full, with private items)
echo "--- 6. cargo doc --workspace --no-deps --document-private-items ---"
cargo doc --workspace --no-deps --document-private-items
echo "OK"

if [ -z "${RAPID}" ]; then
    # 10. Build (all-features)
    echo "--- 10. cargo build --workspace --all-features ---"
    cargo build --workspace --all-features
    echo "OK"

    # 11. Clippy (all-features, zero-warnings check — warns allowed)
    echo "--- 11. cargo clippy --workspace --all-targets --all-features ---"
    cargo clippy --workspace --all-targets --all-features
    echo "OK"
fi

echo ""
echo "=== ALL GATES PASSED ==="
