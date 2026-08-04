# State Diagram Admission Contract (Mermaid 11.16.0)

This document tracks the admitted `stateDiagram` parser, model, Dagre layout, and SVG behavior.

Upstream references:

- Parser: `repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/stateDiagram.jison`
- DB/model: `repo-ref/mermaid/packages/mermaid/src/diagrams/state/stateDb.ts`
- Parser tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/state-parser.spec.js`
- Style tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/state/parser/state-style.spec.js`

## Implemented

- Type detection:
  - `stateDiagram` when input starts with `stateDiagram` / `stateDiagram-v2`.
- State declarations (minimal):
  - `id`
  - `id: description` (both `id : desc` and `id:desc`)
  - `state "Description" as id`
- Relations (minimal):
  - `id1 --> id2`
  - `id1 --> id2: label`
- Composite states (groups/containers):
  - `state id { ... }`
  - `state "Description" as id { ... }`
- Start/end rewriting (Mermaid v2 semantics):
  - `[*]` in relations is translated to `root_start` / `root_end` (nested via `<parent>_start/_end`).
- Styling statements:
  - `classDef <classId> <css...>` stores styles and `textStyles` (color -> fill swap).
  - `class <ids> <classId>` applies classes to states (comma-separated ids, optional spaces).
  - Inline `id:::classId` and `[*]:::classId` in relations apply classes to referenced states.
  - `style <ids> <css...>` applies `cssStyles` to states (comma-separated ids).
- Comments:
  - `%% ...` and `# ...` are skipped (including inside composite blocks).
- Notes (minimal):
  - `note left of|right of <id> : <text>`
  - `note left of|right of <id> ... end note`
  - `note "<text>" as <id>` (floating note)
- Click/link statements:
  - `click <id> "<url>" "<tooltip>"`
  - `click <id> href "<url>"`
- Compatibility directives (parsed and ignored, matching upstream runtime behavior):
  - `hide empty description`
  - `scale <n> width`

## Output shape (current)

The parser returns a headless semantic model:

- `states`: map keyed by state id, including `descriptions`, `doc`, `classes`, `styles`.
- `relations`: array of `{ id1, id2, relationTitle }`.
- `styleClasses`: map keyed by class id (`styles`, `textStyles`).
- `nodes`/`edges`: layout-ready arrays (Mermaid `StateDB.getData()` style), including group/note nodes.
- Note sizing parity: note node `padding` follows `config.flowchart.padding` (schema default `15`).
- `config`/`direction`/`other`: `StateDB.getData()` compatible keys for downstream renderers.

## Layout And SVG Admission

- The production graph preserves Mermaid's interleaved child/parent insertion order and compound
  extraction order. `compare-dagre-layout --diagram state` compares the same serialized input in
  Dugong and pinned `dagre-d3-es`, including edge-label anchors and routed points.
- Updated-path edge labels use the shared source-backed geometry helper. Leaf self-loops retain the
  Dagre anchor, while composite self-loops move only after cluster path updates.
- `classDef` shape styles, text styles, CSSOM color serialization, HTML-label selectors, and SVG
  `tspan` selectors are emitted separately like Mermaid.
- `stress_state_batch5_parallel_edges_labels_styles_067` is the signed semantic-label canary. Root
  browser-measurement residuals remain owned by the exact root residual catalog.

## Known gaps (to be closed)

- Sanitization parity is in progress:
  - `nodes[].label`, `edges[].label`, and note text now flow through a Mermaid-inspired sanitizer
    (mirroring `common.sanitizeText*` behavior for common cases and using `securityLevel` / `flowchart.htmlLabels`).
  - Remaining gaps: full DOMPurify parity and `dompurifyConfig` option coverage.
- `stateDomId` / `graphItemCount` parity beyond the covered edge+note scenarios (e.g. more nested/doc translator cases).
- Click/link security-level behavior and renderer-specific handling (e.g. target/sandbox rules).
- Other statement forms in the upstream grammar.
- Diagnostics alignment (error messages and offsets).
