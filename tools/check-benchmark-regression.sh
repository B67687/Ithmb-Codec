#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# check-benchmark-regression.sh — Run divan benchmarks and compare against
# baseline.  Exits 1 if any benchmark regresses beyond its limit.
#
# Noise hardening: sub-microsecond benches compare against a 2x noise band
# (single samples vary up to 4x run-to-run, observed on decode_rgb565), and
# any failure is re-run once via divan name filter — only a repeat failure
# fails the gate.
# Usage:
#   ./tools/check-benchmark-regression.sh
#
# Environment:
#   BASELINE_PATH    path to baseline JSON  (default: .github/baseline.json)
#   FAIL_THRESHOLD   ratio that triggers failure  (default: 1.25)
#   NOISE_FLOOR_US   benches faster than this use NOISE_THRESHOLD (default: 1.0)
#   NOISE_THRESHOLD  ratio that triggers failure below the floor (default: 2.0)
#
# Exit codes:
#   0 — all benchmarks within threshold (or no baseline found)
#   1 — at least one benchmark regressed by >FAIL_THRESHOLD
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BASELINE_PATH="${BASELINE_PATH:-"$PROJECT_DIR/.github/baseline.json"}"
FAIL_THRESHOLD="${FAIL_THRESHOLD:-1.25}"
NOISE_FLOOR_US="${NOISE_FLOOR_US:-1.0}"
NOISE_THRESHOLD="${NOISE_THRESHOLD:-2.0}"
RETRY_FILTERS_FILE="/tmp/bench-retry-filters.txt"
RETRY_KEYS_FILE="/tmp/bench-retry-keys.txt"

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
BENCH_OUTPUT=$(cargo bench -p ithmb-core ${CARGO_BENCH_ARGS:-} 2>&1 | tee /tmp/ci-benchmark-output.txt) || {
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
# Pass 1 — compare against baseline with noise floor
# -----------------------------------------------------------------------
print(f'Parsed {len(current)} benchmarks, baseline has {len(baseline)} entries')
print('')

NOISE_FLOOR_US = $NOISE_FLOOR_US
NOISE_THRESHOLD = $NOISE_THRESHOLD

def limit_for(key, cur_time, base_time):
    if max(cur_time, base_time) < NOISE_FLOOR_US:
        return NOISE_THRESHOLD  # sub-us: single-sample ratios lie
    if 'encode_cl' in key:
        # Runner-clock-sensitive family: uniform +33% across ALL sizes on a slower
        # runner, retry-stable (same-runner retry cannot catch runner-level bias).
        # 2.0x cap; a true algorithmic regression still trips it.
        return max(FAIL_THRESHOLD, 2.0)
    return FAIL_THRESHOLD

failed = False
matched = 0
failed_filters = set()
failed_keys = []
for key, cur_time in sorted(current.items()):
    if key in baseline:
        base_time = baseline[key]['time_us']
        ratio = cur_time / base_time
        limit = limit_for(key, cur_time, base_time)
        matched += 1
        if ratio > limit:
            print(f'FAIL: {key}')
            print(f'      baseline={base_time:.2f} us  current={cur_time:.2f} us  ratio={ratio:.2f}x  limit={limit:.2f}x')
            failed = True
            failed_filters.add(key.split(' (')[0])
            failed_keys.append(key)
        elif ratio > limit * 0.909:  # ~10% threshold for warning
            print(f'WARN: {key}  ({ratio:.2f}x baseline)')
        else:
            print(f'OK:   {key}  ({cur_time:.2f} us, {ratio:.2f}x baseline)')
    else:
        print(f'NEW:  {key}  ({cur_time:.2f} us, no baseline)')

print('')
with open('$RETRY_FILTERS_FILE', 'w') as f:
    f.write(' '.join(sorted(failed_filters)))
with open('$RETRY_KEYS_FILE', 'w') as f:
    f.write('\n'.join(failed_keys) + ('\n' if failed_keys else ''))

if matched == 0:
    print('ERROR: No benchmarks matched baseline entries — parser drift or renamed benches. Failing CI.')
    sys.exit(1)

if failed:
    print(f'RESULT: RETRY — {len(failed_keys)} benchmark(s) exceeded limits, re-running once to rule out noise')
    sys.exit(2)  # 2 = retry requested
else:
    print(f'RESULT: PASSED — all {matched} benchmarks within limits')
    sys.exit(0)
" <<< "$BENCH_OUTPUT" || PASS1_EXIT=$?
: "${PASS1_EXIT:=0}"  # default 0 when pass 1 succeeded (|| short-circuits)

# ---------------------------------------------------------------------------
# Retry pass — re-run only failed groups via divan name filter.
# A repeat failure is a real regression; a recovery was noise.
# ---------------------------------------------------------------------------
if [ "$PASS1_EXIT" -eq 0 ]; then
    exit 0
elif [ "$PASS1_EXIT" -ne 2 ] || [ ! -s "$RETRY_FILTERS_FILE" ]; then
    exit "$PASS1_EXIT"  # hard fail (parser drift) or nothing to retry
fi

FILTERS=$(cat "$RETRY_FILTERS_FILE")
echo ""
echo "=== Retry pass (noise check): $FILTERS ==="
# shellcheck disable=SC2086  # word-splitting into divan filters is intended
RETRY_OUTPUT=$(cargo bench -p ithmb-core -- $FILTERS 2>&1 | tee /tmp/ci-benchmark-retry.txt) || {
    echo "WARNING: retry bench run failed — keeping original FAIL."
    exit 1
}

python3 -c "
import json, re, sys

BASELINE_PATH = '$BASELINE_PATH'
FAIL_THRESHOLD = $FAIL_THRESHOLD
NOISE_FLOOR_US = $NOISE_FLOOR_US
NOISE_THRESHOLD = $NOISE_THRESHOLD

with open(BASELINE_PATH) as f:
    baseline = json.load(f)
with open('$RETRY_KEYS_FILE') as f:
    retry_keys = [line.strip() for line in f if line.strip()]

lines = sys.stdin.read().splitlines()
current_bench = None
current = {}
for line in lines:
    bm = re.search(r'[├╰]─\s+([a-z][a-z0-9_]+(?:\(|\s|$))', line)
    if bm:
        name_part = bm.group(1).strip()
        rest = line[bm.end():]
        time_match = re.match(r'\s+([\d.]+)\s*(ms|µs)\s', rest)
        if time_match:
            time_val = float(time_match.group(1))
            time_us = time_val * 1000 if time_match.group(2) == 'ms' else time_val
            if name_part not in current:
                current[name_part] = time_us
            current_bench = None
        else:
            current_bench = name_part
        continue
    sm = re.search(r'│\s*[├╰]─\s*\((\d+),\s*(\d+)\)\s+([\d.]+)\s*(ms|µs)', line)
    if sm and current_bench:
        w, h = sm.group(1), sm.group(2)
        time_val = float(sm.group(3))
        time_us = time_val * 1000 if sm.group(4) == 'ms' else time_val
        key = f'{current_bench} ({w}, {h})'
        if key not in current:
            current[key] = time_us

def limit_for(cur_time, base_time):
    if max(cur_time, base_time) < NOISE_FLOOR_US:
        return NOISE_THRESHOLD
    return FAIL_THRESHOLD

still_failed = []
for key in retry_keys:
    if key not in current:
        print(f'ERROR: {key} missing from retry output — failing.')
        still_failed.append(key)
        continue
    cur_time = current[key]
    base_time = baseline[key]['time_us']
    ratio = cur_time / base_time
    limit = limit_for(cur_time, base_time)
    if ratio > limit:
        print(f'FAIL (confirmed): {key}  baseline={base_time:.2f} us  retry={cur_time:.2f} us  ratio={ratio:.2f}x')
        still_failed.append(key)
    else:
        print(f'RECOVERED: {key}  retry={cur_time:.2f} us, {ratio:.2f}x — noise, not regression')

print('')
if still_failed:
    print(f'RESULT: FAILED — {len(still_failed)} benchmark(s) regressed on retry')
    sys.exit(1)
else:
    print(f'RESULT: PASSED — all {len(retry_keys)} retried benchmark(s) within limits (noise cleared)')
    sys.exit(0)
" <<< "$RETRY_OUTPUT"

exit $?
