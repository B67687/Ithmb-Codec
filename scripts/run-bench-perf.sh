#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# run-bench-perf.sh — Run divan benchmarks under perf stat and save results.
#
# Usage:
#   ./scripts/run-bench-perf.sh                    # full bench + perf stats
#   ./scripts/run-bench-perf.sh --quick            # skip perf (just benchmarks)
#   ./scripts/run-bench-perf.sh --features simd    # with SIMD feature
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

# ---------------------------------------------------------------------------
# Extract summary table
# ---------------------------------------------------------------------------

echo ""
echo "=== Summary (throughput in GB/s) ==="
python3 -c "
import json
with open('$DIVAN_JSON') as f:
    data = json.load(f)
benches = data.get('results', [])
if isinstance(benches, dict):
    benches = [benches]
print(f\"{'Benchmark':30s} {'Size':10s} {'Throughput':>12s} {'Time':>10s}\")
print('-' * 62)
for b in benches:
    name = b.get('name', '') or b.get('id', '')
    # Divan JSON shape varies by version — try common keys
    for k in ('args', 'parameters', 'params'):
        if k in b:
            args = b[k]
            break
    else:
        args = ''
    thrpt = b.get('throughput', b.get('gbps', b.get('mbps', '')))
    time_ns = b.get('avg', b.get('mean', b.get('median', '')))
    time_us = ''
    if isinstance(time_ns, (int, float)):
        time_us = f'{time_ns/1000:.1f} us'
    thrpt_str = ''
    if isinstance(thrpt, (int, float)):
        thrpt_str = f'{thrpt:.2f} GB/s' if thrpt > 1 else f'{thrpt*1000:.1f} MB/s'
    print(f'{name:30s} {str(args):10s} {thrpt_str:>12s} {time_us:>10s}')
"
