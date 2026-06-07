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
  - The default Dagre cycle-breaking path `acyclic::run(...)` no longer recurses through
    `dfs_fas(...)`. Deep acyclic successor chains now use explicit heap-backed DFS frames while
    preserving Dagre's node order, edge order, self-loop skip, and feedback-edge collection
    behavior.
- `dugong-graphlib`:
  - `alg::preorder(...)` and `alg::postorder(...)` no longer recurse over successor depth. Deep
    Graphlib successor chains now preserve traversal order through explicit stacks.
  - `alg::find_cycles(...)` no longer recurses through Tarjan SCC traversal. Deep public Graphlib
    successor chains now use explicit heap-backed frames and avoid stack overflow while preserving
    existing SCC and self-loop cycle reporting.
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
  - Mermaid config merging no longer depends on recursive `serde_json::Value` clone/drop for
    clone-on-write, `set_value(...)`, `deep_merge(...)`, frontmatter merges, directive merges, or
    legacy root `fontFamily` mirroring. Deep host `site_config` values now merge through explicit
    heap-backed traversal.
  - Init directive sanitization no longer recurses through user-authored JSON values. Directive
    object/array walking uses explicit path stacks, removes blocked `secure` / `__*` keys with
    non-recursive drops, and still clears blocked string values.
  - Frontmatter and init directive config bodies now reject nesting deeper than
    `MAX_DIAGRAM_NESTING_DEPTH` before entering the recursive YAML / JSON5 parsers, including
    flow collections, YAML indentation depth, and inline YAML sequence indicators. Accepted
    nesting continues through the existing merge semantics; excessive nesting returns a normal
    invalid-frontmatter or invalid-directive error instead of overflowing the Rust stack.
  - Frontmatter stripping in both preprocess and `DetectorRegistry::detect_type(...)` now uses
    line scanning instead of a broad DOTALL regex over user input.
  - Preprocess CRLF normalization no longer compiles a regex on the public preprocessing boundary.
    It now scans `\r\n` / `\r` line endings directly while preserving Mermaid's normalization to
    `\n`.
  - Preprocess Mermaid entity placeholder encoding no longer compiles the `#\w+;` entity regex or
    the integer-classification regex on the public preprocessing boundary. It now scans ASCII word
    entity placeholders directly, matching Mermaid's JavaScript `\w` source shape.
  - Preprocess `style` / `classDef` hex-protection no longer compiles regexes before entity
    placeholder encoding. It now scans each line for Mermaid's source-shaped
    `style.*:\S*#.*;` and `classDef.*:\S*#.*;` behavior, including the greedy final-semicolon
    boundary.
  - Preprocess HTML attribute cleanup no longer compiles tag or double-quoted-attribute regexes
    on the public preprocessing boundary. It now scans Mermaid's source-shaped
    `<(\w+)([^>]*)>` tag match and `="([^"]*)"` attribute rewrite directly, including JavaScript
    ASCII `\w` tag names and first-`>` tag termination.
  - Sanitizer line-break placeholder handling no longer compiles Mermaid common
    `/<br\s*\/?>/gi` on first public sanitize use. It now scans the same source-shaped `<br>`
    variants directly before escaping non-loose HTML labels.
  - Sanitizer URL-attribute minimal entity decoding no longer compiles fixed regexes for
    `&colon;`, `&newline;`, `&tab;`, decimal colon, or hex colon entities before URI validation.
    It now scans those fixed shapes directly while preserving the existing replacement order.
  - Sanitizer DOMPurify-like `data-*` / `aria-*` attribute-name validation no longer compiles
    fixed regexes on first public sanitize use. It now scans the pinned DOMPurify 3.4.0
    `DATA_ATTR` / `ARIA_ATTR` source shapes directly while preserving the existing configuration
    gates and validation order.
  - Sequence compat JSON construction no longer serializes the typed render model and then panics
    if expected object fields are missing. `SequenceDiagramRenderModel::to_compat_json(...)` now
    assembles the compatibility object directly, preserving serde rename behavior, optional
    `placement` / zero `centralConnection` omission, and autonumber integer/float encoding.
  - XYChart compat JSON construction no longer serializes the typed render model with
    `serde_json::to_value(...).expect(...)`. `XyChartDiagramRenderModel::to_compat_json(...)` now
    assembles the public object directly and clones retained effective config through the shared
    non-recursive JSON clone helper.
  - Block, State, Treemap, Sankey, C4, and Architecture semantic JSON roots now retain effective
    `config` through the shared non-recursive JSON clone helper instead of recursive
    `serde_json::Value::clone()`. C4, Sankey, and Architecture also assemble their final public
    root objects with hand-built maps so a deep retained config is moved into the result instead of
    being wrapped through `json!`.
  - GitGraph, Kanban, Packet, QuadrantChart, Radar, Requirement, and Mindmap semantic JSON roots
    now follow the same retained-config projection rule. Their public root objects are hand-built
    maps where needed, including Mindmap's empty-root early return, so retained host config is not
    recursively cloned or re-wrapped through `json!`.
  - C4 diagram detection no longer depends on lazily compiling a static regex on the first
    detection pass. The detector now uses equivalent string checks for Mermaid's upstream
    ungrouped regex shape, avoiding a fixed stack-heavy regex initialization point in small-stack
    public parse paths.
  - Detector/preprocess Mermaid comment cleanup no longer constructs a regex in
    `DetectorRegistry`. Both public detection and preprocessing now share a source-shaped line
    scanner that mirrors Mermaid 11.15 `cleanupComments`: remove indented `%%` comment lines with
    a non-empty comment body, preserve `%%{...}%%` directives until directive processing, trim
    leading blank/comment lines, and handle final comments without a trailing newline.
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
  - Architecture `iconText` XHTML fragment normalization no longer recurses while rewriting
    foreignObject namespaces or serializing the fragment tree. Public `architecture-beta` service
    `iconText` SVG output now covers a `1,200`-level nested XHTML fragment, while the lower-level
    normalizer covers a `2,048`-level fragment on a `64KB` stack.
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
  - Verification: `cargo +1.95 nextest run -p merman-core`, `cargo +1.95 fmt`, and
    `git diff --check` passed for the shared config/directive/frontmatter cleanup. Focused
    small-stack coverage now includes deep host `site_config`, accepted init/frontmatter config,
    excessive init/frontmatter config rejection, excessive inline YAML sequence rejection, deep
    directive sanitizer traversal, config clone-on-write, detector frontmatter stripping, and
    legacy non-string YAML key conversion behavior.
  - Verification: `cargo +1.95 fmt --check -p merman-core`,
    `cargo +1.95 nextest run -p merman-core parse_sequence_render_model_uses_typed_variant_without_changing_json_parse`,
    `cargo +1.95 nextest run -p merman-core sequence`, and `git diff --check` passed for the
    Sequence compat JSON construction cleanup.
  - Verification: `cargo +1.95 fmt --check -p merman-core`,
    `cargo +1.95 nextest run -p merman-core parse_xychart_render_model_uses_typed_variant_without_changing_json_parse`,
    `cargo +1.95 nextest run -p merman-core xychart`, and `git diff --check` passed for the
    XYChart compat JSON construction cleanup.
  - Verification: `cargo +1.95 nextest run -p merman-core
    retained_semantic_config_handles_deep_public_config_with_small_stack`,
    `cargo +1.95 nextest run -p merman-core
    block_render_model_uses_typed_variant_without_changing_json_parse
    treemap_render_model_uses_typed_variant_without_changing_json_parse
    parse_sankey_render_model_uses_typed_variant_without_changing_json_parse
    c4_render_model_uses_typed_variant_without_changing_json_parse`,
    `cargo +1.95 nextest run -p merman-core state architecture c4 block treemap sankey`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the retained
    semantic config projection cleanup.
  - Verification: `cargo +1.95 nextest run -p merman-core
    remaining_retained_semantic_config_handles_deep_public_config_with_small_stack`,
    `cargo +1.95 nextest run -p merman-core
    parse_kanban_render_model_uses_typed_variant_without_changing_json_parse
    parse_packet_render_model_uses_typed_variant_without_changing_json_parse
    parse_requirement_render_model_uses_typed_variant_without_changing_json_parse
    parse_radar_render_model_uses_typed_variant_without_changing_json_parse
    parse_gitgraph_render_model_uses_typed_variant_without_changing_json_parse
    parse_quadrant_chart_render_model_uses_typed_variant_without_changing_json_parse
    mindmap_render_model_projects_same_look_and_theme_shape_as_json_model`,
    `cargo +1.95 nextest run -p merman-core gitGraph kanban packet quadrant radar requirement
    mindmap`, `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the
    remaining retained semantic config projection cleanup.
  - Verification: `cargo +1.95 nextest run -p merman-core
    c4_detector_preserves_upstream_ungrouped_regex_shape
    auto_detect_common_headers_with_deep_config_small_stack`,
    `cargo +1.95 nextest run -p merman-core detect`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the C4 detector
    small-stack cleanup.
  - Verification: `cargo +1.95 nextest run -p merman-core
    cleanup_mermaid_comments_matches_mermaid_line_comment_shape
    detector_registry_strips_mermaid_comment_lines_without_regex
    preprocess_strips_mermaid_comment_at_eof_without_regex
    detector_registry_strips_deep_frontmatter_with_small_stack
    auto_detect_common_headers_with_deep_config_small_stack`,
    `cargo +1.95 nextest run -p merman-core detect`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the detector
    comment-cleanup regex removal.
  - Verification: `cargo +1.95 nextest run -p merman-core
    normalize_crlf_matches_mermaid_line_ending_cleanup
    preprocess_normalizes_crlf_without_regex
    preprocess_strips_mermaid_comment_at_eof_without_regex`,
    `cargo +1.95 nextest run -p merman-core detect`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the preprocess
    CRLF regex removal.
  - Verification: `cargo +1.95 nextest run -p merman-core
    encode_entity_placeholders_matches_mermaid_ascii_word_shape
    preprocess_encodes_entities_without_entity_regex
    preprocess_normalizes_crlf_without_regex`,
    `cargo +1.95 nextest run -p merman-core detect flowchart`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the preprocess
    entity placeholder regex removal.
  - Verification: `cargo +1.95 nextest run -p merman-core
    encode_entity_placeholders_matches_mermaid_ascii_word_shape
    preprocess_encodes_entities_without_entity_regex`,
    `cargo +1.95 nextest run -p merman-core detect flowchart`,
    `cargo +1.95 fmt --check -p merman-core`, and `git diff --check` passed for the preprocess
    style/classDef hex-protection regex removal.
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
  - Verification: `cargo fmt --check`,
    `cargo nextest run -p merman-render architecture_svg_handles_deep_icon_text_xhtml_fragment`,
    `cargo nextest run -p merman-render normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack`,
    `cargo nextest run -p merman-render architecture_layout_handles_deep_group_chain`,
    `cargo nextest run -p merman-render --test architecture_layout_test --test architecture_svg_test`,
    and `git diff --check` passed for the Architecture `iconText` XHTML fragment cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman`,
    `cargo nextest run -p merman-render --test class_svg_test`, and
    `cargo nextest run -p merman-render state` passed.
