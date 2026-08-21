# Standards Audit Exceptions

The repository is a Rust workspace, so a few checks in the cross-repository
audit do not describe applicable project conventions. These exceptions are
documented rather than satisfied with placeholder files or fabricated assets.

## Architecture and entry point

- `architecture-template-match`: the audit's four templates are Java/Kotlin/
  Python-oriented directory layouts. Rust workspace architecture is defined by
  `Cargo.toml`, crate manifests, and the documented crate boundaries instead.
- `main-entry-point`: the executable entry point is
  `crates/ithmb-cli/src/main.rs`; the audit only recognizes root-level or
  `src/main.go`, `main.py`, `Main.java`, and `Program.cs` paths. Adding a
  duplicate root entry point would be incorrect.

## Directory naming

- `top-level-dirs-kebab`: the failing names are local tool/build directories
  such as `.venv`, `.ruff_cache`, `.playwright-mcp`, `target`, and
  `mutants.out`; they are not repository paths and are intentionally not
  renamed for an audit.
- `no-uppercase-dirs`: `.github/ISSUE_TEMPLATE` is the GitHub-required name
  for issue templates. Renaming it would disable GitHub's convention.

## Secrets management

- `sops-config-exists`, `encrypted-env-tracked`, and `gitattributes-diff`:
  this library does not deploy or store environment secrets. There is no
  legitimate encrypted environment file to commit, and adding a placeholder
  SOPS recipient or empty encrypted file would be misleading.

## Screenshots

- `screenshots-dir-exists` and `screenshot-naming`: this is a headless codec
  library and CLI. The README's existing SVG is explicitly labelled a concept
  render, not a product screenshot; no genuine UI screenshot exists to add.

The self-consistency failure is consequently a derived result of these
documented, non-applicable checks rather than an independent repository issue.
