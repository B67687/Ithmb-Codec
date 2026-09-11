#!/usr/bin/env bash
# check-release.sh — executable pre-flight checklist for the ithmb release train.
# Canonical home: Codec repo (docs/RELEASE_TRAIN.md). Run from the repo root:
#   ./scripts/check-release.sh codec 1.9.11
#   ./scripts/check-release.sh plugin 1.1.5   (run inside the Plugin repo)
#   ./scripts/check-release.sh web 1.4.19      (run inside the Web repo)
# Exit 0 = all machine checks pass. Manual gates are printed, not enforced.
set -u
REPO="${1:?usage: check-release.sh <codec|plugin|web> <version>}"
VER="${2:?usage: check-release.sh <codec|plugin|web> <version>}"
FAIL=0
fail() { echo "FAIL: $1"; FAIL=1; }
pass() { echo "ok: $1"; }

[ -z "$(git status --porcelain | grep -v '^??')" ] && pass "tree clean (tracked)" || fail "tracked tree not clean"

case "$REPO" in
codec)
  grep -q "^version = \"$VER\"$" Cargo.toml && pass "Cargo.toml = $VER" || fail "Cargo.toml != $VER"
  grep -q '^version.workspace = true' pymod/Cargo.toml 2>/dev/null && pass "pymod inherits workspace version" || fail "pymod version not workspace-inherited"
  ! grep -q '^version *=' pymod/pyproject.toml && pass "pyproject has no hardcoded version (maturin derives)" || fail "pyproject hardcodes version (1.9.9-wheel incident)"
  grep -q "## \[$VER\]" CHANGELOG.md && pass "CHANGELOG has [$VER]" || fail "CHANGELOG missing [$VER]"
  if grep -i 'enterprise' .github/workflows/release.yml | grep -vi 'no enterprise product' | grep -q .; then fail "release template claims Enterprise"; else pass "release template has no Enterprise claim"; fi
  if grep -qi 'wheel' .github/workflows/release.yml; then
    grep -qi 'not on PyPI\|manual install' .github/workflows/release.yml && pass "wheels labeled manual-install, not PyPI (honest)" || fail "wheels built but not labeled manual-install (PyPI confusion)"
  else pass "no wheels built, nothing to label"
  fi
  ;;
plugin)
  grep -q "^version = \"$VER\"$" Cargo.toml && pass "Cargo.toml = $VER" || fail "Cargo.toml != $VER"
  grep -q "\"version\": \"$VER\"" igplugin.json && pass "igplugin.json = $VER" || fail "igplugin.json != $VER"
  grep -q "env!(.*VERSION.*$VER\|$VER" src/lib.rs 2>/dev/null || true
  grep -q "## \[$VER\]" CHANGELOG.md && pass "CHANGELOG has [$VER]" || fail "CHANGELOG missing [$VER]"
  grep -q -- "--draft" .github/workflows/ci.yml && pass "release job creates drafts" || fail "release job not draft-gated"
  ;;
web)
  grep -q "\"version\": \"$VER\"" package.json && pass "package.json = $VER" || fail "package.json != $VER"
  grep -q "## \[$VER\]" CHANGELOG.md && pass "CHANGELOG has [$VER]" || fail "CHANGELOG missing [$VER]"
  grep -qi "wasm[^\n]*[0-9]\+\.[0-9]\+\.[0-9]\+" CHANGELOG.md && pass "CHANGELOG has WASM provenance line" || fail "CHANGELOG missing WASM provenance (which core tag?)"
  ;;
*) echo "unknown repo: $REPO"; exit 2 ;;
esac

if git rev-parse "v$VER" >/dev/null 2>&1; then fail "tag v$VER already exists locally"; else pass "tag v$VER free"; fi

echo "--- manual gates (verify by hand) ---"
echo "1. Release notes = current-version section ONLY (never full CHANGELOG)."
echo "2. Release created as DRAFT first; publish only after QA passes."
echo "3. Asset names carry the NEW version (spot-check one download)."
echo "4. Plugin: QA on real ImageGlass stable; fixtures zip present (5 files)."
echo "5. Dev-first: all fixes land on -Dev; public only via squash with explicit go."
echo "6. Post-publish: crates.io index / release assets / live-site WASM sha verified."
exit $FAIL
