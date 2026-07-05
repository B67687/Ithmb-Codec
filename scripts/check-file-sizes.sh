#!/usr/bin/env bash
# check-file-sizes.sh — verify no file exceeds SLOC threshold
#
# Counts lines of code excluding blanks, single-line comments, and
# comment-only blocks. Fails if any source file exceeds threshold.
#
# Rules:
# - Files with "SIZE_OK" anywhere in the file are exempt (main source)
# - Files under src/*/test/ use THRESHOLD_TEST (default 500)
# - All other files use THRESHOLD (default 250)
#
# Usage: check-file-sizes.sh [--quiet] [directory]

set -euo pipefail

THRESHOLD=250
THRESHOLD_TEST=550
QUIET=false
TARGET="${2:-src/IthmbCodec}"

if [ "${1:-}" = "--quiet" ]; then
  QUIET=true
  TARGET="${2:-src/IthmbCodec}"
elif [ -n "${2:-}" ]; then
  TARGET="$2"
fi

if [ ! -d "$TARGET" ]; then
  echo "ERROR: directory $TARGET not found"
  exit 1
fi

FAILED=0
EXEMPTED=0
TOTAL_FILES=0

while IFS= read -r -d '' file; do
  TOTAL_FILES=$((TOTAL_FILES + 1))

  # Check SIZE_OK exemption in first 5 lines
  if grep -q 'SIZE_OK' "$file" 2>/dev/null; then
    EXEMPTED=$((EXEMPTED + 1))
    [ "$QUIET" = false ] && echo "EXEMPT: $file — SIZE_OK annotation"
    continue
  fi

  # Determine threshold based on path
  threshold=$THRESHOLD
  if echo "$file" | grep -q '/test/'; then
    threshold=$THRESHOLD_TEST
  fi

  # Count non-blank, non-comment-only lines
  sloc=$(awk '
    BEGIN { in_block = 0 }
    /\/\*/ { in_block = 1; next }
    /\*\// { in_block = 0; next }
    in_block { next }
    /^\s*\/\// { next }
    /^\s*$/ { next }
    { count++ }
    END { print count+0 }
  ' "$file")

  if [ "$sloc" -gt "$threshold" ]; then
    echo "FAIL: $file — ${sloc} SLOC (threshold: ${threshold})"
    FAILED=$((FAILED + 1))
  elif [ "$QUIET" = false ]; then
    echo "OK: $file — ${sloc} SLOC"
  fi
done < <(find "$TARGET" -name '*.rs' -not -path '*/target/*' -print0)

echo ""
echo "--- Checked $TOTAL_FILES files ($EXEMPTED exempted, $FAILED failed) ---"

if [ "$FAILED" -gt 0 ]; then
  echo "FAIL: $FAILED file(s) exceed SLOC threshold (main: $THRESHOLD, test: $THRESHOLD_TEST)"
  exit 1
else
  echo "OK: All files within SLOC thresholds"
  exit 0
fi
