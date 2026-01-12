# Block Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Block diagram parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

Upstream references:

- Grammar: `repo-ref/mermaid/packages/mermaid/src/diagrams/block/parser/block.jison`
- DB behavior: `repo-ref/mermaid/packages/mermaid/src/diagrams/block/blockDB.ts`
- Tests: `repo-ref/mermaid/packages/mermaid/src/diagrams/block/parser/block.spec.ts`

## Supported (current)

- Headers:
  - `block`
  - `block-beta`
- Statements:
  - Column setting: `columns <n>` and `columns auto` (maps to `-1` columns).
  - Space blocks: `space` and `space:<n>`.
  - Node statements:
    - `id`
    - `id["label"]` and other supported bracket/paren shape delimiters (see “Node shapes”).
    - Width-in-columns: `id:2` / `id["Two slots"]:2` (colon size).
    - Edges:
      - `A --> B`
      - `A -- "label" --> B`
      - Link marker validation is aligned with upstream `LINK` token families (`--`, `==`, `-.`, `~~~` variants).
  - Composite blocks:
    - Anonymous: `block ... end` / `block-beta ... end`
    - Titled: `block:<id>["title"] ... end`
  - Style directives:
    - `classDef <id> <style,style,...>;`
    - `class <id[,id...]> <className>`
    - `style <id[,id...]> <style,style,...>`
- Node shapes (mapped via Mermaid’s `typeStr2Type`):
  - `[]` square
  - `()` round
  - `(())` circle
  - `((()))` doublecircle
  - `{}` diamond
  - `{{}}` hexagon
  - `([])` stadium
  - `[[]]` subroutine
  - `[()]` cylinder
  - `[//]` lean_right
  - `[\\\\]` lean_left
  - `[/\\]` trapezoid
  - `[\\/]` inv_trapezoid
  - `blockArrow<["..."]>(right|left|up|down|x|y[, ...])` block_arrow

## DB-level behavior (Phase 1)

- Labels are sanitized at DB-time (matches Mermaid’s `sanitizeText` in `blockDB.ts`).
- `space:<n>` expands into `n` concrete space blocks in the rendered children list.
- Edge ids are counted and rewritten as `<count>-<start>-<end>` (matches Mermaid’s `edgeCount` behavior).
- A warning is emitted when a node `widthInColumns` exceeds the configured `columns` for its parent composite.

## Output shape (Phase 1)

- The headless output is a snapshot aligned with Mermaid’s Block DB:
  - `type`
  - `blocks`: the hierarchical root children
  - `edges`: flattened edge list
  - `blocksFlat`: all blocks as a flat list (useful for parity assertions)
  - `classes`: collected `classDef` map
  - `warnings`: collected warning messages (string-equivalent to Mermaid `log.warn(...)`)
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `block` grammar and DB behavior
compatibility at the pinned baseline tag.

