# Security Policy

## Scope

This policy covers repos under the B67687 organization related to the Ithmb-Codec project:

- **Ithmb-Codec** — Rust SIMD codec library, CLI tools, and documentation
- **Imageglass-Ithmb-Plugin** — ImageGlass v10 native codec plugin
- **Ithmb-Codec-Dev** — Website, WASM decoder, and developer tools

## Reporting a Vulnerability

If you discover a security issue in any of our projects — especially the Rust codec library (memory safety) — please report it privately:

- **Email**: enterprise@ithmb-codec.dev
- **Response**: We aim to acknowledge within 48 hours and provide a fix timeline within 5 business days.

## Security Properties

### WASM decoder

- Zero-trust by design. No file content is ever transmitted. All processing is local in a browser WASM sandbox.

### Rust codec

    - Memory-safe by Rust's guarantees. The only unsafe code is in SIMD intrinsics and C FFI bindings in c_api.rs (audited, tested).

## Acknowledgments

We believe in coordinated disclosure. Contributors who report valid issues will be credited in our acknowledgments (with consent).
