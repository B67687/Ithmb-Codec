# ADR-0010: Whole-Codebase SAFETY Review (Per-Block Contracts + UB Fixes)

**Status:** Accepted (2026-09-07)

## Context

86 unsafe sites across 24 Codec files (SIMD kernels + C-API boundary) had
accumulated without uniform safety documentation. A systematic review visited
every site (the plugin's 108 sites across 9 files were reviewed in the same
round; plugin fixes F-P1/F-P2/F-P3 are recorded in that repo).

## Decision

1. **Per-block contracts.** Every unsafe site now carries `# Safety` /
   `// SAFETY:` documenting its invariant. Contracts are living code docs,
   reviewed with the code they guard.
2. **Fix the real bugs found, minimally:**
    - F3: CLCL hot-row `[u8;8]` → `[u8;16]` (genuine stack-overflow UB —
      tests never caught it).
    - F1: UYVY quad dispatch gated on runtime `sse4.1` (was illegal
      instruction on older CPUs).
    - F2: partial quad rows (`n%4`) return `BufferTooShort` at the row-fn top.
    - Q1: dead x86 (non-64) cfgs narrowed to x86_64 across 16 files.
3. **Visibility least-privilege.** `pub` → `pub(super)`/private
   (`unreachable_pub` 59 → 0) — smaller public surface, fewer audit targets.

## Consequences

- `cargo clippy` + the SAFETY contracts are the ongoing enforcement; no new
  CI job was needed.
- Miri remains unavailable on GitHub runners (jailed child); fuzz (6 targets,
  restored) is the automated UB backstop alongside review discipline.
