## Description

<!-- What does this change do? -->

## Checklist

- [ ] CHANGELOG.md updated under `[Unreleased]`
- [ ] Tests added for new behavior (happy path + edge + error)
- [ ] All existing tests pass in both Debug and Release
- [ ] Static analysis passes (`cargo clippy --workspace -- -D warnings` + `cargo fmt --check`)
- [ ] Code coverage gate passes (≥72%)
- [ ] No file exceeds 250 pure LOC (or SIZE_OK comment added)
- [ ] Commit signed with correct date
- [ ] README stats verified if metrics changed
