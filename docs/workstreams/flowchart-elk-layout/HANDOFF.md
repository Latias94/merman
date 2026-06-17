# Flowchart ELK Layout - Handoff

Status: Active
Last updated: 2026-06-17

## Current State

Flowchart ELK now defaults to the source-backed Mermaid ELK adapter / Eclipse ELK layered port in
public render paths and xtask diagnostics. The dedicated probe gate covers every unique upstream
`flowchart-elk.spec.js` layout body; the remaining question is admission policy, not whether the
current spec body set needs a separate heuristic implementation.

## Decision So Far

- Keep source-backed as the default Flowchart ELK backend; preserve `compat` only as an explicit
  alpha fallback.
- Keep probe admission explicit; do not move Flowchart ELK fixtures into the broad SVG parity
  matrix until the policy is intentionally changed.
- Treat the six missing exact-call fixtures as duplicate-body traceability gaps, not unique layout
  gaps.
- Port future missing semantics from Mermaid / Eclipse ELK source rather than fitting fixture
  geometry.

## Next Recommended Action

Run `cargo run -p xtask -- check-flowchart-elk-source-backed-probes` and
`cargo run -p xtask -- audit-flowchart-elk-source-backed-coverage`, then decide whether to import
the six duplicate exact-call fixtures or start broad Flowchart matrix admission for the admitted
source-backed probes.
