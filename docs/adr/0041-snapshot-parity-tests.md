# ADR 0041: Snapshot Parity Tests (Golden Fixtures)

## Status

Accepted

## Context

`merman` aims for 1:1 parity with Mermaid `@11.12.2`. As more diagrams and edge-cases are added, the
main risk is silent behavior drift (especially across refactors).

We already have a stable headless output model (`ParsedDiagram.model`) and a CLI capable of printing
it as JSON, which makes it practical to adopt snapshot-style regression tests.

Upstream end-to-end SVG baselines are generated separately via the official Mermaid CLI (see
`docs/rendering/UPSTREAM_SVG_BASELINES.md`).

## Decision

- Store source fixtures as `.mmd` under `fixtures/` (recursive).
- Store golden snapshots next to the fixture as `*.golden.json`.
- Define the snapshot value as:
  - `diagramType`: detected diagram type string.
  - `model`: the semantic headless JSON model, with the top-level `config` field removed to keep
    snapshots smaller and avoid duplicating separately-verified generated defaults.
  - Additionally normalize known non-portable/dynamic fields:
    - `mindmap.model.diagramId` is replaced with `"<dynamic>"` (upstream uses UUID v4).
    - `gantt.model.tasks[*].{startTime,endTime,renderEndTime}` are converted from epoch millis into
      local ISO strings (`YYYY-MM-DDTHH:mm:ss.SSS`) so snapshots are stable across timezones.
- Add an integration test in `merman-core` that:
  - parses every fixture with `Engine::parse_diagram`
  - compares the snapshot JSON `Value` to the golden JSON `Value`
  - fails with a message instructing to regenerate snapshots when mismatched.
- Add an `xtask` command to regenerate all golden snapshots:
  - `cargo run -p xtask -- update-snapshots`

## Consequences

- Changes to parsing behavior become immediately visible as snapshot diffs.
- Updating golden snapshots is an explicit action and should be paired with a clear reason (e.g.
  upstream parity fix).
- Because `config` is excluded from snapshots, changes to generated defaults remain covered by
  `xtask verify-generated`.
