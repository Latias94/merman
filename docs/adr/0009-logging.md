# ADR-0009: Logging

## Status

Accepted

## Context

Mermaid includes logging for debugging and diagnostics. `merman` should support similar visibility
without forcing logging behavior on library consumers.

In Rust ecosystems, structured logging via `tracing` is a common best practice, and integrates well
with many backends.

## Decision

- Use `tracing` for internal logging in core crates.
- `merman-core` must not initialize any global subscriber; that is the responsibility of the
  application or higher-level crates (e.g. CLI).
- Log events should use stable, structured fields where possible (diagram type, phase, etc.), and
  avoid logging untrusted raw input by default.

## Consequences

- Applications control log routing/format/levels.
- Diagnostics can be enabled in development without affecting production behavior.

