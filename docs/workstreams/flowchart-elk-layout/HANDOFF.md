# Flowchart ELK Layout - Handoff

Status: Active
Last updated: 2026-06-15

## Current State

Flowchart ELK is no longer a binary "supported / unsupported" question. The codebase already has a
lightweight renderable subset, and the remaining question is how far the upstream `flowchart-elk`
fixture surface should go before a full ELK port becomes worthwhile.

## Decision So Far

- Preserve the default compat renderer path while using the source-backed backend for ELK parity
  convergence.
- Keep source-backed probe admission explicit; do not add Flowchart ELK fixtures to default SVG
  parity until they pass a dedicated lane and default policy is intentionally changed.
- Use nested subgraph, direction, and ordering-heavy fixtures to decide which remaining semantics
  need more Eclipse ELK source-port work.
- Only widen admission when the fixture has source-backed evidence rather than geometry fitting.

## Next Recommended Action

Run `cargo run -p xtask -- check-flowchart-elk-source-backed-probes` as the fixed A0.5 source-backed
gate, then import/probe the first Tier A batch from
`https://github.com/mermaid-js/mermaid/blob/develop/cypress/integration/rendering/flowchart/flowchart-elk.spec.js`.
