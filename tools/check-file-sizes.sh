#!/usr/bin/env bash
# check-file-sizes.sh — verify no file exceeds 250 pure LOC
#
# Counts lines of code excluding blanks, single-line comments, and
# comment-only blocks. Fails if any tracked source file exceeds 250 SLOC.
#
# Usage: check-file-sizes.sh [--quiet] [directory]
#
# Flags:
#   --quiet  Only report failures, suppress per-file listing
#
# Exits 0 if all files pass, 1 if any file exceeds threshold.

set -euo pipefail

THRESHOLD=250
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
TOTAL_FILES=0

while IFS= read -r -d '' file; do
  TOTAL_FILES=$((TOTAL_FILES + 1))

  # Count non-blank, non-comment-only lines
  # Strip: blank lines, // comments, /* */ blocks, XML doc comments
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

  if [ "$sloc" -gt "$THRESHOLD" ]; then
    echo "FAIL: $file — ${sloc} SLOC (threshold: ${THRESHOLD})"
    FAILED=$((FAILED + 1))
  elif [ "$QUIET" = false ]; then
    echo "OK: $file — ${sloc} SLOC"
  fi
done < <(find "$TARGET" -name '*.cs' -not -path '*/obj/*' -not -path '*/bin/*' -print0)

echo ""
echo "--- Checked $TOTAL_FILES files ---"

if [ "$FAILED" -gt 0 ]; then
  echo "FAIL: $FAILED file(s) exceed ${THRESHOLD} SLOC threshold"
  exit 1
else
  echo "OK: All files within ${THRESHOLD} SLOC threshold"
  exit 0
fi
