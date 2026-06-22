#!/usr/bin/env bash
# test-neon-locally.sh — Run NEON SIMD tests under ARM64 QEMU user-mode
#
# Purpose: validate NEON-accelerated decode paths without native ARM64 hardware.
# Uses QEMU user-mode with binfmt support to execute ARM64 .NET test binaries.
#
# Prerequisites:
#   1. QEMU user-mode + binfmt:  sudo apt install qemu-user-static binfmt-support
#   2. ARM64 .NET SDK:           ./scripts/setup-dotnet-arm64.sh  (manual, see below)
#
# Usage:
#   bash scripts/test-neon-locally.sh
#
# The existing SIMD-vs-scalar identity tests (w=8 vs w<8) naturally exercise the
# NEON paths under ARM64, because AdvSimd.IsSupported is true and the dispatcher
# routes to the NEON implementation instead of SSE2.

set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==========================================="
echo "  NEON ARM64 Local Test (QEMU user-mode)"
echo "==========================================="

# ---- Check QEMU ----
if ! command -v qemu-aarch64 &>/dev/null; then
  echo ""
  echo "ERROR: qemu-aarch64 not found."
  echo ""
  echo "  Install it:"
  echo "    sudo apt install qemu-user-static binfmt-support"
  echo ""
  echo "  Or skip local testing and push to GitHub — the test-neon CI workflow"
  echo "  will run on a native ARM64 runner (ubuntu-24.04-arm)."
  echo ""
  exit 1
fi

# ---- Check binfmt ----
if [ ! -f /proc/sys/fs/binfmt_misc/qemu-aarch64 ]; then
  echo "WARNING: binfmt support for aarch64 not registered."
  echo "  Run: sudo apt install qemu-user-static"
  echo "  (postinst scripts register the binfmt entries)"
  echo ""
  echo "  Attempting to continue anyway (will likely fail)..."
fi

# ---- Check ARM64 .NET SDK ----
if ! dotnet --list-sdks 2>/dev/null | grep -q "10.0"; then
  echo ""
  echo "WARNING: .NET 10 SDK not found on PATH."
  echo "  The ARM64 test requires the ARM64 .NET runtime."
  echo ""
  echo "  To install the ARM64 .NET SDK for QEMU:"
  echo "    wget https://dot.net/v1/dotnet-install.sh -O /tmp/dotnet-install.sh"
  echo "    chmod +x /tmp/dotnet-install.sh"
  echo "    /tmp/dotnet-install.sh --channel 10.0 --arch arm64 --install-dir ~/.dotnet-arm64"
  echo "    export DOTNET_ROOT=~/.dotnet-arm64"
  echo "    export PATH=~/.dotnet-arm64:\$PATH"
  echo ""
fi

# ---- Publish ARM64 self-contained test ----
echo ""
echo "Publishing tests for linux-arm64 (self-contained)..."
dotnet publish src/IthmbCodec/test/IthmbCodec.Tests.csproj \
  -c Release \
  -r linux-arm64 \
  --self-contained true \
  -o /tmp/ithmb-test-arm64 \
  /p:PublishAot=false

echo ""
echo "Running tests under QEMU ARM64..."
echo ""

# QEMU args:
#   -L /usr/aarch64-linux-gnu  — sysroot for ARM64 shared libs
#   -E LD_LIBRARY_PATH         — forward library path
SYSROOT="/usr/aarch64-linux-gnu"
if [ -d "$SYSROOT" ]; then
  QEMU_LD_PREFIX="$SYSROOT" qemu-aarch64 \
    /tmp/ithmb-test-arm64/IthmbCodec.Tests \
    --filter "FullyQualifiedName~SIMD" \
    -nocolor
else
  echo "NOTE: No ARM64 sysroot at $SYSROOT — QEMU may fail if libraries are missing."
  qemu-aarch64 /tmp/ithmb-test-arm64/IthmbCodec.Tests \
    --filter "FullyQualifiedName~SIMD" \
    -nocolor
fi

EXIT_CODE=$?
echo ""
if [ $EXIT_CODE -eq 0 ]; then
  echo "✅ NEON tests passed under QEMU ARM64"
else
  echo "❌ NEON tests FAILED under QEMU ARM64 (exit: $EXIT_CODE)"
fi
exit $EXIT_CODE
