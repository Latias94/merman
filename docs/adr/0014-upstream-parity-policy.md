# ADR-0014: Upstream Parity Policy (Mermaid is the Spec)

## Status

Accepted

## Context

`merman` is intended to be a 1:1 re-implementation of Mermaid (pinned to a specific upstream tag/commit).
This repo is not a “Mermaid-like” parser or a simplified subset. Any behavior differences from upstream
Mermaid are considered bugs.

At the same time, the project will be delivered incrementally (phases). During early phases, only a
subset of the full Mermaid surface area may be implemented, but the end state must converge to full
parity with the pinned upstream baseline.

## Decision

- **Mermaid upstream is the single source of truth** for:
  - grammar and parsing behavior
  - configuration semantics
  - diagram detection rules
  - error conditions and tolerated inputs
- **No intentional divergence**:
  - we do not introduce “Rust-idiomatic” semantic changes
  - we do not accept “works for our use-case” deviations
  - if upstream behavior is surprising, we still match it (and document it)
- **Incremental delivery is allowed**, but must be framed as “not implemented yet”, never as a
  different spec:
  - alignment docs track current coverage vs upstream
  - missing features are explicitly marked TODO with a pointer to upstream behavior
- **Test strategy is upstream-driven**:
  - prefer porting Mermaid’s existing tests/fixtures (e.g. `packages/mermaid/src/**.spec.*`,
    Cypress samples, demo sources) into Rust tests
  - add targeted regression tests for any discovered divergence
- **Output model aligns to Mermaid DB semantics**:
  - headless parse results should match Mermaid’s DB-like structures and invariants where feasible
  - the internal AST may evolve, but the semantic output should converge to upstream expectations

## Consequences

- Parity work is prioritized over shortcuts; initial phases may be smaller but are safer long-term.
- Any mismatch is treated as a bug and fixed with a test that demonstrates upstream behavior.
- Documentation must clearly distinguish:
  - “Supported (implemented now)” vs “Supported in Mermaid but not implemented yet”.

