# Security Policy

## Scope

This policy covers all repos under the B67687 organization on GitHub, including:

- **Ithmb-Codec** — Rust SIMD codec library and CLI tools. Website, WASM decoder, and telemetry at [Ithmb-Codec-Dev](https://github.com/B67687/Ithmb-Codec-Dev)
- **Imageglass-Ithmb-Plugin** — ImageGlass v10 native codec plugin
- **Oh-My-Learner** — CLI spaced repetition study tool
- **Development-Protocol** — Document-driven AI agent framework

## Reporting a Vulnerability

If you discover a security issue in any of our projects — especially the Rust codec library (memory safety) or an AI agent workflow — please report it privately:

- **Email**: enterprise@ithmb-codec.dev
- **Response**: We aim to acknowledge within 48 hours and provide a fix timeline within 5 business days.

## Security Properties of the WASM Decoder and Telemetry

The WASM decoder and telemetry collection are maintained in the [Ithmb-Codec-Dev](https://github.com/B67687/Ithmb-Codec-Dev) repository:

- **WASM decoder**: Zero-trust by design. No file content is ever transmitted. All processing is local in a browser WASM sandbox.
- **Telemetry**: Opt-in only, metadata-only (prefix, dimensions), rate-limited, deduplicated. No PII, no file content, no cookies.
- **Endpoint**: Cloudflare Worker at `ithmb-telemetry.ithmb-codec.workers.dev` (rate limit: 10 submissions/day/fingerprint)

## Known Security Properties

- **WASM decoder**: Zero-trust by design. No file content is ever transmitted. All processing is local in a browser WASM sandbox.
- **Telemetry**: Opt-in only, metadata-only (prefix, dimensions), rate-limited, deduplicated. No PII, no file content, no cookies.
- **Rust codec**: Memory-safe by Rust's guarantees. The only unsafe code is in SIMD intrinsics (audited, tested).
- **AI agents**: Execute within the constraints of the host environment. No network access beyond specified tool calls.

## Acknowledgments

We believe in coordinated disclosure. Contributors who report valid issues will be credited in our acknowledgments (with consent).
