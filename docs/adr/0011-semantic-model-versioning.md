# ADR-0011: Semantic Model Versioning and Stability

## Status

Accepted

## Context

Downstream crates (renderers, UI integrations, CLI tooling) need a stable headless output model to
avoid frequent breaking changes. However, `merman` is a 1:1 re-implementation of Mermaid, and the
baseline compatibility target may change over time as we upgrade to new Mermaid versions.

Committing to a strong cross-version compatibility guarantee too early can slow down alignment and
increase maintenance cost.

## Decision

- The semantic model (DB-like model) is guaranteed to be stable for the pinned upstream baseline.
- We do not guarantee stability across major baseline upgrades.
- When the upstream baseline changes, semantic model changes are allowed, but must be documented in:
  - the baseline ADR, and
  - changelog/release notes (when publishing).

## Consequences

- Early development can focus on correctness against the baseline.
- Downstream consumers can pin `merman` versions to keep behavior stable.

