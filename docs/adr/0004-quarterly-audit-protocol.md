# ADR-0004: Quarterly Adversarial Audit Protocol

**Status**: Accepted (2026-07-01)
**Context**: The v1.5.0 and v1.6.0 audits independently discovered 28 source bugs,
5 CI config issues, and 8 documentation errors. None of these were caught by CI
gates or existing tests — they required a dedicated adversarial review by someone
unfamiliar with the recent changes.

## Decision

Adopt the Layered Retrospective Protocol from the methodology repo as a
quarterly adversarial audit. Schedule: first Monday of January, April, July, October.

## Audit Protocol (4 phases, ~60 minutes)

### Phase 1 — Collect Evidence (parallel, 15 min)

Dispatch 4 research agents against the current codebase:

| Agent | What to examine |
|-------|----------------|
| Code structure | Architecture, file sizes, patterns, code smells, naming |
| Testing & CI | Test pyramid, coverage, CI workflows, gates |
| Process & docs | CHANGELOG, README, ADRs, commit history |
| External | Dependencies, upstream changes, security advisories |

### Phase 2 — Expand & Verify (10 min)

Cross-reference agent reports for contradictions and overlaps.
Verify key claims by reading actual code or running tests.

### Phase 3 — Layer Extraction (15 min)

Extract findings into 3 layers:
- **Layer 1**: Concrete bugs, regressions, gaps found
- **Layer 2**: Patterns — what recurred since last audit
- **Layer 3**: Meta-patterns — why patterns exist

### Phase 4 — Write Artifacts (20 min)

- Update docs/standards/STANDARDS.md with new gaps found
- File GitHub issues for each actionable finding
- Create new ADRs for decisions made during audit

## Checklist (also in issue template)

- [ ] All 4 agents dispatched and results collected
- [ ] Claims verified against actual code
- [ ] New bugs filed as GitHub issues
- [ ] docs/standards/STANDARDS.md updated
- [ ] New ADRs created for design decisions
- [ ] Audit results shared with team

## Expected Outcomes

- 5-15 findings per audit (based on v1.5.0 and v1.6.0 experience)
- At least 1 actionable CI gate or automation improvement
- Zero findings that became production incidents (goal)
