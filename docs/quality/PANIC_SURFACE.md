# Panic Surface Policy

This repo is parity-focused, but it is also intended to be used as a library in headless contexts.
Library code should not panic on user-controlled input.

## Policy

- **No panics in library code on user input.**
  - Avoid `unwrap()` / `expect()` in production code paths that can be reached by parsing or
    rendering untrusted Mermaid text, or by calling public APIs with arbitrary data.
- **Panics are acceptable** in:
  - tests, examples, and `xtask`
  - generated code (e.g. parser generator output)
  - “impossible states” guarded by prior checks (prefer `debug_assert!` if it helps)
- When an invariant is violated, prefer:
  - returning an error when the caller can act on it
  - degrading gracefully (best-effort output) when strictness would be counterproductive (e.g.
    layout on disconnected graphs)

## Current status (2026-06-07)

- `dugong` (Dagre port):
  - No `unwrap/expect/panic!` usage in `crates/dugong/src` (production code).
  - Layout-related helpers are now defensive against:
    - empty graphs
    - disconnected graphs (build a forest instead of panicking)
    - missing node/rank metadata (treat as defaults where possible)
  - `rank::util::longest_path(...)`, `order::sort_subgraph(...)`, and
    `order::sort_subgraph_ix(...)` no longer recurse over user-controlled graph depth. Deep edge
    chains and compound subgraph chains now use explicit heap-backed traversal.
- `dugong-graphlib`:
  - `alg::preorder(...)` and `alg::postorder(...)` no longer recurse over successor depth. Deep
    Graphlib successor chains now preserve traversal order through explicit stacks.
- `merman-core`:
  - `MermaidConfig::set_value` no longer panics if the config was constructed from a non-object
    JSON value (it coerces to an object).
  - Ishikawa render-model construction and semantic JSON projection no longer recurse over the
    user-authored tree. Deeply nested Ishikawa input now uses explicit heap-backed traversal for
    arena-to-tree conversion, flattened node projection, and root JSON projection.
  - TreeView render-model construction and semantic JSON projection no longer recurse over the
    user-authored tree. The parser still enforces `MAX_DIAGRAM_NESTING_DEPTH`, but accepted
    `treeView-beta` chains now use explicit heap-backed traversal for arena-to-tree conversion,
    flattened node projection, and root JSON projection.
  - Treemap render-model construction and semantic JSON projection no longer recurse over the
    user-authored hierarchy. `parse_treemap(...)` now builds deep semantic `root` / `nodes` values
    with explicit heap-backed traversal and hand-built `Map` output, avoiding deep `json!`
    serialization of user-authored trees.
  - Mindmap section assignment, flat semantic node/edge projection, typed render-model projection,
    and nested `rootNode` JSON projection no longer recurse over the user-authored hierarchy.
    Deep semantic output is assembled with explicit heap-backed traversal and hand-built final JSON
    maps so the nested `rootNode` is moved into the result instead of being wrapped through deep
    `json!` serialization.
  - Block deep composite hierarchies no longer depend on recursive `Clone` while populating parent
    children or projecting `blocksFlat`, and `parse_block(...)` now assembles the final semantic
    object with a hand-built map instead of wrapping deep block trees through `json!`.
