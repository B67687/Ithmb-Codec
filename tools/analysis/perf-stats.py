#!/usr/bin/env python3
"""
perf-stats.py -- Decoder latency distribution analysis from Divan benchmark output.

Reads a JSON file (produced by `cargo bench -- --output-json=<path>`) or raw
Divan terminal output and computes per-benchmark latency statistics.

Usage:
    # Run benchmarks and save JSON:
    cargo bench -p ithmb-core --bench decoders -- --output-json=results.json

    # Analyze:
    python3 tools/analysis/perf-stats.py results.json

    # Pipe mode:
    cat results.json | python3 tools/analysis/perf-stats.py
"""

import json
import math
import re
import sys
from pathlib import Path

VB = "\u2502"
TREE = frozenset("\u2502\u251C\u2570\u2500\u252C\u250C")

UNIT_MULT = {"ns": 1.0, "us": 1000.0, "\u00b5s": 1000.0, "ms": 1_000_000.0, "s": 1_000_000_000.0}


def parse_time(s: str) -> float | None:
    s = s.strip()
    if not s or s in ("-", "\u2014"):
        return None
    if " " not in s:
        try:
            return float(s)
        except ValueError:
            return None
    v, u = s.rsplit(" ", 1)
    try:
        val = float(v.strip())
    except ValueError:
        return None
    unit = u.strip()
    mult = UNIT_MULT.get(unit)
    if mult is None:
        stripped = unit.rstrip("s")
        mult = {"n": 1.0, "u": 1000.0, "\u00b5": 1000.0, "m": 1_000_000.0}.get(stripped)
    return val * mult if mult else None


def strip_tree(s: str) -> str:
    while True:
        nxt = s.lstrip("".join(TREE)).lstrip()
        if len(nxt) == len(s):
            return s
        s = nxt


def split_name_value(s: str) -> tuple[str, str]:
    s = s.strip()
    if ")" in s:
        idx = s.rfind(")")
        name = s[: idx + 1].strip()
        rest = s[idx + 1 :].strip()
        if rest:
            return name, rest
    m = re.search(r"    +", s)
    if m:
        name = s[: m.start()].strip()
        rest = s[m.end() :].strip()
        if rest:
            return name, rest
    return s, ""


def parse_divan_output(text: str) -> list[dict]:
    results = []
    parent_stack: list[str] = []
    for line in text.splitlines():
        if line.startswith("Timer") or line.startswith("Warning") or line.startswith("warning"):
            continue
        st = line.lstrip()
        is_child = bool(st) and st[0] == VB
        content = strip_tree(st)
        if not content:
            continue
        has_vb = VB in content
        if not has_vb:
            name = content.strip()
            if name:
                if is_child:
                    if parent_stack:
                        parent_stack[-1] = name
                else:
                    parent_stack = [name]
            continue
        segments = content.split(VB)
        if len(segments) < 6:
            continue
        first = segments[0].strip()
        if not first:
            continue
        has_timing = any(s.strip() and s.strip() != "-" for s in segments[1:4])
        if not has_timing:
            name = first.strip()
            if name:
                if is_child:
                    if parent_stack:
                        parent_stack[-1] = name
                else:
                    parent_stack = [name]
            continue
        bench_name, fastest_str = split_name_value(first)
        if not fastest_str:
            continue
        # Build hierarchical name: parent decoder + child params
        if is_child and parent_stack:
            full_name = parent_stack[-1] + "::" + bench_name
        else:
            full_name = bench_name
        slowest_s = segments[1].strip()
        median_s = segments[2].strip()
        mean_s = segments[3].strip()
        samples_s = segments[4].strip()
        iters_s = segments[5].strip()
        fastest = parse_time(fastest_str)
        slowest = parse_time(slowest_s)
        median = parse_time(median_s)
        mean = parse_time(mean_s)
        if None in (fastest, slowest, median, mean):
            continue
        samples = int(samples_s.replace(",", "")) if samples_s else 0
        iters = int(iters_s.replace(",", "")) if iters_s else 0
        results.append(
            {
                "name": full_name,
                "fastest_ns": fastest,
                "slowest_ns": slowest,
                "median_ns": median,
                "mean_ns": mean,
                "samples": samples,
                "iters": iters,
            }
        )
    return results


def compute_stats(ns_values: list[float]) -> dict:
    n = len(ns_values)
    if n == 0:
        return {}
    sv = sorted(ns_values)
    p50 = sv[n // 2]
    mean = sum(ns_values) / n
    mn = sv[0]
    mx = sv[-1]
    stddev = math.sqrt(sum((x - mean) ** 2 for x in ns_values) / max(n - 1, 1))
    p95 = sv[min(int(0.95 * n), n - 1)]
    p99 = sv[min(int(0.99 * n), n - 1)]
    return {
        "p50_ns": p50,
        "p95_ns": p95,
        "p99_ns": p99,
        "mean_ns": mean,
        "min_ns": mn,
        "max_ns": mx,
        "stddev_ns": stddev,
        "count": n,
    }


def fmt_time(ns: float) -> str:
    if ns >= 1_000_000_000:
        return f"{ns / 1_000_000_000:.3f} s"
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.3f} ms"
    if ns >= 1000:
        return f"{ns / 1000:.3f} us"
    return f"{ns:.0f} ns"


def main():
    if len(sys.argv) > 1 and sys.argv[1] in ("-h", "--help"):
        print(__doc__.strip())
        return
    if len(sys.argv) > 1:
        data = json.loads(Path(sys.argv[1]).read_text())
    else:
        data = json.load(sys.stdin)
    raw = data.get("raw_output", "")
    results = parse_divan_output(raw) if raw else data.get("results", [])
    if not results:
        print("No results.")
        return
    grouped = {}
    for r in results:
        parts = r["name"].split("::")
        dec = parts[0] if len(parts) > 1 else r["name"]
        grouped.setdefault(dec, []).append(r)
    print("# Decoder Latency Distribution\n")
    print("| Decoder | Params | p50 | p95 | p99 | Mean | Min | Max | Stddev |")
    print("|---------|--------|-----|-----|-----|------|-----|-----|--------|")
    for dec in sorted(grouped):
        for e in sorted(grouped[dec], key=lambda x: x["name"]):
            params = e["name"].split("::", 1)[-1] if "::" in e["name"] else e["name"]
            s = compute_stats([e["fastest_ns"], e["slowest_ns"], e["median_ns"], e["mean_ns"]])
            print(
                f"| {dec:25s} | {params:12s}"
                f" | {fmt_time(s['p50_ns']):>10s} | {fmt_time(s['p95_ns']):>10s}"
                f" | {fmt_time(s['p99_ns']):>10s} | {fmt_time(s['mean_ns']):>10s}"
                f" | {fmt_time(s['min_ns']):>10s} | {fmt_time(s['max_ns']):>10s}"
                f" | {fmt_time(s['stddev_ns']):>10s} |"
            )
    print()
    print(
        "*Note*: p95/p99 estimated from 4-point summary (fastest/slowest/median/mean). "
        "Accurate high-percentile analysis requires raw per-iteration samples."
    )


if __name__ == "__main__":
    main()
