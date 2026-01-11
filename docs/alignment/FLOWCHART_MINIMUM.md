# Flowchart Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for flowchart parsing in `merman`.

## Supported (current)

- Header:
  - `graph <DIR>` (e.g. `graph TD`)
  - `flowchart <DIR>` (treated as flowchart-v2 by default config)
- Detection feature note:
  - Full build detects `flowchart-elk` as `flowchart-elk` and sets `layout=elk`.
  - Tiny build does not register `flowchart-elk` and falls back to `flowchart-v2`.
- Statement separators: `;` and newlines.
- Subgraphs:
  - Implemented subset:
    - `subgraph <title> ... end` (title-only; auto-id behavior matches Mermaid for titles containing whitespace)
    - `subgraph <id>[<title>] ... end` (explicit id + bracket title)
    - nested subgraphs supported
    - `direction <DIR>` inside subgraph is captured as `dir` on the subgraph
    - `labelType` matches Mermaid (`text`/`string`/`markdown`) for subgraph titles
    - `nodes` membership ordering matches Mermaid FlowDB (chain membership is reverse-ordered, e.g. `a-->b` contributes `["b","a"]`)
  - Note: this is not yet full Mermaid parity for subgraph semantics; see
    "Not yet implemented (Mermaid-supported)" below.
- Nodes:
  - bare IDs (`A`)
  - labeled shapes:
    - `A[Label]` (square)
    - `A(Label)` (round)
    - `A((Label))` (circle)
    - `A{Label}` (diamond)
    - `A{{Label}}` (hexagon)
    - `A[[Label]]` (subroutine)
    - `A(-Label-)` (ellipse)
    - `A([Label])` (stadium)
    - `A[(Label)]` (cylinder)
    - `A>Label]` (odd)
    - `A(((Label)))` (doublecircle)
    - `A[/Label/]` (lean_right)
    - `A[\\Label\\]` (lean_left)
    - `A[/Label\\]` (trapezoid)
    - `A[\\Label/]` (inv_trapezoid)
    - `A[|borders:lt|Label]` (rect)
  - `labelType` for node labels: `text` / `string` / `markdown` (quoted labels are `string`, backtick-wrapped labels are `markdown`)
  - Node label text supports unicode and common punctuation (including `/`, `\\`, and `<br>` as plain text)
  - Text label restrictions (Mermaid-compatible):
    - unquoted `text` labels reject `()[]{}"`; use quoted `string` labels to include these characters
  - shapeData (`@{...}`) is supported (Mermaid-compatible):
    - inline on nodes (e.g. `D@{ shape: rounded } --> E`, `C[Hello]@{ shape: circle }`)
    - standalone statements (e.g. `B@{ shape: circle }`)
    - supports `&` groups (e.g. `D@{ shape: rounded } & E`)
    - node metadata keys (subset aligned with Mermaid `NodeMetaData`):
      - `shape` (validated against Mermaid shapes registry; errors match Mermaid messages)
      - `label` (supports YAML multiline `|` and quoted multiline strings with Mermaid `<br/>` rewrite semantics)
      - `icon`, `img`, `form`, `pos`, `constraint`, `w`, `h`
    - lexer parity: `}` and `@` are allowed inside double-quoted strings; newlines inside double-quoted strings are rewritten to `<br/>`
  - Output note:
    - `shape` is the parsed vertex shape (e.g. `square`, `round`, `diamond`)
    - `layoutShape` mirrors Mermaid FlowDB `getData().nodes[].shape` (e.g. `squareRect`, `roundedRect`)
- Edges:
  - Mermaid-like link tokenization (`LINK` + `START_LINK` semantics):
    - normal (`--`), thick (`==`), dotted (`-.` / `...`), invisible (`~~~`)
    - start/end markers (`<`, `x`, `o`, `>`) including double-ended forms (e.g. `A<-->B`)
    - open-ended links are supported across strokes (e.g. `A---B`, `A===B`, `A-.-B`)
    - arrowhead variants are supported:
      - point: `-->` / `==>` / `-.->`
      - circle: `--o` / `==o` / `.-o`
      - cross: `--x` / `==x` / `.-x`
  - edge labels:
    - pipe form `|label|` (e.g. `A--x|text|B`)
    - "new notation" text between link parts (e.g. `A-- text -->B`, `A== text ==>B`, `A-. text .->B`)
  - edge semantic fields aligned with Mermaid FlowDB `destructLink`:
    - `type` (e.g. `arrow_point`, `double_arrow_point`)
    - `stroke` (`normal`/`thick`/`dotted`/`invisible`)
    - `length`
  - `labelType` for edge labels: `text` / `string` / `markdown` (same rules as node labels)
  - edge ids (`LINK_ID`) are supported (e.g. `A e1@-->B`):
    - auto IDs are always generated for edges without a user-defined ID: `L_<from>_<to>_<counter>`
    - if the same user-defined edge ID is used again, Mermaid-compatible fallback assigns an auto ID
    - edge metadata via shapeData statements:
      - `e1@{ curve: basis }` sets `interpolate`
      - `e1@{ animate: true }` / `e1@{ animation: fast }` are supported
  - supports inline node refs within chains (e.g. `A[Start]-->B{Is it?}`)
  - supports `&` groups in a minimal form (e.g. `a & b --> c & e` expands to multiple edges)
- Styles:
  - `style <ID> <style1>[,<style2>...]`
- Classes:
  - `classDef <id1>[,<id2>...] <style1>[,<style2>...]`
  - `class <id1>[,<id2>...] <className>`
  - inline vertex class via `:::ClassName` (minimal: single class)
- Link styles:
  - `linkStyle <index|index1,index2> <style...>` (bounds validated; error message matches Mermaid)
- Interactions:
  - `click <ID> "href"` / `click <ID> href "href"`
  - optional `"tooltip"` and optional target (`_blank`, etc.)
  - callback forms are parsed and marked `clickable`; callback execution depends on `securityLevel` (Mermaid parity)
  - link URLs are formatted like Mermaid `utils.formatUrl` (e.g. `javascript:` URLs become `about:blank` when `securityLevel != loose`)
  - tooltips and labels are sanitized like Mermaid `common.sanitizeText` (baseline parity; full DOMPurify parity is tracked as a gap)

## Not yet implemented (Mermaid-supported)

- Full flowchart class/style/click/linkStyle parity (advanced syntax, and renderer-level semantics).
- Complex edge routing tokens and multi-edge chains.
- Unicode escapes, HTML labels, and renderer-level markdown/HTML semantics (beyond baseline sanitization).
- Full Mermaid flowchart grammar compatibility.

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `flowchart`/`graph` grammar
compatibility at the pinned baseline tag.