- `dugong` / `dugong-graphlib` cycle traversal:
  - `dugong_graphlib::alg::find_cycles(...)` now uses an iterative Tarjan traversal and is covered
    by a `2,048`-edge public Graphlib successor-chain regression on a `64KB` stack.
  - `dugong::acyclic::run(...)` now uses an iterative DFS feedback-arc scan for the default
    Dagre acyclicer path and is covered by a `2,048`-edge successor-chain regression on a `64KB`
    stack.
  - Verification: `cargo nextest run -p dugong-graphlib`,
    `cargo nextest run -p dugong`, `cargo nextest run -p merman-render --test class_svg_test`,
    `cargo nextest run -p merman-render --test flowchart_svg_test`,
    `cargo nextest run -p merman-render state`, `cargo fmt --check -p dugong -p dugong-graphlib`,
    and `git diff --check` passed.
- `manatee`:
  - FCoSE relative-placement DAG construction no longer inserts keys and immediately unwraps
    mutable map lookups for source/destination adjacency, reverse edges, or indegree updates. The
    code now uses entry-based buckets so malformed or future-expanded relative-placement input does
    not depend on that local construction invariant staying panic-safe.
  - FCoSE compound inclusion depth calculation and layout-base graph preorder reconstruction no
    longer recurse over compound nesting depth. Deep compound chains now use explicit heap-backed
    traversal and are covered by a `2,048`-level small-stack regression.
  - COSE-Bilkent radial tree placement no longer recurses while positioning deep forest branches.
    The public `layout_indexed(...)` path now uses explicit heap-backed frames for the former
    `branch_radial_layout(...)` traversal, so Mindmap-style deep trees do not depend on the Rust
    call stack before spring embedding.
  - Verification: `cargo fmt --check -p manatee -p merman-render`,
    `cargo nextest run -p manatee`, `cargo nextest run -p merman-render architecture`, and
    `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3`
    passed for the FCoSE relative-placement cleanup.
  - Final commit verification: `cargo fmt --check -p manatee -p merman-render -p merman` and
    `cargo nextest run -p manatee` passed.
  - Verification: `cargo +1.95 nextest run -p manatee
    layout_indexed_handles_deep_tree_radial_layout_with_small_stack`,
    `cargo +1.95 nextest run -p manatee`, `cargo +1.95 nextest run -p merman-render --test
    mindmap_svg_test`, and `cargo +1.95 fmt` passed for the COSE-Bilkent radial tree cleanup.
