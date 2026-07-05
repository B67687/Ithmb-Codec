#!/usr/bin/env python3
"""Check that stdin has zero warnings/errors. Exits 1 if any found."""

import sys
import re

lines = sys.stdin.read().splitlines()
has_issues = False
for line in lines:
    if re.match(r"^(warning|error)\b", line):
        print(line, file=sys.stderr)
        has_issues = True

if has_issues:
    print("FAILED: warnings or errors found", file=sys.stderr)
    sys.exit(1)
