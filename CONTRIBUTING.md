# Contributing

Pull requests are **closed** on this repo — they cannot be opened. The contribution channel is **issues**.

A good issue contains: what you did, what you expected, what happened instead, and your
environment (OS, toolchain versions). For decode problems, attach the sample file plus the
device/app that produced it — a real sample is worth more than a long description.
Unknown-format samples are especially valuable: they grow the profile catalog.

## Orientation tour (30 minutes)

1. `README.md` — what the codec is and its current capabilities.
2. `docs/FORMAT.md` — the format bible; read before touching any decoder.
3. `ARCHITECTURE.md` — crate layout and data flow.
4. `SPECIFICATION.md` — behaviors with acceptance criteria.
5. `docs/adr/` — why past decisions were made; read before relitigating one.
6. `crates/` — `ithmb-core` (decoder), CLI, language bindings.

## Per-repo guidance

- **New/changed profiles** need fixtures plus a bench entry; update `docs/FORMAT.md`.
- **Decode changes** must keep the golden-file suite green and note any speed delta.
- **Python (`pymod/`)**: version is derived from `Cargo.toml` (maturin) — never hardcode it.
- **Releases** follow `docs/RELEASE_TRAIN.md` (codec → plugin → web, in that order).
- **Committing**: atomic, present-tense imperative subjects, signed, no tool-attribution trailers.

## Security

See `SECURITY.md` for the policy and the private reporting channel.
