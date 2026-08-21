# PROJECT_MODEL.md: The Ithmb-Codec Whole-Project State Machine

> **Purpose:** This document is the project's health contract. Every feature addition, bugfix, or release is a state transition; the transition table below catches invariant violations at spec time, not as production regressions. Adding or removing a state means updating this table, making the change explicit instead of silently breaking the pipeline's invariants.
>
> **Why this exists:** SPECIFICATION.md section 2 mandates that every project document its whole-project state machine. This is that mandate applied to Ithmb-Codec.

## States

The project's lifecycle states, mapped to the real history of this codebase:

```
IDEA -> SPEC'D -> PROTOTYPED -> IMPLEMENTED -> POLISHED -> SHIPPED -> MAINTAINED -> EVOLVED
```

| State | Meaning |
|---|---|
| `IDEA` | Seed ambition: "I want to open .ithmb thumbnail files in ImageGlass on Windows" (docs/RETROSPECTIVE_SPEC.md Layer 0) |
| `SPEC'D` | Format research complete: 33 open-source implementations surveyed, format tables extracted from iOpenPod, libgpod, Keith's iPod Photo Reader |
| `PROTOTYPED` | C# Native AOT plugin prototype for ImageGlass v10 (B67687/Ithmb-Codec-CSharp, archived) |
| `IMPLEMENTED` | Rust workspace port: 8 decoders, 53 profiles, PhotoDB parser, 7 encoders, CLI, WASM, Python, C ABI |
| `POLISHED` | Hardening: 5 review rounds (~47 findings fixed), fuzz, Miri, benchmark regression baseline, CI pipeline complete |
| `SHIPPED` | Release pipeline live: workspace version 1.9.9, signed v* tags, 5-target cross-compile, maturin wheels |
| `MAINTAINED` | Bugfix-only cycles: no new features, releases gated on the full CI set |
| `EVOLVED` | New features via ROADMAP (v1.10, v2.0, v2.1): NEON row functions, encoding SIMD, PyPI publishing, HEIC/AVIF |

**Current state: POLISHED** (hardening complete, release pipeline live, working tree at HEAD aae56f4).

## Valid Transitions

### Standard path (forward, state-to-next-state)

```
IDEA -> SPEC'D -> PROTOTYPED -> IMPLEMENTED -> POLISHED -> SHIPPED -> MAINTAINED -> EVOLVED
```

### Valid deviations

| Transition | When valid |
|---|---|
| `IDEA -> SPEC'D` | Ambition ratified (STRATEGY gate) |
| `SPEC'D -> PROTOTYPED` | Validation gate COMMIT |
| `PROTOTYPED -> IMPLEMENTED` | Port COMMIT: Rust rewrite approved |
| `IMPLEMENTED -> POLISHED` | All decoders and wrappers working; hardening begins |
| `POLISHED -> SHIPPED` | All release gates pass (pr-checks + ci-full + release.yml) |
| `SHIPPED -> MAINTAINED` | Bugfix release cycle |
| `SHIPPED -> EVOLVED` | New feature work (ROADMAP item) |
| `MAINTAINED -> EVOLVED` | Feature work resumes after a maintenance period |
| `EVOLVED -> POLISHED` | Feature hardening before the next release |
| `SHIPPED -> IMPLEMENTED` | Rework: a shipped feature is re-implemented (rare, requires a learning shift) |
| Any state -> any EARLIER state | Deliberate divergence restart: the user may always jump back to an earlier state and restart from there; divergence is a valid reason to restart when continuing would compound it |

## Invalid Transitions

```
IMPLEMENTED -> SHIPPED (must pass through POLISHED: no release without hardening gates)
PROTOTYPED -> SHIPPED (skipping implementation)
IDEA -> IMPLEMENTED (skipping spec and prototype)
SHIPPED -> IDEA (no full restart without an explicit decision and a learning shift)
Any transition that violates an invariant below
```

> These are invariants: **no release ships without passing POLISHED. No feature lands without passing through the CI gate set. No state change happens silently: every transition is logged.**

