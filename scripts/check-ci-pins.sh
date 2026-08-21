#!/usr/bin/env bash
# Fail CI if any workflow action is not SHA-pinned, any cargo/pip install is not
# version-pinned, or any curl/wget tool download is not pinned to an exact release.
# Prevents supply-chain drift (mutable tags, floating installs, latest-URL downloads)
# from regressing silently.
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

# 3. Every curl/wget download of a tool must pin an exact release version in the
#    URL (reject mutable 'latest' tags and unversioned asset paths). This guards
#    the pinned-prebuilt-binary supply-chain pattern (see ADR-0008): without it,
#    a floating URL silently reopens the same drift the SHA/version pins prevent.
for f in .github/workflows/*.yml; do
  while IFS= read -r line; do
    # Reject any curl/wget line that references a mutable 'latest' URL.
    if grep -qE -- 'https?://[^ ]*releases/latest' <<<"$line"; then
      echo "UNPINNED DOWNLOAD URL in $f: $line"
      fail=1
    fi
    # For any curl/wget line carrying an http(s) URL, require /releases/download/
    # with an explicit version (v?[0-9]) somewhere in the path.
    if grep -qE -- '\b(curl|wget)\b' <<<"$line" && grep -qE -- 'https?://' <<<"$line"; then
      if ! grep -qE -- '/releases/download/[^ ]*v?[0-9]+' <<<"$line"; then
        echo "UNPINNED DOWNLOAD URL in $f: $line"
        fail=1
      fi
    fi
  done < <(grep -E 'run: .*\b(curl|wget)\b' "$f" || true)
done

if [ "$fail" -ne 0 ]; then
  echo "CI pin check FAILED — pin actions to SHAs, installs to versions, and download URLs to pinned releases."
  exit 1
fi
echo "CI pins OK"

if [ "$fail" -ne 0 ]; then
  echo "CI pin check FAILED — pin actions to SHAs and installs to versions."
  exit 1
fi
echo "CI pins OK"
