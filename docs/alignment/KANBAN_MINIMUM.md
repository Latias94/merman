# Kanban Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for Kanban parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `kanban` (case-insensitive).
  - Allows empty lines above the header (preprocessing trims leading whitespace).
- Nodes:
  - Line indentation (leading whitespace length) is used as the node `level`.
  - Node forms (aligned with Mermaid’s Jison grammar, shared with mindmap):
    - `id` (no brackets): `root`
    - `id(label)`: `theId(child1)`
    - `id[label]`: `root[The root]`
    - `(label)` / `((label))` / `(-label-)` / `{{label}}` etc.
    - If no explicit id is provided (e.g. `(root)`), id defaults to the label.
- Hierarchy rules:
  - The first node defines the “section level”.
  - Nodes at the section level are treated as sections (columns).
  - Nodes deeper than the section level become items and are attached to the most recent section.
  - If any previously parsed node has a level lower than the section level, parsing fails with:
    - `Items without section detected, found section ("...")`
- Decorations (applied to the last parsed node):
  - `::icon(name)` sets `icon`.
  - `:::class list` sets `cssClasses`.
- Inline comments:
  - Trailing `%% ...` is stripped unless inside quotes (mirrors Mermaid tokenization).
- Task metadata (`@{ ... }`):
  - Supports both single-line and multi-line blocks.
  - Parsed as YAML (JSON-like schema in Mermaid); supported keys:
    - `priority`, `assigned`, `ticket`, `icon`, `label`, `shape`
  - `label` overrides the node label (no sanitization at DB-time; matches Mermaid).
  - `shape` validation:
    - If `shape` is not lowercase or contains `_`, parsing fails with:
      - `No such shape: <shape>. Shape names should be lowercase.`
    - Only `shape: kanbanItem` is accepted as a node shape override (matches Mermaid DB behavior).
  - Double-quoted strings inside `@{ ... }` rewrite `\n\\s*` to `<br/>` (matches Mermaid lexer behavior).

## Output shape (Phase 1)

- The semantic output is a headless snapshot aligned with Mermaid’s Kanban DB behavior:
  - `type`
  - `sections`: the raw section nodes (similar to `kanbanDb.getSections()`), used by parity tests
  - `nodes`: `kanbanDb.getData().nodes`-like layout nodes (sections + items)
  - `edges: []`, `other: {}`
  - `config`

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `kanban` grammar and DB behavior
compatibility at the pinned baseline tag.

