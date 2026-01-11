# ADR-0012: Tiny vs Full Scope

## Status

Accepted

## Context

Mermaid has a "tiny" build that excludes some large features and optional dependencies. `merman`
should support both "full" and "tiny" without codebase forks, but the exact scope needs a clear,
testable definition.

## Decision

- Default build is "full" (matches `mermaid` package behavior).
- "tiny" is an opt-in feature set that may exclude:
  - certain diagrams (e.g. those guarded behind Mermaid's `includeLargeFeatures`),
  - optional heavy dependencies (e.g. KaTeX),
  - optional layout engines beyond the baseline default.
- The authoritative scope definition is compatibility tests plus documentation:
  - detector order and available diagrams must be documented per feature set,
  - tests must assert expected detectability and error messages for excluded features.

## Build notes

- Full (default): `cargo build -p merman-core`
- Tiny: `cargo build -p merman-core --no-default-features`

## Consequences

- We can evolve the exact tiny scope incrementally while keeping it explicit and test-driven.
- Consumers can choose the smallest build that still supports their diagram subset.
