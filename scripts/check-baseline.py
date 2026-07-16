#!/usr/bin/env python3
"""Compare divan benchmark output against a JSON baseline.

Usage:
  FAIL_THRESHOLD=3.0 cargo bench -p ithmb-core --bench decoders 2>&1 | scripts/check-baseline.py
Exit codes:
  0 — all within threshold (or no baseline found)
  1 — at least one benchmark regressed by >FAIL_THRESHOLD
"""

import json
import os
import re
import sys

BASELINE_PATH = ".github/baseline.json"
FAIL_THRESHOLD = float(os.environ.get("FAIL_THRESHOLD", "5.0"))
WARN_THRESHOLD = float(os.environ.get("WARN_THRESHOLD", "3.0"))


def parse_current(input_lines: list[str]) -> dict[str, float]:
    """Parse divan grouped output into {benchmark_name: median_time_us}.

    Divan grouped output format:

        ├─ decode_rgb565
        │  ├─ (64, 64)              0.5016 µs      ...
        │  ├─ (256, 256)            7.513 µs       ...
        │  ╰─ (720, 480)            44.21 µs       ...

    The first column after the size is the median time (what we want).
    """
    current: dict[str, float] = {}
    current_bench: str | None = None

    # Match bench group header: ├─ decode_<name> or ╰─ decode_<name> (possibly with trailing spaces)
    bench_pat = re.compile(r"[├╰]─\s+(decode_[a-z0-9_]+)\s")

    # Match size line: ├─ (<W>, <H>) or ╰─ (<W>, <H>)
    # The first number after the closing paren is the median time in µs
    size_pat = re.compile(r"[├╰]─\s*\((\d+),\s*(\d+)\)\s+([\d.]+)\s*[µu]s")

    for line in input_lines:
        bm = bench_pat.search(line)
        if bm:
            current_bench = bm.group(1)
            continue

        sm = size_pat.search(line)
        if sm and current_bench:
            w, h = sm.group(1), sm.group(2)
            time_us = float(sm.group(3))
            key = f"{current_bench} ({w}, {h})"
            # Only keep the first occurrence (median) per key
            if key not in current:
                current[key] = time_us

    return current


def main() -> None:
    input_lines = sys.stdin.read().splitlines()

    try:
        with open(BASELINE_PATH) as f:
            baseline = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError) as e:
        print(f"Baseline regression check: SKIP (no baseline: {e})")
        sys.exit(0)

    current = parse_current(input_lines)

    if not current:
        print("Baseline regression check: SKIP (no benchmark results parsed)")
        sys.exit(0)

    failed = False
    for key, cur_time in sorted(current.items()):
        if key in baseline:
            base_time = baseline[key]["time_us"]
            ratio = cur_time / base_time
            if ratio > FAIL_THRESHOLD:
                print(
                    f"FAIL: {key} {base_time:.1f} us -> {cur_time:.1f} us "
                    f"({ratio:.2f}x)"
                )
                failed = True
            elif ratio > WARN_THRESHOLD:
                print(
                    f"WARN: {key} {base_time:.1f} us -> {cur_time:.1f} us "
                    f"({ratio:.2f}x)"
                )
            else:
                print(f"OK:   {key} {base_time:.1f} us -> {cur_time:.1f} us")
        else:
            print(f"NEW:  {key} {cur_time:.1f} us (no baseline)")

    if failed:
        print("Baseline regression check: FAILED")
        sys.exit(1)
    else:
        print("Baseline regression check: PASSED")


if __name__ == "__main__":
    main()
