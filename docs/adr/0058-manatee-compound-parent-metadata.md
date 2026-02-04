# ADR-0058: Add `parent` metadata to `manatee::Node` (compound node groundwork)

Date: 2026-02-04

## Context

Mermaid `@11.12.2` Architecture uses Cytoscape-FCoSE with compound nodes (groups). Several downstream
layout behaviors depend on the inclusion tree:

- inter-graph (`isInterGraph`) edge classification
- inclusion tree depth (`getInclusionTreeDepth()`) used by `layout-base` `calcIdealEdgeLengths()`
- LCA-level endpoint selection (`getSourceInLca()` / `getTargetInLca()`)

The current `manatee` graph model is flat (nodes + edges) and does not carry compound membership
information, which makes it hard to implement higher-fidelity FCoSE parity without re-plumbing the
input API later.

## Decision

Extend `manatee::Node` with an optional `parent: Option<String>` field.

- `parent` represents the compound node id (e.g. Mermaid Architecture group id).
- When no compound semantics are needed, `parent` is `None`.
- This change is **data-only**: existing algorithms do not change behavior yet.

## Consequences

### Pros

- Enables incremental implementation of compound-aware FCoSE semantics without an API redesign.
- Allows callers (e.g. Architecture renderer) to pass Mermaid group membership explicitly.

### Cons

- Public API change: all `manatee::Node` constructors must provide `parent`.
- Without follow-up work, the field is unused.

## Follow-ups

- Introduce a compound-aware internal graph representation for FCoSE (inclusion tree + LCA queries).
- Use `parent` to classify inter-graph edges and apply `layout-base` `calcIdealEdgeLengths()` logic.
- Validate against Mermaid upstream SVG baselines (focus: `parity-root` viewBox/max-width drift).
