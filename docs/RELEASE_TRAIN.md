# Release Train — Cross-Repo Release Standard (canonical)

**Applies to:** Ithmb-Codec, Imageglass-Ithmb-Plugin, Ithmb-Codec-Web.
**Companion:** `docs/standards/RELEASE_WORKFLOW.md` (the dev/public ritual).
Written from the 1.9.10 / 1.1.4 / 1.4.18 train (2026-09-10), which exposed every
rule below the hard way. Plugin and Web point here; if the train changes, change
THIS file.

## 1. Dependency graph (verified, not assumed)

- **Plugin → Codec** via crates.io: `ithmb-core = "1.9"` floats to the latest
  1.9.x on rebuild. Lockfile update + rebuild verification required each train.
- **Web → Codec** via WASM: wasm-pack from core source + hand-adapted glue
  (`ithmb_wasm.js` custom loader, `ithmb_wasm_bg.js` reformatted — never
  overwrite with stock output), guarded by `scripts/check-wasm-drift.sh`.
- Release order is dependency order. Each step consumes the previous step's
  finished, published artifact — never a dev snapshot.

## 2. Version policy

- Repos version **independently** (no lockstep). Patch for fixes/hardening,
  minor for features, major for breaking — SemVer, no exceptions.
- Every downstream changelog **names the exact upstream version built against**
  ("Update ithmb-core to 1.9.10", "WASM rebuilt from public tag v1.9.10").
  That cross-reference is the mechanism that keeps repos updated within themselves.
- `ithmb-cli` versions independently (1.9.5 while core is 1.9.10). Bump/republish
  only when the CLI itself changes.

## 3. The train (ordered, gated)

- **R1 Codec**: bump + changelog → `local-ci.sh` → ship public → signed tag →
  public CI green → `cargo publish` (explicit confirmation; irreversible) →
  verify on index. Gate for R2/R3: index shows the version.
- **R2 Plugin**: float core → rebuild + full tests → bump + changelog (names core)
  → **version-triple check** (Cargo.toml == igplugin.json == version baked into
  a freshly compiled `.so`; compile strictly AFTER the bump) → ship public
  (direct fast-forward; branch protection enforces it) → tag → GitHub Release
  (**draft-first**, 3 CI zips + `ithmb-test-fixtures.zip` + how-to-test notes) →
  human Windows QA on the draft → publish.
- **R3 Web**: rebuild WASM from the public core tag (copy ONLY `.wasm`, drift
  gate) → bump + changelog (names core + WASM provenance) → full local gates →
  squash to public → public CI green → verify live (site 200 + WASM sha256
  matches the release build).
- **Tag policy**: signed `vX.Y.Z` on Codec + Plugin public. Web ships
  **untagged by policy** (not published to any registry); package.json +
  changelog + WASM provenance is its version record. No retro-tagging.

## 4. Reference checklist (audited 2026-09-10; automate per §7)

- Codec: workspace Cargo.toml, Cargo.lock, CHANGELOG, `crates/ithmb-core/README.md`
  (the crates.io front page — usage example must name the release), cli README,
  root README, RULES.md / SPECIFICATION.md / PROJECT_MODEL.md version refs.
  `pymod/pyproject.toml` carries NO version (maturin derives from Cargo.toml).
- Plugin: Cargo.toml, Cargo.lock, CHANGELOG, igplugin.json, AGENTS.md, SPEC,
  compiled `.so` (triple-check), 3 zips + fixtures, Release body.
- Web: package.json, CHANGELOG (+WASM provenance line), `.wasm` binary.
  HTML `?v=` stamps only move if stamped assets change (wasm currently unstamped).

## 5. Irreversible steps (explicit confirmation each)

`cargo publish` (no unpublish, only yank). Signed tags pushed to public.
Plugin Release publish (d2phap starts testing from it). Everything else can be
amended/rewound; these three cannot.

## 6. Draft-first + honest notes (learned 2026-09-10)

- Manual releases publish as **drafts**; QA passes before publish. CI-created
  releases are draft by workflow construction.
- Release notes contain the **current version's section only** — never the whole
  changelog (bug, fixed in Plugin `ci.yml` + this doc).
- No aspirational claims. Every label must describe a performed step: no
  "Enterprise" without an enterprise product, no `pip install` without PyPI
  (wheels are manual-install artifacts until then). Past release bodies were
  corrected (Codec v1.9.5/6/9/10); keep them honest going forward.
- Public corrections fold back into the shipped version (amend + force-update +
  re-tag) while nothing has escaped; never spawn patch releases for reverts.

## 7. Local artifact dry-run (before every tag move)

- Wheels: `maturin build` in pymod, assert `ithmb_python-<version>-…whl`.
- Plugin: `package.sh` per available platform; assert manifest version +
  executable; assert `.so` bakes the version (`strings`).
- WASM: record `sha256sum`; it must match the live file at verify time.
- Future work: `scripts/check-release.sh` — executable version of §4, failing the
  release on any stale reference. Spec'd, not yet built.

## 8. Fixtures rule (learned 2026-09-10)

- Release fixtures are **single-frame per-format files from `ithmb-gen`**
  (recommended dims, deterministic), named `ithmb-test-fixtures.zip` —
  the established name (v1.1.0/v1.1.1 set it; v1.1.3 dropped it; restored).
- **Never F-prefix multi-frame goldens** — decoder test material, not
  plugin-openable. Proper = valid headers + spec dims + non-flat deterministic
  content + decode-verified. Seeded high-entropy fixtures are a future hardening
  (current set is smooth gradients).
- Open question (follow-up audit queued): F-prefix files as codec goldens —
  suspected historical wrongness, never investigated.

## 9. Epilogue (queued, not started)

Org migration + lowercase rename as one hop (redirects cover reads): transfer
repos, update remotes/Pages/docs-links/registry metadata, message d2phap to
update his references. After the train, as its own job.
