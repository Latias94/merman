# Mindmap Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for mindmap parsing in `merman`.

## Supported (current)

- Header:
  - `mindmap` (case-insensitive), optionally preceded by empty lines.
  - Leading comment-only lines (e.g. `%% comment`) are ignored.
  - Trailing comments on the `mindmap` line are ignored (e.g. `mindmap %% comment`).
- Hierarchy:
  - indentation-based parent selection (Mirrors Mermaid `MindmapDB.getParent(level)`; `level` is the raw leading whitespace length minus the root’s base indent, not a normalized “depth” counter).
  - multiple roots are rejected with the Mermaid-compatible error message.
- Nodes:
  - bare IDs: `root`
  - explicit IDs + shaped labels:
    - `id[Label]` (rect)
    - `id(Label)` (rounded-rect)
    - `id((Label))` (circle)
    - `id{{Label}}` (hexagon)
    - `id)Label(` (cloud)
    - `id))Label((` (bang)
  - shaped labels without an explicit ID:
    - `(Label)` (id == descr)
  - quoted descriptions inside delimiters:
    - `id["String containing []"]` (brackets/parentheses inside the quoted descr are preserved)
- Decorations (apply to the most recently added node):
  - icon: `::icon(name)`
  - classes: `:::class1 class2`
- Comments:
  - full-line comments: `%% comment`
  - end-of-line comments: `... %% comment` (ignored outside quoted strings)
- Empty lines and whitespace-only lines are ignored.
- Layout-relevant details (Mirrors Mermaid `MindmapDB.getData()`):
  - rect-like nodes (`[]`, `()`, `{{}}`) double padding (2x)
  - generated node `domId` follows `node_<id>`
  - generated edge ids follow `edge_<parentId>_<childId>` and are unique

## Output shape (Phase 1)

- The semantic output is aligned with Mermaid `MindmapDB.getData()`:
  - `nodes` / `edges` for layout algorithms
  - `rootNode` holding the full tree
  - `config` reflecting the effective Mermaid config, forcing `layout = "cose-bilkent"` unless the user explicitly sets `layout`
  - section classes:
    - root: `mindmap-node section-root section--1`
    - children: `mindmap-node section-<n>` (plus any user-provided classes)
    - edges: `edge section-edge-<n> edge-depth-<depth>`

## Not yet implemented (Mermaid-supported)

- Full mindmap grammar parity beyond the minimum slice above (additional delimiter variants, advanced text escaping, and renderer-level semantics).

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `mindmap` grammar compatibility
at the pinned baseline tag.
