# Radar Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Radar parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Note: Mermaid exposes this diagram as `radar-beta` but registers it with diagram id `radar`.

Upstream references:

- Parser/AST bridge: `repo-ref/mermaid/packages/mermaid/src/diagrams/radar/parser.ts`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/radar/db.ts`
- Upstream tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/radar/radar.spec.ts`

## Supported (current)

- Header:
  - `radar-beta`
  - `radar-beta:`
  - `radar-beta :`
- Common metadata:
  - `title ...`
  - `accTitle: ...`
  - `accDescr: ...` and `accDescr{...}` (single-line)
  - Last assignment wins.
- Axes:
  - `axis A,B,C`
  - Optional axis label: `A["Axis A"]`
  - If no label is provided, label defaults to axis name.
- Curves:
  - `curve mycurve{1,2,3}`
  - Optional curve label: `mycurve["My Curve"]{...}`
  - Entries:
    - numeric entries: `1,2,3`
    - detailed entries (any order): `A: 1, B: 2, C: 3` (colon optional per upstream grammar)
  - Detailed entries are reordered to match the axis list.
  - Missing detailed entry throws:
    - `Missing entry for axis <axis label>`
- Options (Mermaid defaults, last value wins):
  - `showLegend true|false` (default `true`)
  - `ticks <number>` (default `5`)
  - `min <number>` (default `0`)
  - `max <number>` (default `null`)
  - `graticule circle|polygon` (default `circle`)

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s Radar DB behavior:
  - `type`
  - `title`, `accTitle`, `accDescr`
  - `axes`: `{ name, label }[]`
  - `curves`: `{ name, label, entries: number[] }[]`
  - `options`: `{ showLegend, ticks, min, max, graticule }`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `radar-beta` grammar and DB
behavior compatibility at the pinned baseline tag.
