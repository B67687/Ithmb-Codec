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

# 2. Build (default)
echo "--- 2. cargo build --workspace ---"
cargo build --workspace 2>&1 | scripts/check-zero-warnings.py
echo "OK"

# 3. Build (simd)
echo "--- 3. cargo build --workspace --features simd ---"
cargo build --workspace --features simd 2>&1 | scripts/check-zero-warnings.py
echo "OK"

# 4. Clippy (default)
echo "--- 4. cargo clippy --workspace --all-targets ---"
cargo clippy --workspace --all-targets 2>&1 | scripts/check-zero-warnings.py
echo "OK"

# 5. Clippy (simd)
echo "--- 5. cargo clippy --workspace --all-targets --features simd ---"
cargo clippy --workspace --all-targets --features simd 2>&1 | scripts/check-zero-warnings.py
echo "OK"

# 6. Clippy (all-features, strict)
echo "--- 6. cargo clippy --workspace --all-features --all-targets -- -D warnings ---"
cargo clippy --workspace --all-features --all-targets -- -D warnings
echo "OK"

# 7. Test (default)
echo "--- 7. cargo test --workspace ---"
cargo test --workspace
echo "OK"

# 8. Test (simd)
echo "--- 8. cargo test --workspace --features simd ---"
cargo test --workspace --features simd
echo "OK"

# 9. Doc (full, with private items)
echo "--- 9. cargo doc --workspace --no-deps --document-private-items ---"
cargo doc --workspace --no-deps --document-private-items 2>&1 | scripts/check-zero-warnings.py
echo "OK"

if [ -z "${RAPID}" ]; then
    # 10. Build (all-features)
    echo "--- 10. cargo build --workspace --all-features ---"
    cargo build --workspace --all-features 2>&1 | scripts/check-zero-warnings.py
    echo "OK"

    # 11. Clippy (all-features, zero-warnings check)
    echo "--- 11. cargo clippy --workspace --all-targets --all-features ---"
    cargo clippy --workspace --all-targets --all-features 2>&1 | scripts/check-zero-warnings.py
    echo "OK"
fi

echo ""
echo "=== ALL GATES PASSED ==="
