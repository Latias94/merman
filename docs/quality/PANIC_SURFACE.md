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
- `merman-render`:
  - Class namespace edge bucketing no longer unwraps the optional namespace root after a separate
    guard. Edges without complete same-root attribution degrade to outer-edge rendering instead of
    depending on that invariant staying panic-safe.
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
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo nextest run -p merman-render state` passed.
- `manatee`:
  - FCoSE relative-placement DAG construction no longer inserts keys and immediately unwraps
    mutable map lookups for source/destination adjacency, reverse edges, or indegree updates. The
    code now uses entry-based buckets so malformed or future-expanded relative-placement input does
    not depend on that local construction invariant staying panic-safe.
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
  - Flowchart, Ishikawa, TreeView, Treemap, and Mindmap now have explicit-stack coverage for
    representative deep or maximum-accepted inputs, but similar tree-shaped families should be
    audited before release hardening is considered complete.

## Suggested workflow

- When adding new code, prefer `Option`/`Result` over `unwrap/expect` unless it is in tests/examples.
- When porting upstream JS, treat “throw” sites as `Result` boundaries in Rust, unless upstream
  behavior explicitly crashes (rare).
