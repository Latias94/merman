# HPD-050 - Class Namespace And Dugong Deep Traversal

Task: HPD-050 release-boundary panic-surface hardening

## Context

After Flowchart's nested subgraph path was converted to explicit stacks, the next tree-shaped
public input candidate was Class. Mermaid `classDiagram` input can nest `namespace` blocks, and the
local renderer routes those namespaces through dugong-adjacent compound graph layout before
emitting nested namespace SVG roots.

## Red Signal

A deep public namespace chain split cleanly by phase:

- parse-only stayed green;
- layout overflowed first through recursive dugong/graphlib traversal;
- after layout traversal was hardened, SVG output exposed recursive
  `render_class_namespace_root(...)` traversal.

Depths above `128` remained too expensive for a routine public Class regression on Windows, so the
public end-to-end test stays at `128` and the deeper stack-safety proof lives in cheaper
dugong/graphlib tests.

## Changes

- Replaced `dugong::rank::util::longest_path(...)` recursive DFS with an explicit frame stack that
  preserves child-rank and `minlen` propagation.
- Replaced `dugong_graphlib::alg::preorder(...)` and `postorder(...)` recursive DFS with explicit
  stacks while preserving successor order and missing-root panic behavior.
- Replaced `dugong::order::sort_subgraph_ix(...)`, timed sort-subgraph traversal, and public
  `sort_subgraph(...)` recursive compound traversal with explicit enter/exit frames.
- Replaced recursive Class namespace root SVG emission with explicit render frames while preserving
  the existing root, cluster, edge-label, node, child-root, edge-path, and close ordering.
- Added regressions for:
  - public Class parse, layout, and SVG output through a `128`-level namespace chain;
  - Graphlib preorder/postorder over a `2,048`-edge successor chain on a `64KB` stack;
  - dugong longest-path over a `2,048`-edge chain on a `64KB` stack;
  - public `sort_subgraph(...)` over a `2,048`-level compound chain on a `64KB` stack.

## Verification

- `cargo fmt --check -p dugong -p dugong-graphlib -p merman-render` - passed.
- `cargo nextest run -p dugong-graphlib --test alg_test` - passed.
- `cargo nextest run -p dugong --test rank_util_test` - passed.
- `cargo nextest run -p dugong --test order_sort_subgraph_test` - passed.
- `cargo nextest run -p merman-render --test class_svg_test` - passed.
- `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `git diff --check` - passed.

## Boundary

No SVG baseline, root override, Architecture root-bounds formula, or Mermaid parity fixture changed.
This is stack-safety hardening for a public Class namespace path plus its dugong/graphlib layout
dependencies. It does not claim closure of Class root residuals or Architecture solver diagnostics.
