# ADR-0006: Feature Flags (Tiny vs Full)

## Status

Accepted

## Context

Upstream Mermaid has a “tiny” build that excludes some large features/diagrams. We want to support
both without forking the codebase.

## Decision

- Introduce a feature split to distinguish “full” vs “tiny” behavior.
- Default is “full” to match `mermaid` package behavior.
- Current implementation:
  - `merman-core` default features include `large-features` (full build).
  - Build tiny via `--no-default-features` (disables `large-features`).
- “tiny” primarily affects:
  - which diagrams are registered/detectable,
  - optional dependencies (e.g. KaTeX),
  - layout engines.

## Consequences

- Detector order and diagram registry become feature-dependent and must be documented and tested.
