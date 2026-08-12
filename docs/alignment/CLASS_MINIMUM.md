# Class Diagram Admission Contract

This document defines the admitted `classDiagram` parser, model, Dagre/ELK layout, and SVG contract.

## Baseline

Upstream baseline: Mermaid `11.16.1` at
`7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`.

## Supported (current)

- Header:
  - `classDiagram`
  - `classDiagram-v2` (parsed as the same semantic model; detector selection depends on
    `class.defaultRenderer`)
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

## Remaining Gaps

- Remaining interactivity parity:
  - Full DOMPurify parity (and `dompurifyConfig` option coverage) for HTML labels/tooltips.
- Full name/label token parity (unicode tokenization, punctuation edge cases) with Mermaid Jison.
- Full error surface parity (token/loc/expected) with Mermaid Jison errors.
