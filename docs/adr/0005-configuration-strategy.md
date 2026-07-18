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
  - A raw map layer for trusted site config and retained semantic config.
- The baseline source of truth is Mermaid's complete default-config construction: the pinned
  schema, `defaultConfig.ts`, and the `config.ts` clone semantics (see ADR-0019).
- `Engine::default()` loads the pure upstream JSON value projection, applies Merman's typed hardened
  site policy, and then applies theme defaults.
- `Engine::with_site_config(...)` deep-merges user overrides onto the engine's default config
  to avoid dropping Mermaid defaults that affect detection, such as `layout` and
  `flowchart.defaultRenderer`.
- Sanitize untrusted init directives against Mermaid's generated flat key shape. Unknown, null, and
  prototype-pollution keys are removed; trusted site config remains forward-compatible.
- Keep Merman security policy separate from upstream data. The upstream artifact retains Mermaid's
  six secure keys; the default Engine uses the local ten-key hardened policy.

## Consequences

- Defaults remain aligned with Mermaid.
- We can incrementally “type” more config fields as needed without breaking consumers.
- JSON value parity no longer loses legal function or `undefined` keys, because directive shape is
  generated and verified separately.