## Invariants (What Must Never Change)

1. **The 8 MB file size guard never shrinks below the largest known frame (810 KB).** It may only grow. (ADR-0005)
2. **The C ABI surface never breaks:** `ithmb_decode` and `ithmb_prefix_to_profile` keep their names and signatures; symbol presence is verified by `nm` in CI.
3. **No GPL code enters the codebase.** Clean-room MIT only; the ImageGlass PR #2316 reference is behavior-informed, never copied.
4. **Inward dependencies hold:** `ithmb-core` never imports CLI, WASM, Python, or C-ABI code.
5. **`unsafe` stays confined** to `crates/ithmb-core/src/simd/*.rs` and the C-ABI boundary in `c_api.rs` (workspace lint `unsafe_code = "deny"`).
6. **Profile 1044 stays disabled.** Writing it corrupts iPod cover art (DISABLED_PREFIXES in profile_db.rs).
7. **Decoded output length always equals width x height x 4** (BGRA8). No decoder returns a partial frame.
8. **Every release passes the full CI gate set:** fmt, clippy `-D warnings`, deny, audit, doc `-D warnings`, gitleaks, typos, lychee, benchmark regression (25% threshold), fuzz, C-ABI symbol check, WASM build.
9. **No new runtime dependency without a Y-Statement** recorded in SPECIFICATION.md section 2.

## Blast Radius Map (coupled components that co-change)

| Change | Co-changes (must be checked together) |
|---|---|
| Profile schema (`profile.rs`) | `profile_db.rs`, `profile_parser.rs`, `pipeline/`, `device_profiles.rs`, CLI `--list-profiles`, `docs/PROFILES.md`, profile tests |
| `DecodeConfig` / `TransformConfig` | All `decode_*` entry points, `pipeline/`, CLI, WASM, Python, C ABI, config tests |
| Format prefix or encoding enum | `photodb/parser.rs`, the format decoders, `profile_db.rs`, `docs/FORMAT.md`, roundtrip tests |
| SIMD kernels (`simd/`) | `yuv.rs`, all YUV decoders (uyvy, ycbcr420, cl, clcl), Miri tests, benchmark regression baseline |
| `DecodeError` enum | Every module, CLI error printing, Python exceptions, WASM error strings, error tests |
| Dependency change (Cargo.toml) | `deny.toml`, `Cargo.lock`, CI tool pins (`scripts/check-ci-pins.sh`), ADR in `docs/adr/` |
| CI workflow change | `pr-checks.yml`, `ci-full.yml`, `release.yml`, `scripts/check-ci-pins.sh`, `tools/check-benchmark-regression.sh` |
| PhotoDB chunk layout | `photodb/parser.rs`, `photodb/types.rs`, `photodb/writer`, integrity checker, PhotoDB tests |
| Encoder change (`enc/`) | `ithmb-gen`, roundtrip tests, synthetic vector fixtures |

## Adding or Removing a State (Transition Update Procedure)

When a state is added, removed, renamed, or reordered:

1. Update this file's **States** table
2. Update this file's **Valid Transitions**: add or remove the state's arrows
3. Check the **Invariants**: does the change violate any?
4. Update the state machine diagram in **SPECIFICATION.md** section 2 (PROJECT_MODEL subsection)
5. Grep all protocol files for stale references to the old state (`grep -rn "OLDSTATE" *.md`)
6. If this is a NEW state: does it need a transition-table test (a checklist asserting valid entry/exit conditions)?

> The failure mode this prevents: adding a state without updating the transition table silently breaks the invariants and produces an orphaned-reference cascade across the governance documents.

## Test

The transition-table test for this project:

- [ ] State machine diagram in SPECIFICATION.md section 2 matches the States table here
- [ ] Every governance document references states that exist in this table
- [ ] No orphaned references to removed/renamed states anywhere in `*.md`
- [ ] Invariants hold: no path skips POLISHED before SHIPPED
- [ ] Every transition in this run is logged in `docs/shift-log.md`; an unlogged transition is an invariant violation