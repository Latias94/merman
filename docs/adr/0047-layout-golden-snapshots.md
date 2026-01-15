# ADR 0047: Layout Golden Snapshots (Geometry Layer)

## Status

Accepted

## Context

`merman` aims for 1:1 parity with Mermaid `@11.12.2`. We already use:

- semantic golden snapshots (`*.golden.json`) to lock parsing behavior
- upstream SVG fixtures (`fixtures/upstream-svgs/**`) to lock end-to-end rendering output

However, end-to-end SVG diffs are hard to debug because deviations can originate from multiple
layers:

1. parsing (semantic model)
2. layout (node/edge geometry)
3. rendering (SVG encoding, markers, styles, text measurement approximations)

We want a stable, intermediate “geometry layer” snapshot that allows regressions to be detected and
localized earlier than SVG output, without needing to diff full SVG files.

## Decision

- Add an additional golden snapshot type: **layout snapshots** stored as `*.layout.golden.json`
  next to the source fixture `.mmd`.
- Define the layout snapshot value as:
  - `diagramType`: detected diagram type string
  - `layout`: the headless layout output (nodes/edges/clusters/points/labels/bounds), with:
    - floating-point values rounded to a small fixed precision to improve stability across minor
      numeric refactors
    - deterministic ordering inherited from layout code (nodes/edges sorted by id where
      applicable)
- Add an integration test in `merman-render` that:
  - parses every fixture with `Engine::parse_diagram`
  - runs `layout_parsed`
  - compares the computed snapshot JSON `Value` to `*.layout.golden.json` if present
  - fails with a message instructing how to regenerate layout snapshots
- Add an `xtask` command to regenerate layout snapshots:
  - `cargo run -p xtask -- update-layout-snapshots [--diagram <name>] [--filter <substr>]`
  - Alias: `cargo run -p xtask -- gen-layout-goldens ...`

## Consequences

- Layout regressions (geometry/spacing/routing) become visible even when SVG output still “looks
  plausible”.
- When a parity fix requires changing layout, updating `*.layout.golden.json` becomes an explicit,
  reviewable action, similar to semantic snapshots.
- Layout snapshots are **not** a replacement for upstream SVG parity checks; they complement them
  by narrowing the failure surface.

