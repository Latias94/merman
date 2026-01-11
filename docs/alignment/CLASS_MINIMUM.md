# Class Diagram Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for `classDiagram` parsing in
`merman`.

## Baseline

Upstream baseline: `mermaid@11.12.2` (see `docs/adr/0001-upstream-baseline.md`).

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

## Not yet implemented (Mermaid-supported)

- Remaining interactivity parity:
  - Full DOMPurify parity (and `dompurifyConfig` option coverage) for HTML labels/tooltips.
- Full name/label token parity (unicode tokenization, punctuation edge cases) with Mermaid Jison.
- Full error surface parity (token/loc/expected) with Mermaid Jison errors.

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `classDiagram` grammar and
behavior compatibility at the pinned baseline tag.
