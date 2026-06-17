# Flowchart ELK Layout - Handoff

Status: Active
Last updated: 2026-06-17

## Current State

Flowchart ELK now defaults to the source-backed Mermaid ELK adapter / Eclipse ELK layered port in
public render paths and xtask diagnostics. The dedicated probe gate covers every exact upstream
`flowchart-elk.spec.js` render call, and those source-backed probes are admitted to the default
Flowchart SVG parity matrix. The remaining work is future hardening, not whether the current spec
body set needs a separate heuristic implementation.

## Decision So Far

- Keep source-backed as the default Flowchart ELK backend; preserve `compat` only as an explicit
  alpha fallback.
- Admit the current Flowchart ELK probes to broad SVG parity only through the source-backed backend;
  explicit `compat` runs stay outside parity admission.
- Treat duplicate-body exact-call fixtures as traceability, not additional unique layout semantics.
- Port future missing semantics from Mermaid / Eclipse ELK source rather than fitting fixture
  geometry.

## Next Recommended Action

Run `cargo run -p xtask -- check-flowchart-elk-source-backed-probes` and
`cargo run -p xtask -- audit-flowchart-elk-source-backed-coverage`, then use broad Flowchart SVG
compare failures as the signal for the next source-port gap.
