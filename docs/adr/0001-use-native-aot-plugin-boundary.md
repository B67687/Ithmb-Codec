# ADR-0001: Native AOT Plugin Boundary

**Status**: Accepted (2026-06-08)
**Context**: ImageGlass requires plugins as Native AOT-compiled libraries exposing `ig_plugin_get_api`.

## Decision

Use .NET Native AOT (`<PublishAot>true</PublishAot>`) to produce a single-file unmanaged
shared library. The public API surface is exactly one function (`ig_plugin_get_api`)
exported via `[UnmanagedCallersOnly]`. Everything else is `internal`.

## Consequences

- **Positive**: Zero-runtime deployment. Single binary. No .NET Runtime required.
- **Positive**: Forced clean separation — plugin ABI cannot leak internals.
- **Negative**: No JIT, no reflection, no runtime code generation.
- **Negative**: SIMD dispatch must use compile-time ISA checks (`IsSupported`),
  not runtime JIT recompilation.
- **Negative**: JSON parsing must be hand-written (no `System.Text.Json` source gen
  in Native AOT for this use case — later resolved with custom `JsonParser.cs`).

## Alternatives Considered

| Approach | Why rejected |
|----------|-------------|
| ImageGlass managed plugin API | ImageGlass v3 only supports Native AOT plugins |
| Shared library in C with P/Invoke | Would lose SIMD intrinsics, memory safety, and .NET ecosystem |
