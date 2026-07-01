#!/usr/bin/env bash
# profile-diff.sh — compare embedded profiles against upstream sources
#
# Extracts all profile definitions from the embedded ProfilesJson.cs and
# optionally compares against known upstream reference implementations.
#
# Usage:
#   bash tools/profile-diff.sh [--dump] [--check-upstream]
#
# Flags:
#   --dump           Print all profiles in a structured table
#   --check-upstream Compare against known upstream repos (requires network)
#
# Upstream sources for cross-reference (manual):
#   - libgpod: https://github.com/nothings/ithmb (reference profiles)
#   - iOpenPod: https://github.com/minego/iOpenPod (iOS decoder + profiles)
#   - ithmbrdr: https://github.com/tomaskovacik/ithmbrdr (Python decoder)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILES_FILE="$SCRIPT_DIR/../src/IthmbCodec/IthmbCodecPlugin.ProfilesJson.cs"

if [ ! -f "$PROFILES_FILE" ]; then
  echo "ERROR: ProfilesJson.cs not found at $PROFILES_FILE"
  echo "Run this script from the repo root or tools/ directory."
  exit 1
fi

# Extract prefix values from the embedded JSON
echo "=== Ithmb-Codec Embedded Profiles ==="
echo "Source: $PROFILES_FILE"
echo ""

# Count profiles by counting "prefix" occurrences
PROFILE_COUNT=$(grep -c '"prefix"' "$PROFILES_FILE" || true)
echo "Active profiles in embedded JSON: $PROFILE_COUNT"

# List all profile prefixes in sorted order
echo ""
echo "=== Profile Prefixes ==="
grep -oP '"prefix":\s*\K[0-9]+' "$PROFILES_FILE" | sort -n | while read -r prefix; do
  echo "  $prefix"
done

# Extract disabled profiles (commented out)
DISABLED_COUNT=$(grep -c '//.*"prefix"' "$PROFILES_FILE" || true)
if [ "$DISABLED_COUNT" -gt 0 ]; then
  echo ""
  echo "=== Disabled Profiles ==="
  grep -oP '//.*"prefix":\s*\K[0-9]+' "$PROFILES_FILE" | sort -n | while read -r prefix; do
    echo "  $prefix (disabled)"
  done
fi

echo ""
echo "=== Summary ==="
echo "Total entries:     $(grep -c '"prefix"' "$PROFILES_FILE")"
echo "Active:            $PROFILE_COUNT"
echo "Disabled:          $DISABLED_COUNT"

# Optionally dump full profile table
if [ "${1:-}" = "--dump" ]; then
  echo ""
  echo "=== Full Profile Table ==="
  echo "Prefix | Width | Height | Encoding | FrameBytes | Flags"
  echo "-------|-------|--------|----------|------------|------"
  grep -oP '\{[^}]+\}' "$PROFILES_FILE" | grep '"prefix"' | while read -r line; do
    prefix=$(echo "$line" | grep -oP '"prefix":\s*\K[0-9]+')
    width=$(echo "$line" | grep -oP '"width":\s*\K[0-9]+')
    height=$(echo "$line" | grep -oP '"height":\s*\K[0-9]+')
    encoding=$(echo "$line" | grep -oP '"encoding":\s*\K[0-9]+')
    frames=$(echo "$line" | grep -oP '"frameByteLength":\s*\K[0-9]+')
    interlaced=$(echo "$line" | grep -oP '"interlaced":\s*\K[a-z]+')
    echo "$prefix | $width | $height | $encoding | $frames | intl=$interlaced"
  done
fi

if [ "${1:-}" = "--check-upstream" ]; then
  echo ""
  echo "=== Upstream Comparison ==="
  echo "NOTE: Manual cross-reference required. Clone upstream repos and compare:"
  echo ""
  echo "  # libgpod reference:"
  echo "  git clone https://github.com/nothings/ithmb.git /tmp/ithmb-ref"
  echo "  grep format_id /tmp/ithmb-ref/*.c | sort | uniq"
  echo ""
  echo "  # iOpenPod reference:"
  echo "  git clone https://github.com/minego/iOpenPod.git /tmp/iopenpod-ref"
  echo "  grep -r 'prefix' /tmp/iopenpod-ref/Sources/ --include='*.swift' | sort"
  echo ""
  echo "See docs/synthesis/v1.6.0.md for the original 22-source survey methodology."
fi
