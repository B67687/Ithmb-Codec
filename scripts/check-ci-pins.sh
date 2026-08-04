#!/usr/bin/env bash
# Fail CI if any workflow action is not SHA-pinned or any cargo/pip install
# is not version-pinned. Prevents supply-chain drift (mutable tags, floating
# installs) from regressing silently.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0

# 1. Every `uses:` must reference a 40-hex commit SHA (tag allowed as a comment).
for f in .github/workflows/*.yml; do
  while IFS= read -r line; do
    if ! grep -qE 'uses: [^#]*@[0-9a-f]{40}([[:space:]]#|$)' <<<"$line"; then
      echo "UNPINNED ACTION in $f: $line"
      fail=1
    fi
  done < <(grep -E '^\s*-?\s*uses:' "$f" || true)
done

# 2. Every cargo install / pip install must pin a version.
for f in .github/workflows/*.yml; do
  while IFS= read -r line; do
    if ! grep -qE -- '(--version [0-9]|==[0-9])' <<<"$line"; then
      echo "UNPINNED INSTALL in $f: $line"
      fail=1
    fi
  done < <(grep -E 'run: .*\b(cargo install|pip install)\b' "$f" || true)
done

if [ "$fail" -ne 0 ]; then
  echo "CI pin check FAILED — pin actions to SHAs and installs to versions."
  exit 1
fi
echo "CI pins OK"
