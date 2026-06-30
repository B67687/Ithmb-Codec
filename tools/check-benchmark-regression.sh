#!/bin/bash
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

# Find all report CSV files (one per benchmark class)
RESULTS_CSVS=$(find "$RESULTS_DIR" -name "*-report.csv" -type f 2>/dev/null)
if [ -z "$RESULTS_CSVS" ]; then
    echo "WARNING: No benchmark report CSVs found in '$RESULTS_DIR'."
    exit 0
fi

CSV_COUNT=$(echo "$RESULTS_CSVS" | wc -l | tr -d ' ')
echo "Comparing $CSV_COUNT report CSV(s) against baseline '$BASELINE'..."
echo ""

HAS_REGRESSION=0

# ---------------------------------------------------------------------------
# Step 1: Read baseline into associative array (Method -> MeanNs)
# ---------------------------------------------------------------------------
declare -A BASELINE_MEANS
while IFS=',' read -r method mean_ns; do
    [ "$method" = "Method" ] && continue
    method="${method// /}"
    mean_ns="${mean_ns// /}"
    BASELINE_MEANS["$method"]="$mean_ns"
done < "$BASELINE"

# ---------------------------------------------------------------------------
# Step 2: Convert a BenchmarkDotNet mean string to nanoseconds
# ---------------------------------------------------------------------------
# BenchmarkDotNet embeds unit suffixes (ns, us, μs, ms) in the Mean column.
to_ns() {
    local val="$1"
    val="${val//[^0-9.]/}"   # strip non-numeric except dot
    if [[ "$1" == *ms ]]; then
        # milliseconds -> ns
        awk "BEGIN { printf \"%.0f\", ${val} * 1000000 }"
    elif [[ "$1" == *μs ]] || [[ "$1" == *us ]]; then
        # microseconds -> ns
        awk "BEGIN { printf \"%.0f\", ${val} * 1000 }"
    else
        # already ns or no unit
        printf "%.0f" "$val"
    fi
}

# ---------------------------------------------------------------------------
# Step 3: Collect current method/mean values from ALL report CSVs
# ---------------------------------------------------------------------------
# Strategy: for each CSV, read its header to find Method/Mean columns,
# then emit "Method,MeanNs" lines which we aggregate into a combined loop.
declare -A CURRENT_MEANS

collect_current() {
    local csv
    while IFS= read -r csv; do
        [ -z "$csv" ] && continue
        local hdr method_col mean_col
        hdr=$(head -1 "$csv")
        IFS=',' read -ra COLS <<< "$hdr"
        method_col=-1
        mean_col=-1
        for i in "${!COLS[@]}"; do
            local col="${COLS[$i]}"
            col="${col//\"/}"
            col="${col// /}"
            case "$col" in
                Method|Benchmark) method_col=$i ;;
                Mean|MeanNs|MeanTime) mean_col=$i ;;
            esac
        done
        [ "$method_col" -lt 0 ] || [ "$mean_col" -lt 0 ] && continue
        # Read data rows (skip header)
        while IFS=',' read -ra ROW2; do
            local method="${ROW2[$method_col]}"
            method="${method//\"/}"
            local mean_str="${ROW2[$mean_col]}"
            mean_str="${mean_str//\"/}"
            echo "${method}|${mean_str}"
        done < <(tail -n +2 "$csv")
    done < <(printf '%s\n' "$RESULTS_CSVS")
}

while IFS='|' read -r method mean_str; do
    mean_val=$(to_ns "$mean_str")
    if [[ "$mean_val" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
        CURRENT_MEANS["$method"]="$mean_val"
    fi
done < <(collect_current)

# ---------------------------------------------------------------------------
# Step 4: Print comparison table
# ---------------------------------------------------------------------------
printf "%-30s %15s %15s %10s\n" "Benchmark" "Baseline (ns)" "Current (ns)" "Change"
printf "%-30s %15s %15s %10s\n" "---------" "-------------" "------------" "------"

for method in "${!CURRENT_MEANS[@]}"; do
    mean_val="${CURRENT_MEANS[$method]}"
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
done

echo ""

if [ "$HAS_REGRESSION" -eq 1 ]; then
    echo "FAIL: One or more benchmarks exceeded 10% regression threshold."
    exit 1
fi

echo "OK: All benchmarks within 10% of baseline."
exit 0
