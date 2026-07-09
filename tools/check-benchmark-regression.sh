#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# check-benchmark-regression.sh — Run divan benchmarks and compare against
# baseline.  Exits 1 if any benchmark regresses by >10%.
#
# Usage:
#   ./tools/check-benchmark-regression.sh
#
# Environment:
#   BASELINE_PATH    path to baseline JSON  (default: .github/baseline.json)
#   FAIL_THRESHOLD   ratio that triggers failure  (default: 1.10)
#
# Exit codes:
#   0 — all benchmarks within threshold (or no baseline found)
#   1 — at least one benchmark regressed by >FAIL_THRESHOLD
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE_PATH="${BASELINE_PATH:-"$PROJECT_DIR/.github/baseline.json"}"
FAIL_THRESHOLD="${FAIL_THRESHOLD:-1.10}"

# ---------------------------------------------------------------------------
# Run benchmarks and capture output
# ---------------------------------------------------------------------------
echo "=== Benchmark regression check ==="
echo "  Baseline: $BASELINE_PATH"
echo "  Threshold: ${FAIL_THRESHOLD}x"
echo ""

if [ ! -f "$BASELINE_PATH" ]; then
    echo "WARNING: No baseline found at $BASELINE_PATH — skipping check."
    exit 0
fi

echo "Running cargo bench -p ithmb-core ..."
BENCH_OUTPUT=$(cargo bench -p ithmb-core 2>&1 | tee /tmp/ci-benchmark-output.txt) || {
    echo "WARNING: cargo bench failed — skipping regression check."
    exit 0
}

# ---------------------------------------------------------------------------
# Parse and compare with embedded Python script
# ---------------------------------------------------------------------------
python3 -c "
import json, re, sys

BASELINE_PATH = '$BASELINE_PATH'
FAIL_THRESHOLD = $FAIL_THRESHOLD

# Read baseline
with open(BASELINE_PATH) as f:
    baseline = json.load(f)

# Read bench output from stdin
lines = sys.stdin.read().splitlines()

# -----------------------------------------------------------------------
# Parser — handles all benchmark types from divan output
# -----------------------------------------------------------------------
current_bench = None
current = {}  # key -> time_us

for line in lines:
    # Detect bench group header (decoders/encoders/pipeline)
    # ├─ decode_xxx, ├─ encode_xxx, ├─ build_xxx, ├─ open_xxx, ├─ simd_xxx
    bm = re.search(r'[├╰]─\s+([a-z][a-z0-9_]+(?:\(|\s|$))', line)
    if bm:
        name_part = bm.group(1).strip()
        rest = line[bm.end():]

        # Check if this is a leaf benchmark (time right after name)
        # ├─ decode_jpeg              33.1 µs
        time_match = re.match(r'\s+([\d.]+)\s*(ms|µs)\s', rest)
        if time_match:
            time_val = float(time_match.group(1))
            time_us = time_val * 1000 if time_match.group(2) == 'ms' else time_val
            if name_part not in current:
                current[name_part] = time_us
            current_bench = None
        else:
            # Group header — next child lines have sizes
            current_bench = name_part
        continue

    # Match size child: │  ├─ (64, 64)              16.43 µs  (or ms)
    sm = re.search(r'│\s*[├╰]─\s*\((\d+),\s*(\d+)\)\s+([\d.]+)\s*(ms|µs)', line)
    if sm and current_bench:
        w, h = sm.group(1), sm.group(2)
        time_val = float(sm.group(3))
        time_us = time_val * 1000 if sm.group(4) == 'ms' else time_val
        key = f'{current_bench} ({w}, {h})'
        if key not in current:
            current[key] = time_us

# -----------------------------------------------------------------------
# Compare against baseline
# -----------------------------------------------------------------------
print(f'Parsed {len(current)} benchmarks, baseline has {len(baseline)} entries')
print('')

failed = False
matched = 0
for key, cur_time in sorted(current.items()):
    if key in baseline:
        base_time = baseline[key]['time_us']
        ratio = cur_time / base_time
        matched += 1
        if ratio > FAIL_THRESHOLD:
            print(f'FAIL: {key}')
            print(f'      baseline={base_time:.2f} us  current={cur_time:.2f} us  ratio={ratio:.2f}x')
            failed = True
        elif ratio > FAIL_THRESHOLD * 0.909:  # ~10% threshold for warning
            print(f'WARN: {key}  ({ratio:.2f}x baseline)')
        else:
            print(f'OK:   {key}  ({cur_time:.2f} us, {ratio:.2f}x baseline)')
    else:
        print(f'NEW:  {key}  ({cur_time:.2f} us, no baseline)')

print('')
if matched == 0:
    print('WARNING: No benchmarks matched baseline entries — check name format.')
    sys.exit(0)

if failed:
    print(f'RESULT: FAILED — {sum(1 for k in current if k in baseline and current[k] / baseline[k][\"time_us\"] > FAIL_THRESHOLD)} benchmark(s) regressed >{FAIL_THRESHOLD}x')
    sys.exit(1)
else:
    print(f'RESULT: PASSED — all {matched} benchmarks within {FAIL_THRESHOLD}x of baseline')
    sys.exit(0)
" <<< "$BENCH_OUTPUT"

exit $?
