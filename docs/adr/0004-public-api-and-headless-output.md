# ADR-0004: Public API and Headless Output Model

## Status

Accepted

## Context

We want 1:1 Mermaid behavior but also a Rust-friendly API for integration. Mermaid's upstream API
returns `{ diagramType, config }` from `mermaidAPI.parse()` where `config` is the merged overrides
from front-matter and directives, not necessarily the full effective config.

## Decision

- Provide a compatibility-oriented parse result that includes:
  - `diagram_type`: detected diagram type string.
  - `config`: merged overrides extracted from front-matter and directives (directive wins).
  - `effective_config`: the effective config after applying site defaults + overrides.
  - `title`: title extracted from front-matter (if any).
- Support `suppress_errors` only on model-producing parse and render facades. Once preprocessing
  and diagram detection have succeeded, a family parse failure is projected as the Error family
  model instead of being returned as an error. Detection and preprocessing failures remain errors.
- Editor and analysis snapshots do not accept an error-suppression option. They retain the original
  family failure together with parser-backed recovery facts so diagnostics cannot be reconstructed
  from a second parse.
- Keep rendering out of `merman-core`. Rendering crates should consume a stable semantic model
  (DB-like model) rather than raw grammar AST.

## Consequences

- The “headless contract” is stable and suitable for CLI/UI/server integration.
- Parser implementations can evolve without changing the public API.
