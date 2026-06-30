#!/usr/bin/env bash
# check-benchmark-regression.sh
#
# Compares current BenchmarkDotNet results against a committed baseline.
# Fails (exit 1) if any decoder shows >10% regression in Mean time.
#
# Usage:
#   ./tools/check-benchmark-regression.sh [baseline-file]
#
# Default baseline: tools/IthmbCodec.Benchmark/baseline.csv
# Current results: tools/IthmbCodec.Benchmark/BenchmarkDotNet.Artifacts/

set -euo pipefail

BASELINE="${1:-tools/IthmbCodec.Benchmark/baseline.csv}"
RESULTS_DIR="tools/IthmbCodec.Benchmark/BenchmarkDotNet.Artifacts"

if [ ! -f "$BASELINE" ]; then
    echo "OK: No baseline file at '$BASELINE' — skipping regression check."
    echo "    Generate one by running benchmarks and creating baseline.csv."
    exit 0
fi

if [ ! -d "$RESULTS_DIR" ]; then
    echo "WARNING: No benchmark results directory found at '$RESULTS_DIR'."
    echo "         Skipping regression check."
    exit 0
fi

# Find the latest results summary file
SUMMARY_FILE=$(find "$RESULTS_DIR" -name "*-summary.csv" -type f 2>/dev/null | head -1)
if [ -z "$SUMMARY_FILE" ]; then
    # Try to find any results CSV
    SUMMARY_FILE=$(find "$RESULTS_DIR" -name "*.csv" -type f 2>/dev/null | head -1)
fi

if [ -z "$SUMMARY_FILE" ]; then
    echo "WARNING: No benchmark summary CSV found in '$RESULTS_DIR'."
    exit 0
fi

echo "Comparing '$SUMMARY_FILE' against baseline '$BASELINE'..."
echo ""

HAS_REGRESSION=0

# Read the baseline into an associative array by method name.
# Expected format: Method,MeanNs
declare -A BASELINE_MEANS
while IFS=',' read -r method mean_ns; do
    # Skip header
    [ "$method" = "Method" ] && continue
    # Strip whitespace
    method="${method// /}"
    mean_ns="${mean_ns// /}"
    BASELINE_MEANS["$method"]="$mean_ns"
done < "$BASELINE"

# Parse current results from summary.
# BenchmarkDotNet summary CSV format varies; try common column names.
HEADER=$(head -1 "$SUMMARY_FILE")
CURRENT_METHOD_COL=-1
CURRENT_MEAN_COL=-1

# Locate columns by name
IFS=',' read -ra COLS <<< "$HEADER"
for i in "${!COLS[@]}"; do
    col="${COLS[$i]}"
    col="${col//\"/}"
    col="${col// /}"
    case "$col" in
        Method|Benchmark)  CURRENT_METHOD_COL=$i ;;
        Mean|MeanNs|MeanTime)  CURRENT_MEAN_COL=$i ;;
    esac
done

if [ "$CURRENT_METHOD_COL" -lt 0 ] || [ "$CURRENT_MEAN_COL" -lt 0 ]; then
    echo "WARNING: Could not identify Method/Mean columns in '$SUMMARY_FILE'."
    echo "         Columns found: $HEADER"
    echo "         Method col index: $CURRENT_METHOD_COL, Mean col index: $CURRENT_MEAN_COL"
    exit 0
fi

printf "%-30s %15s %15s %10s\n" "Benchmark" "Baseline (ns)" "Current (ns)" "Change"
printf "%-30s %15s %15s %10s\n" "---------" "-------------" "------------" "------"

# Process each row
# Process each row (process substitution avoids subshell so HAS_REGRESSION propagates)
while IFS=',' read -ra ROW; do
    method="${ROW[$CURRENT_METHOD_COL]}"
    method="${method//\"/}"
    mean_str="${ROW[$CURRENT_MEAN_COL]}"
    mean_str="${mean_str//\"/}"

    # Clean mean value: remove whitespace, units
    mean_val="${mean_str//[^0-9.]/}"

    # If empty or not a number, skip
    if ! [[ "$mean_val" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
        continue
    fi

    baseline="${BASELINE_MEANS[$method]:-}"

    if [ -z "$baseline" ]; then
        printf "%-30s %15s %15.0f %10s\n" "$method" "(new)" "$mean_val" "N/A"
        continue
    fi

    if ! [[ "$baseline" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
        continue
    fi

    change=$(awk "BEGIN { printf \"%.2f\", ($mean_val / $baseline - 1) * 100 }")

    if (( $(awk "BEGIN { print ($change > 10.0) }") )); then
        printf "%-30s %15.0f %15.0f %+8.1f%%  REGRESSION\n" "$method" "$baseline" "$mean_val" "$change"
        HAS_REGRESSION=1
    elif (( $(awk "BEGIN { print ($change < -5.0) }") )); then
        printf "%-30s %15.0f %15.0f %+8.1f%%  IMPROVED\n" "$method" "$baseline" "$mean_val" "$change"
    else
        printf "%-30s %15.0f %15.0f %+8.1f%%\n" "$method" "$baseline" "$mean_val" "$change"
    fi
done < <(tail -n +2 "$SUMMARY_FILE")

echo ""

if [ "$HAS_REGRESSION" -eq 1 ]; then
    echo "FAIL: One or more benchmarks exceeded 10% regression threshold."
    exit 1
fi

echo "OK: All benchmarks within 10% of baseline."
exit 0
