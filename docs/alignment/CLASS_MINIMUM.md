# Class Diagram Admission Contract

This document defines the admitted `classDiagram` parser, model, Dagre/ELK layout, and SVG contract.

## Baseline

Selected upstream baseline: Mermaid `12.0.0` at
`98a0945418c76238f15df2afaddbba4272656c3b`. Historical fixture comments retain their
original collection versions.

## Supported (current)

- Header:
  - `classDiagram`
  - `classDiagram-v2` (the same semantic model and detector identity; layout uses top-level `layout`)
- Statement separators: newline
- Comments: `%% ...`
- Accessibility metadata:
  - `accTitle: ...`
  - `accDescr: ...`
  - multiline `accDescr { ... }`
- Direction:
  - `direction TB|BT|LR|RL`
- Classes:
  - `class <Name>`
  - `class <Name>["Text label"]`
  - inline css class shorthand: `class <Name>...:::<CssClass>`
  - member block: `class <Name> { <member lines> }`
  - standalone member statements: `<Name>: <member>`
- Members:
  - attributes vs methods classification using Mermaid rules (method if `)` is present)
  - annotations inside member lists: `<<annotation>>` (both as standalone statements and inside
    member blocks)
- Relations:
  - basic relations with `--` / `..` and endpoint markers (`<|`, `|>`, `*`, `o`, `()`, `<`, `>`)
  - relation labels: `A --> B : label`
- Notes:
  - `note for <Class> "text"`
  - `note "text"` (unattached note)
- CSS class assignment:
  - `cssClass "<ClassList>" <CssClass>` (comma-separated ids inside the string)
- Namespaces (class grouping):
  - `namespace <Name> { <class statements> }`
- Styles:
  - `style <Class> <style...>` (e.g. `style Class01 fill:#f9f,stroke:#333`)
  - `classDef <CssClass> <style...>` (applies styles to already-defined classes that have the css class)
- Interactivity (headless metadata only):
  - `link <Class> "<url>" ["<tooltip>"] [<_target>]`
  - `click <Class> href "<url>" ["<tooltip>"] [<_target>]`
  - `click <Class> call <function>(<args?>) ["<tooltip>"]`
  - `callback <Class> "<function>" ["<tooltip>"]`
  - link/click URLs are formatted like Mermaid `utils.formatUrl` (e.g. `javascript:` URLs become
    `about:blank` when `securityLevel != loose`)
  - tooltips and other user-visible strings are sanitized like Mermaid `common.sanitizeText`
    (baseline parity; full DOMPurify parity is tracked as a gap)

## Layout And SVG Admission

- Namespace, class, note, lollipop interface, note-edge, and relation insertion order follows
  Mermaid `ClassDB.getData()` and the shared Dagre renderer.
- Historical direct comparison against pinned `dagre-d3-es` established zero drift for graph
  dimensions, node positions, edge-label anchors, routed points, and stable identities. The
  standing contract is now owned by Dugong algorithm tests plus the signed Class SVG and semantic
  canaries below.
- Edge labels use shared updated-path geometry while cardinality terminal labels retain their
  Class-specific marker offsets.
- The complete embedded stylesheet is tested byte-for-byte after scope-id normalization. Common
  CSS, Class rules, marker order, icon rules, Neo rules, theme `strokeWidth`, and final `:root`
  order are part of the contract.
- `stress_class_many_relations_labels_020` is the signed semantic-label canary; nested namespaces
  are additionally covered by `stress_class_nested_namespaces_cross_edges_008`.

### Mermaid 12 ELK paint routes

The default Class renderer consumes the routes prepared by the common ELK paint pass.
`straightenTerminalJogs` in the pinned `layout-algorithms/elk/render.ts` moves a short
terminal channel onto the existing port row only when this introduces no extra crossings.
The SVG painter must use those prepared edges, including repositioned terminal labels;
reloading edges from the layout would silently discard the pass. Explicit
`elk.straightenEdges: false` preserves the original channels.

`class_svg_elk_paints_straightened_terminal_channels_without_moving_ports` covers both
source and target staircases in `stress_class_many_relations_labels_020`, checks unchanged
ports, and distinguishes the opt-out. The eight signed label residuals for that canary
cover browser measurement only after this routing correction. `shapeUtil.ts` uses
`getBBox()` and `getBoundingClientRect()` for Class text; their width differences propagate
through ELK node placement, channel lengths, label anchors, and Neo dash masks. Label text,
style, markers, CSS, edge identity, and route topology remain checked.

### Namespace paint order and the font-size receipt

The common ELK painter emits `clusters`, `edges edgePaths`, `edgeLabels`, then `nodes`,
matching pinned `rendering-util/createGraph.ts`. Namespace backgrounds therefore paint
behind relation paths and markers. Margin marker paths also retain the shared
`markers.js` stroke widths: aggregation uses 2; composition and dependency use 0.

The exact browser-text receipt for
`stress_class_svg_font_size_px_string_precedence_026` was rebound after these corrections.
An old/new compiled-render replay reproduced the previously admitted signature and
proved that reversing only the six marker style additions and empty-group ordering/class
change restored the old SVG byte-for-byte. All path geometry, text/tspan subtrees, CSS,
and root attributes were unchanged, as were the input and pinned upstream SVG hashes.
The original font residual remains: the first long member occupies two local text rows
versus three upstream, retaining the same concatenated-text space difference. Receipt
modes, precision, and comparator policy are unchanged; this refresh admits no new font
or layout difference.

## Remaining Gaps

- Remaining interactivity parity:
  - Full DOMPurify parity (and `dompurifyConfig` option coverage) for HTML labels/tooltips.
- Full name/label token parity (unicode tokenization, punctuation edge cases) with Mermaid Jison.
- Full error surface parity (token/loc/expected) with Mermaid Jison errors.