- `merman-ascii`:
  - Flowchart ASCII group raw-bounds calculation no longer recurses through nested subgraph
    members. The public `merman` ASCII API now resolves group bounds through explicit heap-backed
    enter/exit frames, so terminal rendering of accepted deep Flowchart subgraph chains no longer
    depends on Rust call-stack depth.
  - Verification: `cargo +1.95 nextest run -p merman --features ascii --test ascii_api
    render_ascii_model_handles_deep_flowchart_subgraph_chain_with_small_stack` and
    `cargo +1.95 nextest run -p merman --features ascii --test ascii_api` passed. No-test runs
    without `--features ascii` are not evidence because the integration test is feature-gated.

## Known remaining panic candidates (triage)

The following patterns are intentionally tolerated for now but should be tracked:

- Regex compilation via `Regex::new("...").unwrap()` in detector initialization:
  - input is a static literal; failures indicate a programming error, not user input.
- A small number of `unwrap/expect` in renderer internals:
  - most are on index/iterator operations that are guarded by bounds checks, but they are worth
    auditing because they can become input-reachable if assumptions drift.
- Deep recursive tree walkers in newly supported parser/render families:
  - Flowchart, Class namespaces, Architecture groups, Ishikawa, TreeView, Treemap, Mindmap, Block,
    C4, Architecture XHTML fragments, manatee/FCoSE compounds, ASCII Flowchart groups, and
    dugong/graphlib graph traversals now have explicit-stack coverage for representative deep or
    maximum-accepted inputs, but similar tree-shaped families should be audited before release
    hardening is considered complete.

## Suggested workflow

- When adding new code, prefer `Option`/`Result` over `unwrap/expect` unless it is in tests/examples.
- When porting upstream JS, treat “throw” sites as `Result` boundaries in Rust, unless upstream
  behavior explicitly crashes (rare).
