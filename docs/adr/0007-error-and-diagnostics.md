# ADR-0007: Error and Diagnostics

## Status

Accepted

## Context

`merman` aims to be a 1:1 re-implementation of Mermaid behavior. Parse failures and diagnostics are
part of the user-visible contract, and will be relied upon by downstream tooling (CLI, editors,
CI checks, UI integrations).

At the same time, `merman` is implemented in Rust and should expose structured errors for robust
programmatic handling, while avoiding over-coupling to Mermaid's JavaScript exception types.

## Decision

- Provide structured error types in Rust (`thiserror`), with:
  - a stable error category (enum variants),
  - an error message string intended for end users,
  - optional source-location information (span/line/col) when available.
- Error message alignment policy:
  - For user-facing, high-frequency errors (e.g. malformed front-matter, unknown diagram type),
    the message must match Mermaid's message text at the pinned baseline.
  - For internal/low-frequency errors, prioritize:
    1) error category stability and payload,
    2) message semantic equivalence,
    3) then tighten to exact text if/when needed by tests.
- Do not leak JavaScript-specific error wrapper formatting (e.g. `UnknownDiagramError:` prefix) into
  the Rust API surface. Only the message body is considered part of compatibility.

## Consequences

- We can add richer diagnostics (spans) without breaking downstream consumers.
- Compatibility tests can focus on message strings for selected errors while keeping the rest
  resilient during early development.

