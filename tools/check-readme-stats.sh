#!/bin/bash
# check-readme-stats.sh — verify README numbers match source.
# Usage: check-readme-stats.sh <dotnet-test-passed-count>
#        check-readme-stats.sh (profile-only check, no test check)
set -euo pipefail

errors=0

# === Profile count ===
# Count entries with "prefix" key in embedded profiles (ProfilesJson.cs)
profile_count=$(grep '"prefix"' src/IthmbCodec/IthmbCodecPlugin.ProfilesJson.cs 2>/dev/null | grep -cv '//' || echo 0)
echo "embedded profiles: $profile_count"

if grep -q "\*\*${profile_count} known profiles\*\*" README.md 2>/dev/null; then
    echo "  OK: profile count matches README (Profile Reference)"
else
    echo "  FAIL: README does not contain '**${profile_count} known profiles**'"
    errors=$((errors + 1))
fi

# === Test count (from dotnet test output) ===
if [ -n "${1:-}" ]; then
    tc="$1"
    echo "dotnet test passed: $tc"

    if grep -q "\*\*${tc} tests\*\*" README.md 2>/dev/null; then
        echo "  OK: test count matches README (Testing & validation)"
    else
        echo "  FAIL: README does not contain '**${tc} tests**'"
        errors=$((errors + 1))
    fi

    # Also check Limitations section (secondary location)
    if grep -q "pass roundtrip tests (${tc} total)" README.md 2>/dev/null; then
        echo "  OK: test count matches README (Limitations)"
    else
        echo "  FAIL: README Limitations does not contain 'pass roundtrip tests (${tc} total)'"
        errors=$((errors + 1))
    fi
else
    echo "Test count not provided — skipping test checks (run: check-readme-stats.sh <passed-count>)"
fi

exit $errors
