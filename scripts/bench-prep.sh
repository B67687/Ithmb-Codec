#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# bench-prep.sh — Prepare CPU state for reproducible benchmarks.
#
# Usage:
#   ./scripts/bench-prep.sh          # show-and-set (root) or show-only (user)
#   sudo ./scripts/bench-prep.sh     # full setup
#
# On non-Linux or systems without cpufreq sysfs this exits silently.

set -euo pipefail

SYSFS="/sys/devices/system/cpu"

announce() { printf "\033[36m%s\033[0m\n" "$*"; }
ok()       { printf "\033[32m  ✓ %s\033[0m\n" "$*"; }
skip()     { printf "\033[33m  - %s (skipped — not root)\033[0m\n" "$*"; }
warn()     { printf "\033[31m  ! %s\033[0m\n" "$*"; }

# ---------------------------------------------------------------------------
# Detect platform
# ---------------------------------------------------------------------------

if [[ ! -d "$SYSFS" ]]; then
    echo "bench-prep: cpufreq sysfs not found at $SYSFS — nothing to configure."
    exit 0
fi

readonly HAS_PERF=$(command -v perf &>/dev/null && echo true || echo false)
readonly IS_ROOT=$([[ $EUID -eq 0 ]] && echo true || echo false)

# ---------------------------------------------------------------------------
# Print current state
# ---------------------------------------------------------------------------

print_state() {
    local label="$1"
    echo ""
    announce "=== CPU state ($label) ==="

    # governor
    local gov
    gov="$(cat "$SYSFS/cpu0/cpufreq/scaling_governor" 2>/dev/null || echo "n/a")"
    echo "  governor:        $gov"

    # frequency
    local freq
    freq="$(cat "$SYSFS/cpu0/cpufreq/scaling_cur_freq" 2>/dev/null || echo "n/a")"
    if [[ "$freq" != "n/a" ]]; then
        freq="$((freq / 1000)) MHz"
    fi
    echo "  frequency:       $freq"

    # turbo / boost
    local turbo="n/a"
    if [[ -f "$SYSFS/intel_pstate/no_turbo" ]]; then
        turbo="$(cat "$SYSFS/intel_pstate/no_turbo")"
        if [[ "$turbo" == "0" ]]; then turbo="enabled"; else turbo="disabled ($turbo)"; fi
    elif [[ -f "$SYSFS/cpufreq/boost" ]]; then
        turbo="$(cat "$SYSFS/cpufreq/boost")"
        if [[ "$turbo" == "1" ]]; then turbo="enabled"; else turbo="disabled ($turbo)"; fi
    fi
    echo "  turbo/boost:     $turbo"

    # scaling driver
    local driver
    driver="$(cat "$SYSFS/cpu0/cpufreq/scaling_driver" 2>/dev/null || echo "n/a")"
    echo "  scaling driver:  $driver"

    if $HAS_PERF; then
        echo "  perf:            available"
    else
        echo "  perf:            not installed (optional)"
    fi
}

print_state "before"

# ---------------------------------------------------------------------------
# Apply settings
# ---------------------------------------------------------------------------

if $IS_ROOT; then
    echo ""
    announce "=== Applying settings ==="

    # Set performance governor for all CPUs
    for cpu in "$SYSFS/cpu"*[0-9]/cpufreq/scaling_governor; do
        if [[ -f "$cpu" ]]; then
            echo "performance" > "$cpu" 2>/dev/null || true
        fi
    done
    ok "scaling_governor set to 'performance' on all CPUs"

    # Disable turbo boost
    if [[ -f "$SYSFS/intel_pstate/no_turbo" ]]; then
        echo 1 > "$SYSFS/intel_pstate/no_turbo"
        ok "turbo boost disabled (intel_pstate)"
    elif [[ -f "$SYSFS/cpufreq/boost" ]]; then
        echo 0 > "$SYSFS/cpufreq/boost"
        ok "turbo boost disabled (cpufreq/boost)"
    else
        warn "no turbo control file found"
    fi

    echo ""
    print_state "after"
else
    echo ""
    skip "scaling_governor (needs root)"
    skip "turbo boost control (needs root)"
fi

echo ""
if $IS_ROOT; then
    announce "Benchmark CPU state: ready"
else
    announce "Benchmark CPU state: nominal (no changes made — re-run with sudo for full control)"
fi
