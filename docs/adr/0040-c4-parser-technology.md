# ADR 0040: C4 Parser Technology (Mermaid@11.12.2 parity)

- Status: Accepted
- Date: 2026-01-12
- Baseline: Mermaid `@11.12.2`

## Context

Mermaid’s C4 diagram is implemented using a Jison grammar plus a stateful DB:

- Detector: `packages/mermaid/src/diagrams/c4/c4Detector.ts` (`^\s*C4Context|C4Container|C4Component|C4Dynamic|C4Deployment`)
- Grammar: `packages/mermaid/src/diagrams/c4/parser/c4Diagram.jison`
  - headers: `C4Context` / `C4Container` / `C4Component` / `C4Dynamic` / `C4Deployment`
  - statement macros (subset): `Person(...)`, `System(...)`, `Container(...)`, `Boundary(...) { ... }`, `Rel(...)`, etc.
  - key/value attributes: `$sprite="..."`, `$tags="..."`, `$link="..."` are parsed into single-key objects.
- DB: `packages/mermaid/src/diagrams/c4/c4Db.js`
  - stores shapes, boundaries, relationships, and layout knobs (`c4ShapeInRow`, `c4BoundaryInRow`)
  - tracks a boundary parse stack to attach `parentBoundary` to nested elements
  - `wrap` is controlled by Mermaid’s directive preprocessing (`%%{wrap}%%`) and passed via `init`.

`merman` must provide a headless, pure-Rust parser where upstream behavior is the spec.

## Decision

Implement C4 parsing in `merman-core` as a handwritten macro parser plus a DB-like state model:

- Parse headers and then read statements line-by-line.
- Parse macro invocations of the form `Name(arg0, arg1, ...)` with:
  - empty arguments mapped to empty strings (aligning with Mermaid’s `ATTRIBUTE_EMPTY`)
  - quoted strings (`"..."`, no escape processing)
  - key/value attributes `$key="value"` mapped to `{ key: "value" }`
- Implement boundary blocks (`Boundary(...) { ... }`) by maintaining a boundary stack and setting
  `parentBoundary` on nested shapes.

## Notes (upstream quirks)

- Mermaid’s C4 grammar currently maps `accTitle: ...` into `setTitle(...)` instead of `setAccTitle(...)`.
  `merman` mirrors this behavior in the headless parser for parity.
- Mermaid’s grammar includes `direction TB|BT|LR|RL`. The upstream C4 DB does not expose a direction
  setter; `merman` accepts the statement as a no-op to avoid unnecessary parse failures.
- Mermaid’s `addDeploymentNode(...)` signature includes `sprite`, but the implementation does not
  store it. `merman` ignores `sprite` on deployment nodes for parity.

## Consequences

- The headless output is a semantic snapshot (shapes/boundaries/rels/layout/wrap/config). Rendering
  (SVG, layout routing, theme palette) is out of scope for this phase.
- If C4 grammar evolves to require exact token-stream behavior (e.g., escaping rules inside strings),
  revisit implementing a dedicated lexer.
