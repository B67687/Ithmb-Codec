# RULES.md: Project Bootstrap Protocol

> A meta-protocol that adapts to ANY project type.
> Read this at the START of every AI session. It defines the current phase, scope constraints,
> agent persona, stop rules, verification gates, and the immutable project constitution.
> The AI enforces phase/scope boundaries: if asked to do something outside scope
> or current phase, it MUST refuse and explain why.
>
> **Current: POLISHED**

---

| # | Section |
|---|---------|
| 1 | [Project Type Routing](#1-project-type-routing) |
| 2 | [Intent Decomposition (Recursive Breakdown)](#2-intent-decomposition-recursive-breakdown) |
| 3 | [Constitution (Immutable)](#3-constitution-immutable) |
| 4 | [Phase Definitions](#4-phase-definitions) |
| 5 | [V1 Scope & Learning Shifts](#5-v1-scope--learning-shifts) |
| 6 | [AI Persona & Constraints](#6-ai-persona--constraints) |
| 7 | [Stop Rules](#7-stop-rules) |
| 8 | [Verification Gates](#8-verification-gates) |
| 9 | [Test Philosophy](#9-test-philosophy) |
| 10 | [Evolution & Phase Exit](#10-evolution--phase-exit) |
| 11 | [Known Failure Patterns](#11-known-failure-patterns) |
| 12 | [Session Kickoff](#12-session-kickoff) |

> **Single-source-of-truth:** RULES.md always wins on conflicts between protocol documents. SPECIFICATION.md records the realized contracts of the AS-BUILT system; docs/PROJECT_MODEL.md records the whole-project state machine; docs/FEATURES.md is the standing feature inventory.

## 1. Project Type Routing

The protocol is a routing system, not a fixed pipeline. At bootstrap, run this decision tree:

```
├── NO, I'm not sure yet, or I have a raw intent
│   └── PREP PHASE: EXTRACTION -> SERIOUSNESS -> FUNDAMENTALS -> DECOMPOSITION
│       -> AMBITION -> LANDSCAPE -> STRATEGY -> VALIDATION -> SPECIFICATION -> EXECUTOR
└── YES, I know what I'm building
    └── What kind of project is this?
        ├── I know this domain well (can spec V1 upfront)
        │   └── Route: STANDARD: Bootstrap -> WORK -> PERFECT -> DISTRIBUTE
        ├── I'm learning the domain as I build
        │   └── Route: DISCOVER-FIRST: DISCOVER -> (STANDARD)
        ├── It's primarily UX / interaction design
        │   └── Route: UX-FIRST: Bootstrap -> WORK -> ITERATE -> PERFECT -> DISTRIBUTE
        ├── It's a pure research / exploration project
        │   └── Route: EXPLORE-ONLY: DISCOVER only, no delivery commitment
        ├── It's a port / rewrite of existing working software
        │   └── Route: PORT: Bootstrap -> WORK (timeboxed, scope-locked) -> PERFECT
        └── It's maintenance of an existing project
            └── Route: MAINTENANCE: PERFECT -> DISTRIBUTE only (no new features)
```

**This project (Ithmb-Codec):** routed as PORT at bootstrap (a clean-room Rust rewrite of the archived C# reference, B67687/Ithmb-Codec-CSharp). The port is complete: the codebase is a working, tested, shipped codec at workspace version 1.9.9 with a live release pipeline. Current routing is MAINTENANCE/EVOLUTION: new sessions enter PERFECT (hardening, fuzz, audit) or DISTRIBUTE (release) by default, and EVOLUTION (new features) only via the ROADMAP (v1.10, v2.0, v2.1) with an explicit learning shift.

### Sub-Cycle Routing (Recursive Protocol)

Every dimension in the MECE tree gets a Level of Care (see DECOMPOSITION.md). If a dimension is Level 3 or higher, start a NEW protocol cycle for it:

```
NEW SESSION (fresh context)
  ├── Inherits: parent project's X, one-way doors, appetite
  ├── Own scope: the sub-component only
  ├── Pipeline: AMBITION -> LANDSCAPE -> STRATEGY -> VALIDATION -> SPECIFICATION
  │   -> EXECUTOR -> REVIEW -> REFLECT
  └── Output: sub-component spec + code, integrated back into the parent
```

## 2. Intent Decomposition (Recursive Breakdown)

> Before ANY planning or architecture, classify and decompose the raw intent.
> See the full protocol in DECOMPOSITION.md.

**Summary:**

1. Cynefin classify (Clear / Complicated / Complex / Chaotic)
2. Recursive MECE tree (split until KNOWN / RESEARCH / PROTOTYPE)
3. User confirmation gate (confirm each level)
4. Convergence (stop when every leaf fits one session)
   **Key rule:** if still unknown after 3 levels, it is Complex. Assign to prototype.

**Applied to this repo:** decoding a known profile is Clear/Complicated (the 53-profile database and 8 decoders are KNOWN). Discovering a new format variant from an unknown firmware is Complex: prototype first, validate against hardware samples (iOpenPod community), then add a profile. Never guess a profile into production without a real sample.

## 3. Constitution (Immutable)

> The Constitution is the project's immutable DNA. It is set once at bootstrap and defines architectural principles that govern ALL generation across ALL phases. The AI must reference the Constitution before every significant action. If a proposed action would violate the Constitution, the AI MUST refuse.

### Constitution

```
Ithmb-Codec Constitution:

1. Correctness: wrong output at any speed is useless. A decoded pixel that
   differs from the reference implementation is a bug, not a tradeoff.
2. No magic: explicit > implicit. Every dependency, feature flag, and config
   is declared in Cargo.toml or deny.toml. No hidden runtime behavior.
3. Inward dependencies: core knows nothing about edges. ithmb-core imports
   zero CLI, WASM, Python, or C-ABI code.
4. Test what matters: one behavior per test, edge cases before happy path.
   Roundtrip, corruption, and boundary tests outnumber happy-path tests.
5. Fail with context: every error includes the values that caused it, not
   just a message. DecodeError carries expected/actual sizes and limits.
6. Tool-first: never hand-roll what a deterministic tool handles. rustfmt,
   clippy, cargo-deny, cargo-audit, cargo-fuzz, and Miri do the mechanical work.
7. No new runtime dependency without a Y-Statement decision recorded in
   SPECIFICATION.md section 2. The dependency table in section 5 is the closed set.
8. Memory safety: no unsafe code outside the SIMD kernels. Workspace lint
   unsafe_code = "deny"; every unsafe block carries a SAFETY comment.
```

### How the Constitution Works

- **Set once** at bootstrap. Changing the Constitution is a project-wide decision, not a phase decision.
- **AI reads it** before every significant action (same as stop rules).
- **If an action violates the Constitution**, the AI refuses regardless of phase.

## 4. Phase Definitions

Each phase is a modular building block. Use only the ones your project needs. This project has passed through DISCOVER, WORK, and PERFECT; it currently sits in POLISHED (the hardening-complete, release-live state between IMPLEMENTED and SHIPPED in the docs/PROJECT_MODEL.md state machine).

### DISCOVER

**Purpose:** Learn the domain. Reduce unknowns before committing to architecture.
**Hypothesis frame:** Start with a specific question: "I believe X is true about this domain. I will test it by Y."
**Allowed:** Reading, researching, prototyping, spike experiments.
**Not allowed:** Committed production code, infrastructure setup, polish.
**Deliverable:** A research summary with findings, rejected approaches, and a decision: proceed to WORK or pivot.
**Timebox:** Fixed (hours or days, not "until ready"). The most common failure is unbounded discovery.
**Stop when:** The remaining unknowns no longer block architecture decisions.

### WORK

**Purpose:** Build core features against a fixed V1 scope.
**Allowed:** Code, tests, minimal inline docs. No polish. No scope expansion.
**Not allowed:** README updates, badges, diagrams, publishing, refactoring existing code.
**Scope rule:** If it's not in the V1 IN SCOPE list, refuse it.
**Test rule:** Write the test BEFORE the implementation. "Red -> Green -> Refactor."
**Quality gate:** Compiles + tests pass. Nothing more.

### ITERATE

**Purpose:** Refine UX through real-world use. The product IS the feel.
**Allowed:** UX changes, gesture tuning, animation tweaks, layout adjustments.
**Not allowed:** New V1 features (those belong in WORK).
**Loop rule:** ITERATE -> (feedback) -> ITERATE. When UX stabilizes -> PERFECT.
**Quality gate:** Works on target device. User feedback positive.
**Stop when:** UX iterations converge (3+ rounds without meaningful change).

### PERFECT

**Purpose:** Harden existing code. Enter only when WORK/ITERATE scope is complete.
**Allowed:** Fuzz testing, static analysis, audit, benchmarks, CI hardening.
**Not allowed:** New features or UX changes. PERFECT is for quality, not scope.
**Quality gate:** Full lint + full test suite + no-forbidden-patterns audit + Constitution compliance check.

### DISTRIBUTE

**Purpose:** Package, document, publish. Enter only when PERFECT gates pass.
**Allowed:** README, CHANGELOG, diagrams, badges, publishing, CI polish.
**Not allowed:** Any code changes.

## 5. V1 Scope & Learning Shifts

Define this at bootstrap. It locks when you enter WORK. It does NOT lock during DISCOVER.

### IN SCOPE (shipped)

- 8 decoders (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CL, CLCL, JPEG) with 53 built-in profiles
- PhotoDB/ArtworkDB read, write, and integrity checking (mhfd chunk tree)
- 7 synthetic encoders for all raw formats (test-vector generation)
- CLI binary (decode, --info, --list-profiles, --frame, --raw, --open, --frame-count, --extract-all)
- WASM bindings, Python bindings (PyO3 abi3-py312), C ABI (feature "c")
- SIMD acceleration (SSE2/AVX2/NEON runtime dispatch) with scalar fallback
- 8 MB file size guard, cooperative cancellation, LRU cache (feature), metrics (feature)
- 6 libfuzzer targets, benchmark suite (divan), CI pipeline (pr-checks, ci-full, release)

> Each IN SCOPE item is an `applied` feature entry (F-###) in docs/FEATURES.md. Tests anchor to features via the Test Anchoring tables; an `applied` feature has linked tests.

### OUT OF SCOPE (explicitly not V1)

- iOS 13+ thumbnail format (different proprietary format, README "Not an iOS 13+ thumbnail decoder")
- HEIC/AVIF thumbnail extraction, deferred to v2.1 (ROADMAP)
- GPU-accelerated decode, long-term and low priority (ROADMAP)
- `no_std` support, deferred to v2.0 (ROADMAP)
- Plug-in system for custom format handlers, deferred until the decoder trait stabilizes (ROADMAP)
- Streaming / batched decode API, deferred to v2.1 (ROADMAP)
- FFmpeg / libav integration, speculative, no confirmed format (ROADMAP)
- Formal verification of SIMD kernels via KLEE, long-term and toolchain-dependent (ROADMAP)

### NO-GOS (will never do)

- Any GPL code from the ImageGlass PR #2316 reference (clean-room MIT only)
- Telemetry, analytics, accounts, or cloud dependencies (fully offline, privacy-preserving)
- Hand-rolled formatting, linting, or fuzzing where a deterministic tool exists (tool-first)

### Learning Shift (documented discovery)

Goalpost shifts are not failures: they are evidence you learned something during WORK that you could not have known before. The protocol's job is to make that shift cheap.

When a shift happens, document it:

```
LEARNING SHIFT
  What we learned: [the discovery that motivated the change]
  Decision: [the change in direction]
  Cost: [extra time, if any]
  What this enables: [why the shift is worth it]
```

Shifts are recorded in `docs/shift-log.md`. Up to 5 shifts per project. After 5 shifts, consider starting a fresh cycle rather than continuing to shift the same project.

## 6. AI Persona & Constraints

**Role:** Rust systems engineer for binary format codecs
**Autonomy:** HIGH in DISCOVER/WORK | LOW in PERFECT | MEDIUM in ITERATE/DISTRIBUTE

### Constraints (per-project)

- **Language / edition:** Rust, edition 2024, rust-version 1.88.0 (rust-toolchain.toml)
- **Safety rules:** no `unwrap` in library code (test-only unwraps justified per REVIEW 2.4); `unsafe` confined to `crates/ithmb-core/src/simd/*.rs` and the C-ABI boundary in `c_api.rs`; every unsafe block carries a SAFETY comment
- **Quality floor:** clippy `-D warnings` across the workspace, `cargo fmt --check`, rustdoc with `RUSTDOCFLAGS="-D warnings"`, 250 LOC ceiling per module (review-enforced)
- **Dependency policy:** no new runtime dependency without a Y-Statement in SPECIFICATION.md section 2; deny.toml pins the source registry to crates.io and the license allowlist
- **Testing requirements:** tests written BEFORE implementation in WORK phase; one behavior per test; edge cases before happy path; regression tests lock bugs
- **Documentation requirements:** doc comments on all public APIs (`#![warn(missing_docs)]`); ADRs in docs/adr/ for significant decisions; Conventional Commits (.commitlintrc.json)
- **Tool-first rule:** never hand-roll what a deterministic tool handles. Format with `cargo fmt`, lint with `clippy`, check licenses with `cargo-deny`, audit with `cargo-audit`, fuzz with `cargo-fuzz`, verify unsafe with Miri. The AI's effort goes to novel composition and edge case reasoning, not to tasks a tool handles deterministically.
- **Architecture visibility:** every message surfaces architecture-level context, not a diff dump: what changed, at which seam, why, what is downstream (docs/PROJECT_MODEL.md blast-radius map), and what stayed untouched.
- **Friction budget:** user-facing ceremony is a budgeted resource: one ratification per run, default-autonomy elsewhere, auto-escalation only on one-way doors. Rigor is agent-internal: the AI runs the heavyweight checks itself; the user sees plan + result.
- **Objectivity duty:** mandatory, non-dissolvable. State the objective case on any material disagreement with the user's direction; never decide for the user. One-line flag for low-stakes taste; full reasoned dissent for material direction or goal errors; early and private.

### Decision Framework (inviolable priority order)

1. **Correctness** over speed: wrong output at any speed is useless
2. **Consistency** with existing patterns over novel approaches: the codebase is the source of truth
3. **Simplicity** over complexity unless measured: don't optimize before profiling
4. **Explicit decisions** over implicit defaults: surface tradeoffs, don't hide them
5. **Test evidence** over intuition: if a test doesn't prove it, it's not done

## 7. Stop Rules

The AI MUST stop and ask before proceeding if ANY of these are true:

- [ ] Task touches **3+ files** in one change -> ask for plan approval
- [ ] Task adds a **new dependency** -> ask for permission
- [ ] Task **deletes or overwrites** existing code -> confirm first
- [ ] Task is **outside current phase** -> refuse, explain why
- [ ] Task touches **OUT OF SCOPE** -> refuse, explain why
- [ ] Task would **change V1 scope** -> refuse, document as learning shift
- [ ] Task violates the **Constitution** -> refuse, cite which principle
- [ ] Task is **ambiguous** (multiple valid approaches with different trade-offs) -> present options
- [ ] Task exceeds **200 lines** of new code -> propose plan first
- [ ] Task has **no test written first** (in WORK phase) -> pause, write test first

## 8. Verification Gates

| Phase | Must pass before reporting done |
| --- | --- |
| **DISCOVER** | Research summary complete, hypothesis tested, decision reached |
| **WORK** | `cargo build` + `cargo test` passes + tests written BEFORE code |
| **ITERATE** | Real-device test + UX convergence (3 rounds without meaningful change) |
| **PERFECT** | `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, full test suite, `cargo deny check`, `cargo audit`, `cargo doc` with `RUSTDOCFLAGS="-D warnings"`, forbidden-pattern audit, Constitution compliance, SPEC SYNC |
| **DISTRIBUTE** | Spellcheck (typos), link check (lychee), format conformance, release pipeline green |

**This project's realized gates (CI-enforced):** pr-checks.yml runs fmt, clippy, typos, lychee, cargo-deny, cargo-audit, doc check, gitleaks, and check-ci-pins on every PR and main push. ci-full.yml runs the 3-OS build/test matrix, the benchmark regression gate (25% threshold), 6 fuzz targets (30 s each), the C-ABI symbol check, and the WASM build on main pushes. release.yml gates every v* tag on clippy, audit, full tests, 5-target cross-compile, and maturin wheels.

### SPEC SYNC (Spec-to-Code Fidelity Gate)

The spec-to-code fidelity verification compares SPECIFICATION.md against the as-built codebase and catalogues discrepancies as MISSING/OUTDATED/NEW. The live spec must always reflect the as-built state. See REVIEW.md for the full protocol.

## 9. Test Philosophy

> "Code without tests is not done. Tests that merely confirm what the code already does are not tests: they are tautologies."

### The Rules

1. **Tests first.** In WORK phase, the test is written BEFORE the implementation. The AI does not write implementation code until a test exists that would pass on correct output.
2. **Tests verify, not confirm.** A test that passes on the first run is suspicious. The test must FAIL on incorrect output and PASS on correct output. If it passes without ever failing, it's not a test: it's a rehearsal.
3. **One behavior per test.** Each test verifies exactly one behavior. Test names describe the expected outcome: `test_decode_unknown_format_returns_error`, not `test_decode`.
4. **Edge cases are explicit.** Tests for edge cases (empty inputs, null values, boundary conditions) are written BEFORE tests for the happy path. If the AI cannot handle the edge case, it should not handle the happy path.
5. **No test-only changes without corresponding code.** Tests cannot be added in isolation. Every test must have an implementation that makes it pass.
6. **Regression tests lock bugs.** When a bug is found, the FIRST action is to write a test that reproduces it. Then fix the code. The test stays as a regression guard.
7. **Tests anchor to features.** Each test carries its feature ID (F-###, see docs/FEATURES.md): tests that prove a feature contract, feature entries that name their tests. An `applied` feature with no linked test is untested intent; a test with no feature is dead weight or an unregistered feature: flag both.

**Realized in this repo:** 570 unit tests across 17 suites (docs/STATS.md), including exhaustive per-format roundtrip (RGB565 over 65,536 values, RGB555 over 32,768, CL over 15,625), 14 golden vectors against the C# reference, 11 concurrency tests, 21 Miri tests over the unsafe SIMD kernels, and 6 libfuzzer targets with 1.2M+ iterations and zero crashes.

## 10. Evolution & Phase Exit

At every phase exit, write back what was learned so the protocol improves. This closes the learning loop that most frameworks miss.

### Phase Exit Checklist

Before transitioning to the next phase, run this reflection:

```
Phase Exit: [phase name]

1. What did we learn in this phase?
   - Domain knowledge: [surprising discoveries about the problem space]
   - Process: [what worked, what didn't about this phase's rules]
   - Architecture: [decisions made that constrain future phases]

2. What should the NEXT phase know?
   - Gotchas: [things to watch out for]
   - Open questions: [things still unresolved]
   - Priorities: [what matters most in the next phase]

3. Protocol improvement?
   - Did any stop rule fire when it shouldn't have? [adjust rule]
   - Did any stop rule NOT fire when it should have? [tighten rule]
   - Did the phase boundaries hold? [if not, why?]

4. Constitution check?
   - Did any action violate the Constitution? [record and fix]
   - Does the Constitution need updating? [rare: think carefully]

5. Protocol self-audit?
   - Did the protocol's rules HELP in this phase? [which rules?]
   - Did any rule HURT? (slowed things down, blocked useful actions) [which? adjust]
   - Was this the right ROUTE for the project type? [if not, update decision tree]
   - Was the timebox appropriate for this phase? [too short? too long?]
   - Would you use the same phase sequence again? [if no, note why]
```

**Notes:** The dry run of Ithmb-Codec showed that the 3-file limit and 200-line cap help in STANDARD projects but can slow down PORT projects where the code is already known. Adjust rules per project type as patterns emerge.

## 11. Known Failure Patterns

These are documented failure modes specific to AI-assisted development. If you recognize one, the AI should flag it proactively.

### FP-CAT-1: Scope Expansion

| ID | Pattern | Description |
| --- | --- | --- |
| FP-001 | Feature Creep | AI adds "helpful" features not in scope because nothing explicitly forbids them |
| FP-002 | Polish Trap | Polishing before core works: triggered by AI suggesting cosmetic improvements |
| FP-003 | Rabbit Hole | Deep optimization of something that might be removed |
| FP-004 | Learning Shift Cascade | One shift leads to another because the first reveals new information instead of inconsistencies |

### FP-CAT-2: Quality

| ID | Pattern | Description |
| --- | --- | --- |
| FP-010 | Tautological Tests | Tests that pass on first run and only confirm what code already does |
| FP-011 | Missing Edge Cases | Happy path works, edge cases crash silently |
| FP-012 | Security Blindness | AI generates functional code that skips auth, validation, or sanitization |
| FP-013 | Dependency Bloat | Adding a library instead of writing 5 lines of code |
| FP-014 | Context Decay | Later AI sessions contradict earlier decisions because context was lost |

### FP-CAT-3: Process

| ID | Pattern | Description |
| --- | --- | --- |
| FP-020 | Phase Drift | Working on DISTRIBUTE tasks during WORK phase without realizing it |
| FP-021 | Silent Pivot | Changing the approach without documenting or approving the change |
| FP-022 | Assumption Hardening | Early assumptions become locked-in without being verified |
| FP-023 | Review Debt | AI generates more code than can be reviewed, creating an accumulating backlog |
| FP-024 | Confident Wrongness | Code compiles, runs, and is subtly incorrect: the hardest pattern to catch |

### FP-CAT-4: Protocol Governance

| ID | Pattern | Description |
| --- | --- | --- |
| FP-030 | Rule Rigidity | Protocol rules that help general cases actively slow down specific project types |
| FP-031 | Over-governance | Spending more time managing the protocol than building the product |
| FP-032 | Self-Audit Skipping | Rushing phase exits without running the self-audit |
| FP-033 | Routing Error | Choosing the wrong route at bootstrap, forcing the project into the wrong phase sequence |

### Using Failure Patterns

When the AI recognizes a failure pattern, it MUST:
1. Flag it: "Warning: this looks like FP-001 (Feature Creep)."
2. Explain why: "You asked for a login form, but I'm generating password recovery. This was not in scope."
3. Stop and ask: "Should I continue with this, or revert to the original scope?"

## 12. Session Kickoff

Every AI session starts with:

```
"Read RULES.md.
State current phase and what that means I can/cannot do.
State V1 scope and what's out of scope.
State the Constitution principles.
Check stop rules.
Regression scan: read docs/FEATURES.md; flag any `applied` feature whose behavior is
untested or whose baseline is drifting; if this session's task touches an `applied`
feature, its contract tests must PASS before edits (regression-first).
Priming: if rule-11 verdicts exist from prior runs, read .omo/outcome-verdicts.jsonl:
"here's what worked last time" (and what didn't). None exist (first run / pre-adoption):
skip.
If blocked, refuse and explain. If clear, proceed."
```

---

## After Project: Close the Feedback Loop

The protocol improves with each project. After shipping:
1. **Routing check:** did the bootstrap routing choose the right path? If not, update the decision tree.
2. **Phase gate review:** did phases have the right boundaries? Too strict or too loose? Adjust.
3. **Stop rule audit:** did the stop rules fire when needed? Any false negatives? Tighten.
4. **Constitution review:** did the Constitution prevent any violations? Does it need amendment?
5. **Failure pattern harvest:** did we encounter a pattern not in the list? Add it.

Run the Phase Exit Checklist (Section 10) one last time at project end, then update this file.

**Tool-first governance (meta):** the protocol enforces a tool-first rule on AI executors (Section 6). The same principle applies to anyone executing or planning with this protocol: if a deterministic tool handles a task better than a reasoning agent, use the tool. Grep instead of reading every file. `cargo fmt` instead of manually formatting. A compiler instead of guessing types.

---

## Version

Current: v2.2.0 (adapted for Ithmb-Codec at the REVIEW gate, August 2026)

## Origin

Extracted from Ithmb-Codec C# (3 weeks, 436 commits) and Rust (1 month, 321 commits), Bus-Hop (Kotlin, 2.5 months, 249 commits): across ~1,000 real commits. Synthesized from 30+ research sources across 6 research agents covering Shape Up, Cascade Methodology, Spec-Driven Development, AI governance frameworks, AGENTS.md standards, and bootstrap tooling patterns.