- `merman-render`:
  - Class namespace edge bucketing no longer unwraps the optional namespace root after a separate
    guard. Edges without complete same-root attribution degrade to outer-edge rendering instead of
    depending on that invariant staying panic-safe.
  - Class namespace rendering no longer recursively emits nested namespace root groups. Deep public
    `classDiagram` `namespace` chains now parse, layout, and render SVG through explicit frame
    traversal while preserving the existing root/group/node/edge output order.
  - State edge segment merging no longer unwraps the last accumulated point after a separate
    non-empty guard. Duplicate segment-boundary points are still skipped when present; an unexpected
    empty accumulator now falls through to normal point insertion.
  - Ishikawa layout no longer recurses over user-authored cause/subcause trees while counting
    descendants or flattening label entries. The odd-depth parent-bone lookup now degrades to the
    current branch bone instead of panicking if the traversal invariant is ever violated.
  - TreeView layout no longer recurses over user-authored tree nodes. The layout pass now uses an
    explicit enter/exit stack, preserving preorder node output and postorder vertical-line output
    while keeping the existing depth-limit error for invalid typed models.
  - Treemap layout no longer recurses over user-authored hierarchy nodes while building layout
    nodes, computing subtree sums, or sorting children. The semantic-JSON layout entrypoint now
    projects Treemap nodes iteratively instead of relying on recursive serde deserialization.
  - Mindmap's semantic-JSON layout entrypoint now deserializes only the flat `nodes` / `edges`
    fields consumed by layout, avoiding recursive serde traversal of the deep semantic `rootNode`
    compatibility field.
  - Block's semantic-JSON layout and SVG entrypoints now project `blocksFlat` through an explicit
    heap-backed traversal instead of recursive serde over nested block children. Block SVG metadata
    collection also uses an explicit stack instead of recursive `collect_nodes(...)`.
  - C4 layout no longer recurses over user-authored boundary/deployment-node nesting. The layout
    pass now uses an explicit heap-backed frame stack while preserving the existing parent-bounds
    accumulation semantics for sibling rows, shapes, child boundaries, and root bounds.
  - State composite hierarchies no longer depend on recursive Rust stack traversal for public
    semantic JSON projection, typed render-model parsing, cluster extraction/preparation, nested
    prepared-graph layout, or prepared-graph/AST cleanup. Deep semantic `doc` values are assembled
    with hand-built `Map` output so they are moved into the result instead of recursively wrapped
    through `json!`.
  - Flowchart deep subgraph chains no longer depend on recursive Rust stack traversal for
    extracted cluster placement, fallback subtree rectangle collection, cluster rectangle
    postorder computation, or nested SVG root rendering. A public `flowchart TB` / `subgraph`
    chain now covers parse-for-render-model, layout, and SVG output through ordinary render APIs.
  - Architecture deep group chains no longer depend on recursive Rust stack traversal in SVG group
    rectangle computation. Public `architecture-beta` group chains now cover parse, layout, and SVG
    output through ordinary render APIs, while the renderer-side group-rect calculator has a
    separate `2,048`-level small-stack regression.
  - `layout_parsed(...)` now clones retained semantic JSON with an explicit heap-backed traversal,
    avoiding stack overflow when a supported parser intentionally returns a deeply nested
    `serde_json::Value`.
  - Verification: `cargo fmt --check -p merman-render`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3 --filter namespace`
    passed for the Class namespace cleanup.
  - Verification: `cargo nextest run -p merman-render state` and
    `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the State edge segment cleanup.
  - Verification: `cargo fmt --check -p merman-core -p merman-render`,
    `cargo nextest run -p merman-core ishikawa`,
    `cargo nextest run -p merman-render --test ishikawa_svg_test`, and `git diff --check` passed
    for the Ishikawa deep-tree cleanup.
  - Verification: `cargo nextest run -p merman-core tree_view`,
    `cargo nextest run -p merman-render --test tree_view_svg_test`, and
    `cargo run -p xtask -- compare-tree-view-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the TreeView depth-boundary cleanup.
  - Verification: `cargo nextest run -p merman-core treemap`,
    `cargo nextest run -p merman-render --test treemap_svg_test`, and
    `cargo run -p xtask -- compare-treemap-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the Treemap deep-tree cleanup.
  - Verification: `cargo nextest run -p merman-core mindmap`,
    `cargo nextest run -p merman-render --test mindmap_svg_test`, and
    `cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the Mindmap deep-tree cleanup.
  - Verification: `cargo nextest run -p merman-core block`,
    `cargo nextest run -p merman-render --test block_svg_test`, and
    `cargo run -p xtask -- compare-block-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the Block deep-composite cleanup. The new `1,200`-level Block regressions reproduced
    stack overflow before the non-recursive clone/projection changes.
  - Verification: `cargo fmt --check -p merman-render`,
    `cargo nextest run -p merman-render c4`, and
    `cargo run -p xtask -- compare-c4-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the C4 deep-boundary cleanup. The new `1,500`-level C4 regression reproduced stack
    overflow before the non-recursive layout traversal.
  - Verification: `cargo fmt --check -p merman-render`,
    `cargo nextest run -p merman-render flowchart`,
    `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`,
    and `git diff --check` passed for the Flowchart deep-subgraph cleanup. The new `1,200`-level
    Flowchart regression reproduced stack overflow in the public layout path before the
    non-recursive layout placement/cluster-rect changes.
  - Verification: `cargo fmt --check -p dugong -p dugong-graphlib -p merman-render`,
    `cargo nextest run -p dugong-graphlib --test alg_test`,
    `cargo nextest run -p dugong --test rank_util_test`,
    `cargo nextest run -p dugong --test order_sort_subgraph_test`,
    `cargo nextest run -p merman-render --test class_svg_test`,
    `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`,
    and `git diff --check` passed for the Class namespace / dugong deep traversal cleanup.
  - Verification: `cargo fmt --check -p manatee -p merman-render`,
    `cargo nextest run -p manatee`,
    `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test`,
    `cargo nextest run -p merman-render group_rect_computer_handles_deep_child_group_chain_with_small_stack`,
    `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`,
    and `git diff --check` passed for the Architecture deep-group cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo nextest run -p merman-render state` passed.
- `manatee`:
  - FCoSE relative-placement DAG construction no longer inserts keys and immediately unwraps
    mutable map lookups for source/destination adjacency, reverse edges, or indegree updates. The
    code now uses entry-based buckets so malformed or future-expanded relative-placement input does
    not depend on that local construction invariant staying panic-safe.
  - FCoSE compound inclusion depth calculation and layout-base graph preorder reconstruction no
    longer recurse over compound nesting depth. Deep compound chains now use explicit heap-backed
    traversal and are covered by a `2,048`-level small-stack regression.
  - Verification: `cargo fmt --check -p manatee -p merman-render`,
    `cargo nextest run -p manatee`, `cargo nextest run -p merman-render architecture`, and
    `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the FCoSE relative-placement cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman` and
    `cargo nextest run -p manatee` passed.

## Known remaining panic candidates (triage)

The following patterns are intentionally tolerated for now but should be tracked:

- Regex compilation via `Regex::new("...").unwrap()` in detector initialization:
  - input is a static literal; failures indicate a programming error, not user input.
- A small number of `unwrap/expect` in renderer internals:
  - most are on index/iterator operations that are guarded by bounds checks, but they are worth
    auditing because they can become input-reachable if assumptions drift.
- Deep recursive tree walkers in newly supported parser/render families:
  - Flowchart, Class namespaces, Architecture groups, Ishikawa, TreeView, Treemap, Mindmap, Block,
    C4, manatee/FCoSE compounds, and dugong/graphlib graph traversals now have explicit-stack
    coverage for representative deep or maximum-accepted inputs, but similar tree-shaped families
    should be audited before release hardening is considered complete.

## Suggested workflow

- When adding new code, prefer `Option`/`Result` over `unwrap/expect` unless it is in tests/examples.
- When porting upstream JS, treat “throw” sites as `Result` boundaries in Rust, unless upstream
  behavior explicitly crashes (rare).
