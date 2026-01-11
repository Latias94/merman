# ADR-0005: Configuration Strategy

## Status

Accepted

## Context

Mermaid's behavior depends on configuration defaults (e.g. detector branching based on
`flowchart.defaultRenderer`). A purely dynamic config map risks drifting from Mermaid defaults.
However, fully hand-maintaining a large config schema in Rust is expensive.

## Decision

- Use a layered configuration approach:
  - A typed layer for fields that affect parsing/detection behavior and compatibility.
  - A raw map layer to preserve unknown/forward-compatible fields.
- The baseline source of truth for defaults is the upstream config schema
  (`packages/mermaid/src/schemas/config.schema.yaml` at the pinned baseline tag).
- `Engine::default()` loads a generated defaults artifact derived from the pinned upstream schema
  (see ADR-0019), and then deep-merges user overrides on top.
- `Engine::with_site_config(...)` deep-merges user overrides onto the engine's default config
  to avoid accidentally dropping Mermaid defaults that affect detection (e.g.
  `class.defaultRenderer`, `flowchart.defaultRenderer`).
- Do not bake runtime-specific config behavior into `merman-core`.

## Consequences

- Defaults remain aligned with Mermaid.
- We can incrementally “type” more config fields as needed without breaking consumers.
