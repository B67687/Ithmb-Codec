# Release Workflow — Dev/Public Dual-Repo Standard

**Canonical reference for shipping releases across the ITHMB projects.**
Applies to `Ithmb-Codec` and `Ithmb-Codec-Web`. Both repos' `AGENTS.md`,
`docs/RELEASING.md`, and `docs/FEATURES.md` (web) point here — if the workflow
changes, change THIS file, not the copies.

## The two remotes

| Remote | Repo | Visibility | Purpose | CI |
| ------ | ---- | ---------- | ------- | -- |
| `origin` | `*-Dev` (e.g. `Ithmb-Codec-Dev`, `Ithmb-Codec-Web-Dev`) | **PRIVATE** | Editing repo — ALL work happens here | **Billing-blocked** (private repos need paid minutes) |
| `public` | the public repo (e.g. `Ithmb-Codec`, `Ithmb-Codec-Web`) | **PUBLIC** | Shipped repo — the live site / published code | **FREE + green** (public repos get free minutes) |

**Public CI is the gate.** The private dev repo's Actions cannot run (paid-minute
billing block); the public repo runs the same workflows free. A red dev CI is
**cosmetic** — check the public repo's runs instead.

## The ritual (every release)

1. **All work on dev `main` first.** Commit on `origin/main`, push `origin main`.
2. **Version + changelog on dev.** Bump the version (web: `package.json`;
   Rust: workspace `Cargo.toml` — then `cargo build` once so `Cargo.lock` syncs)
   and add a Keep-a-Changelog entry in the same batch.
3. **Verify locally** before shipping: web `npm run ci` (lint gates + tests,
   `BASE_URL` local); Rust `./scripts/local-ci.sh` (the Linux-runnable superset).
4. **Build the public branch thematically.**
   - Web: checkout `squash-work` (tracks `public/main`), `git reset --hard public/main`,
     then `git cherry-pick -n <dev-commits>` into **1–3 thematic commits**
     (security / features / docs-version). Single-commit releases are a plain
     `git cherry-pick <dev-commit>` (nothing to squash).
   - Rust: `git checkout -B public-ship public/main`, `git cherry-pick -n <dev-commits>`
     (collapse split/revert pairs into one clean commit; exclude stray binaries
     like core dumps — see rule below).
5. **Tree-verify before pushing:** `git diff --quiet <dev-head> <public-branch>`
   must exit 0 (identical trees). Never push public without this check.
6. **Fast-forward public:** `git push public <branch>:main` (web uses
   `squash-work:main`; Rust uses `public-ship:main`, then delete the branch).
7. **Tag on public:** `git tag -s vX.Y.Z` + `git push public vX.Y.Z`.
   Every shipped version gets a tag — untagged versions lose traceability.
8. **Web deploys itself:** Cloudflare Pages auto-builds from the public repo's
   `main`. Verify the live site after shipping (version endpoint, core flows).

## Rules

- **Never push dev `main` to `public` directly** — always the squash/cherry-pick
  ritual above, tree-verified.
- **Never commit directly to `squash-work`** (web) — it is the public mirror;
  build it from dev commits.
- **Keep the trees in sync:** dev `main` and `public/main` must always have
  identical trees. If they diverge (e.g. a correction after shipping), the fix
  is a deliberate, user-approved history rewrite (`--force-with-lease`), not a
  new drift.
- **Never let a stray binary reach public history.** A core dump or build
  artifact swept into a dev commit must be excluded from the public branch
  (and gitignored) before shipping.
- **No dependabot on public.** Dependency bumps flow dev-first like any change
  (local checks: Rust `cargo-outdated` in `local-ci.sh`, web `npm run check:deps`).
- **Corrections to a just-shipped release** fold back into that version (amend +
  force-update + re-tag) rather than spawning a patch release for a revert.
