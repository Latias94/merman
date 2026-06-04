# EventModeling Minimum (Mermaid@11.15.0)

This document tracks the first local support slice for Mermaid `eventmodeling`.

Upstream references at locked commit `41646dfd43ac83f001b03c70605feb036afae46d`:

- Detector: `packages/mermaid/src/diagrams/eventmodeling/detector.ts`
- DB/model: `packages/mermaid/src/diagrams/eventmodeling/db.ts`
- Parser adapter: `packages/mermaid/src/diagrams/eventmodeling/parser.ts`
- Renderer: `packages/mermaid/src/diagrams/eventmodeling/renderer.ts`
- Styles: `packages/mermaid/src/diagrams/eventmodeling/styles.js`
- Types: `packages/mermaid/src/diagrams/eventmodeling/types.ts`
- Grammar: `packages/parser/src/language/eventmodeling/event-modeling.langium`
- Syntax docs: `docs/syntax/eventmodeling.md`

## Implemented (Phase 1)

- Detection:
  - accepts `eventmodeling`
  - exposes internal diagram id `eventmodeling`, matching upstream detector id
- Parser:
  - supports `tf` / `timeframe` and `rf` / `resetframe`
  - captures frame name, entity type, qualified entity identifier, explicit `->>` sources, inline data, and `[[dataReference]]`
  - captures `data` blocks as named data entities
  - ignores blank lines and whole-line `%%` comments
  - has source-backed fixture coverage for full `timeframe` syntax, qualified entity identifiers,
    and `resetframe`
- Render model:
  - typed `EventModelingDiagramRenderModel`
  - compatibility JSON from the same typed model
- Layout:
  - ports the upstream swimlane, box sizing, overlap, relation endpoint, and entity color defaults
  - supports `eventmodeling.padding`, `eventmodeling.useMaxWidth`, and eventmodeling theme variables
  - preserves local swimlane namespace state for stable lane reuse
- SVG:
  - Stage B renderer with upstream-shaped `.em-swimlane`, `.em-box`, `.em-relation`,
    `foreignObject` box labels, and `em-arrowhead-*` marker DOM signals
  - uses frame fill/stroke colors and `themeVariables.emRelationStroke`

## Semantic Policy Audit (P2E-004)

`entity`, `note`, and `gwt` are grammar-level constructs in upstream Mermaid
`packages/parser/src/language/eventmodeling/event-modeling.langium`, and the upstream parser
test named `should parse complex model` verifies that they can exist in the Langium AST. They do
not currently participate in Mermaid's eventmodeling render state: upstream
`packages/mermaid/src/diagrams/eventmodeling/db.ts` builds layout state by iterating
`ast.frames` and consulting `ast.dataEntities`, while upstream `types.ts` and `renderer.ts` expose
only swimlanes, boxes, and relations derived from frames.

Local policy:

| Statement | Upstream parser status | Upstream render status | Local status | Policy |
|---|---|---|---|---|
| `entity` | Parsed into `modelEntities`; also used as the nominal target of GWT statement references. | Not consumed by DB, layout state, or renderer. | Not represented in `EventModelingDiagramRenderModel`; skipped by the render-oriented parser. | Keep out of the render model until upstream renders or validates standalone model entities. |
| `note` | Parsed into `noteEntities` with a source frame and data block. | Not consumed by DB, layout state, or renderer; no syntax-doc or Cypress render example covers it. | Not represented in semantic/layout/SVG output; skipped by the render-oriented parser. | Deferred rendered feature; do not synthesize note boxes without an upstream rendering contract. |
| `gwt` | Parsed into `gwtEntities` with `given`/`when`/`then` statement groups. | Not consumed by DB, layout state, or renderer; upstream validation only checks frame source types. | Not represented in semantic/layout/SVG output; skipped by the render-oriented parser. | Deferred model feature; requires an explicit upstream or project-owned visual/semantic contract before promotion. |

The complex upstream parser test that includes these statements remains excluded from the normal
SVG parity corpus because rendering it would only prove that the current renderer ignores the
extra AST nodes. If merman later grows a full EventModeling AST export distinct from the render
semantic model, that fixture can be added as parser-only coverage without changing SVG admission.

## Data `foreignObject` Audit (P2E-005)

Upstream eventmodeling renders frame inline data and referenced data blocks inside the same
`foreignObject`/`span` box label used for frame names. The current Mermaid baseline:

- ignores the declared data type token (`json`, `md`, `html`, and the other documented data types
  share one renderer path)
- strips the outer `{...}` wrapper in `db.ts`
- runs the inner content through `sanitizeText`
- wraps by space-delimited words with `wrapLabel(..., joinWith: '<br/>')`
- replaces literal spaces with non-breaking spaces before injecting the final string with D3
  `.html(...)`
- sizes the box via browser `calculateTextDimensions(...)`, including the upstream
  data-content width workaround (`dimensions.width / 3`)

Local Stage B support preserves the render-model shape and emits upstream-shaped
`foreignObject` DOM, but it is intentionally not strict HTML/browser-layout parity:

| Area | Upstream behavior | Local behavior | Policy |
|---|---|---|---|
| Data type token | Parsed but not rendered specially. | Parsed data block content is stored without a typed rendering branch. | No action; this matches the upstream renderer contract. |
| Sanitization timing | Sanitizes while building the HTML label in Mermaid DB/render state. | Sanitizes during Rust parse, then XML-escapes code text during SVG emission. | Safe for the current corpus; not a claim of exact DOMPurify serialization parity. |
| HTML data content | Sanitized HTML can survive and is injected through `.html(...)`. | Code text is emitted as escaped XML text, so supported HTML is displayed literally. | Deferred; do not enable raw HTML in eventmodeling data without a focused security and parity pass. |
| Whitespace | Replaces spaces with non-breaking spaces and uses `<br/>` inserted by label wrapping. | Preserves ordinary spaces and literal newlines, then appends `<br/>` for multi-line code blocks. | Bounded residual; current DOM parity mode normalizes text/`&nbsp;` enough for admission. |
| Text metrics | Browser SVG `getBBox()` drives `calculateTextDimensions`, then applies the `/ 3` data width workaround. | Deterministic headless measurement over plain text lines. | Root/box geometry remains a text-metric residual, not a primary admission gate. |

Evidence from the committed data-block fixture
`fixtures/eventmodeling/upstream_cypress_eventmodeling_spec_renders_with_data_block_reference_004.mmd`:
family-local DOM parity passes, while the concrete SVGs still differ in browser-derived geometry
and data text serialization (for example non-breaking spaces in upstream code text versus ordinary
spaces in local output). Keep this distinction explicit when promoting future `html` or `md` data
fixtures.

## Known Gaps

- `xtask compare-eventmodeling-svgs --check-dom --dom-mode parity --dom-decimals 3` passes for the
  current committed baseline corpus.
- A committed upstream SVG baseline corpus exists under `fixtures/upstream-svgs/eventmodeling/`.
- `entity`, `note`, and `gwt` statements are intentionally outside the render semantic model; see
  the P2E-004 policy audit above.
- Strict layout parity is not claimed; local geometry still uses deterministic headless text
  measurement rather than browser `getBBox()` dimensions.
- Browser `foreignObject`, HTML sanitization, and `getBBox()` float parity are audited as bounded
  residuals for Phase 2; strict parity remains deferred.
