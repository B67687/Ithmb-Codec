#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# run-bench-perf.sh — Run divan benchmarks under perf stat and save results.
#
# Usage:
#   ./scripts/run-bench-perf.sh                    # full bench + perf stats
#   ./scripts/run-bench-perf.sh --quick            # skip perf (just benchmarks)
#
# Output:
#   target/bench/perf-{timestamp}.txt   — perf stat output
#   target/bench/divan-{timestamp}.json — divan JSON results
#   target/bench/baseline.json          — latest baseline (overwritten)
#
# Requires:
#   perf  (optional — skipped if not found)
#   cargo (with divan bench targets)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
BENCH_DIR="$PROJECT_DIR/target/bench"

# ---------------------------------------------------------------------------
# Parse flags
# ---------------------------------------------------------------------------

PERF=true
FEATURES=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick) PERF=false; shift ;;
        --features)
            shift
            FEATURES="--features $1"
            shift
            ;;
        *) echo "Unknown flag: $1"; exit 1 ;;
    esac
done

mkdir -p "$BENCH_DIR"

# ---------------------------------------------------------------------------
# Run divan benchmarks
# ---------------------------------------------------------------------------

DIVAN_JSON="$BENCH_DIR/divan-$TIMESTAMP.json"
DIVAN_ARGS="--output-json=$DIVAN_JSON"
echo "=== Running divan benchmarks ==="
echo "  JSON output: $DIVAN_JSON"
echo "  Features:    ${FEATURES:-none}"

# If perf is requested and available, wrap cargo bench in perf stat
PERF_TXT="$BENCH_DIR/perf-$TIMESTAMP.txt"
if $PERF && command -v perf &>/dev/null; then
    echo "  perf stat:   $PERF_TXT"
    echo ""
    perf stat \
        -e cycles,instructions,cache-references,cache-misses,branch-misses \
        -o "$PERF_TXT" \
        cargo bench $FEATURES --profile bench -- "$DIVAN_ARGS"
    echo ""
    echo "=== perf summary ==="
    grep -E '(cycles|instructions|cache-misses|branch-misses|seconds)' "$PERF_TXT" | head -10
elif $PERF && ! command -v perf &>/dev/null; then
    echo "  perf:        not installed — skipping"
    cargo bench $FEATURES --profile bench -- "$DIVAN_ARGS"
else
    cargo bench $FEATURES --profile bench -- "$DIVAN_ARGS"
fi

# ---------------------------------------------------------------------------
# Update baseline symlink
# ---------------------------------------------------------------------------

BASELINE="$BENCH_DIR/baseline.json"
cp "$DIVAN_JSON" "$BASELINE"
echo ""
echo "=== Baseline updated ==="
echo "  $BASELINE"
echo ""
echo "=== Latency Distribution Analysis ==="
if command -v python3 &>/dev/null; then
    python3 "$PROJECT_DIR/tools/analysis/perf-stats.py" "$DIVAN_JSON" 2>/dev/null || \
    echo "  (perf-stats.py skipped — run manually: python3 tools/analysis/perf-stats.py $DIVAN_JSON)"
else
    echo "  (python3 not found — install to get p50/p95/p99 analysis)"
fi
echo ""
echo "Full results: $DIVAN_JSON"
