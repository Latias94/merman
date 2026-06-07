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
  - `rank::util::longest_path(...)` also no longer exposes a frame-pop `expect(...)` after that
    iterative conversion. If the explicit stack invariant is unexpectedly violated, traversal exits
    instead of panicking in the layout ranker.
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
  - `json::write(...)` and `json::write_with_defaults(...)` no longer panic if a graph id/key
    iterator and label lookup drift apart. Missing live node/edge labels now return a
    `serde_json::Error` backed by `InvalidData` instead of triggering JSON writer invariant
    `expect(...)` calls.
  - `Graph` core compaction, adjacency-cache ensure helpers, and edge endpoint insertion no longer
    expose internal invariant `expect(...)` calls. Unexpected cache or index drift now falls back to
    empty/best-effort state or returns from the mutator instead of panicking on the internal guard.
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
  - Block parent-child population and document parsing no longer expose explicit-stack frame
    invariant `expect(...)` calls. Unexpected populate-stack drift exits the loop, and unexpected
    document-frame drift returns a normal block `DiagramParse` error instead of panicking.
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
  - Sanitizer URI-attribute whitespace cleanup no longer compiles the DOMPurify
    `ATTR_WHITESPACE` character-class regex before URI validation. It now scans the pinned
    DOMPurify 3.4.0 character set directly and preserves the cleanup timing before both URI
    allowlist validation and the unknown-protocol script/data guard.
  - Sanitizer unknown-protocol script/data guarding no longer compiles the DOMPurify
    `IS_SCRIPT_OR_DATA` regex. It now scans the pinned DOMPurify 3.4.0 source shape directly,
    preserving ASCII `\w+script:` and case-insensitive `data:` behavior.
  - Sanitizer URI allowlist validation no longer compiles the DOMPurify `IS_ALLOWED_URI` regex.
    It now scans the pinned DOMPurify 3.4.0 source shape directly and intentionally aligns the
    default safe scheme set with upstream by allowing `matrix:`.
  - `sanitize_url(...)` no longer compiles the two `@braintree/sanitize-url` cleanup regexes for
    named HTML control entities or whitespace escape sequences. It now scans the installed
    `@braintree/sanitize-url` 7.1.2 source shapes directly while preserving Mermaid's `^7.1.1`
    dependency behavior covered by the existing sanitize-url vectors.
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
  - Flowchart subgraph membership extraction no longer exposes invariant `expect(...)` panics while
    walking the explicit frame stack. Unexpected empty-stack states degrade to the partially
    accumulated root items instead of panicking on the public flowchart parse/model boundary.
  - C4 diagram detection no longer depends on lazily compiling a static regex on the first
    detection pass. The detector now uses equivalent string checks for Mermaid's upstream
    ungrouped regex shape, avoiding a fixed stack-heavy regex initialization point in small-stack
    public parse paths.
  - Class diagram member parsing and multiline `accDescr` normalization no longer compile
    ClassDB-local regex helpers. Method parsing now uses a source-shaped scanner for Mermaid's
    greedy member regex, and multiline accessibility descriptions collapse newline indentation
    directly.
  - The `zed_50558_class_inheritance` ClassDB golden now matches the source-shaped method parser
    boundary by preserving the space after `+` visibility markers in method ids and display text.
  - Gantt date/duration relative-reference helpers no longer compile the remaining core-local
    regexes. ASCII digit checks, source-shaped `after` / `until` ID capture, duration parsing, and
    strict `YYYY-MM-DD` shape checks now use direct scanners aligned with Mermaid 11.15
    `ganttDb.js`.
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
  - Architecture foreignObject XHTML namespace rewriting no longer panics if its explicit frame
    stack invariant is unexpectedly violated. The defensive fallback returns an empty rewritten
    fragment instead of exposing a library panic on the SVG/HTML normalization boundary.
  - `layout_parsed(...)` now clones retained semantic JSON with an explicit heap-backed traversal,
    avoiding stack overflow when a supported parser intentionally returns a deeply nested
    `serde_json::Value`.
  - RaTeX math-only label splitting no longer compiles a feature-gated `<br>` regex on the render
    path. It now reuses the shared Mermaid `lineBreakRegex = /<br\s*\/?>/gi` scanner used by
    ordinary HTML-label wrapping.
  - FontAwesome icon-token substitution in HTML-ish labels no longer compiles a static regex on
    the render text path. `replace_fontawesome_icons(...)` now scans Mermaid's
    `/(fa[bklrs]?):fa-([\w-]+)/g` source shape directly while preserving the existing local
    `<i class="...">` fallback output.
  - SVG pipeline CSS override processing no longer compiles a static `!important` regex. The
    local `strip_css_important(...)` helper now scans case-insensitive `!important` markers
    directly while preserving the previous trailing word-boundary behavior.
  - SVG pipeline CSS sanitization no longer compiles static regexes for animation declaration
    stripping or CSS degree-unit stripping. `strip_animation_declarations(...)` now scans the
    local `(^|[;{])\s*animation(?:-[a-z-]+)?\s*:[^;}]*;?` boundary directly, and
    `strip_css_deg_units(...)` scans the local `(-?\d+(?:\.\d+)?)deg\b` boundary directly for
    raster-safe output.
  - SVG pipeline attribute sanitization no longer compiles the local double-quoted attribute
    regex. Tag attribute rewriting and bad-`rect` dimension lookup now share a scanner for the
    previous `\s+([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*"([^"]*)"` shape.
  - ER path label-coordinate detection no longer compiles the local decimal regex. The helper now
    scans path `d` strings for ASCII decimal substrings matching the previous `\d+\.\d+` shape and
    rounds them before the existing coordinate containment heuristic.
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
  - Verification: `cargo +1.95 fmt -p merman-core`,
    `cargo +1.95 nextest run -p merman-core block`,
    `rg -n 'populate frame should exist|document frame should exist|root document frame should exist|parent document frame should exist' crates/merman-core/src/diagrams/block.rs`,
    `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse, and
    `git diff --check` passed for the Block frame-invariant panic-surface cleanup.
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
  - Verification: `cargo +1.95 run -p xtask -- update-snapshots --diagram all --filter zed_50558_class_inheritance`,
    `cargo +1.95 nextest run -p merman-core --test snapshots`,
    `cargo +1.95 nextest run -p merman-core class`, and `git diff --check` passed for the
    ClassDB snapshot gate follow-up.
  - Verification: `cargo +1.95 fmt -p merman-core`,
    `cargo +1.95 nextest run -p merman-core gantt`,
    `cargo +1.95 fmt --check -p merman-core`, `git diff --check`, and
    `rg -n 'regex::Regex|Regex::new|OnceLock<Regex>|OnceLock\s*<\s*Regex' crates/merman-core/src -g '*.rs'`
    passed for the Gantt regex removal; the final `rg` reported no production core regex
    compile/cache matches.
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
  - Verification: `cargo +1.95 fmt -p merman-core`,
    `cargo +1.95 nextest run -p merman-core flowchart`,
    `rg -n 'frame stack should not be empty|current frame should exist|finished frame should exist' crates/merman-core/src/diagrams/flowchart/subgraph.rs`,
    and `git diff --check` passed for the Flowchart subgraph builder frame-invariant panic-surface
    cleanup.
  - Verification: `cargo fmt --check -p dugong -p dugong-graphlib -p merman-render`,
    `cargo nextest run -p dugong-graphlib --test alg_test`,
    `cargo nextest run -p dugong --test rank_util_test`,
    `cargo nextest run -p dugong --test order_sort_subgraph_test`,
    `cargo nextest run -p merman-render --test class_svg_test`,
    `cargo run -p xtask -- compare-class-svgs --check-dom --dom-mode parity --dom-decimals 3`,
    and `git diff --check` passed for the Class namespace / dugong deep traversal cleanup.
  - Verification: `cargo +1.95 fmt -p dugong`,
    `cargo +1.95 nextest run -p dugong --test rank_util_test`,
    `rg -n 'longest-path frame should exist' crates/dugong/src/rank/util.rs`, and
    `git diff --check` passed for the Dugong longest-path frame-pop panic-surface cleanup.
  - Verification: `cargo +1.95 fmt -p dugong-graphlib`,
    `cargo +1.95 nextest run -p dugong-graphlib --test json_test`,
    `rg -n 'node_ids\(\) should only yield live nodes|edge_keys\(\) should only yield live edges' crates/dugong-graphlib/src/json.rs`,
    `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse, and
    `git diff --check` passed for the Graphlib JSON writer invariant panic-surface cleanup.
  - Verification: `cargo +1.95 fmt -p dugong-graphlib`,
    `cargo +1.95 nextest run -p dugong-graphlib --test graph_core_test`,
    `rg -n 'children_ix resized to node slots|directed adjacency cache should be present after ensure|undirected adjacency cache should be present after ensure|ensure_node should have inserted the endpoint node' crates/dugong-graphlib/src/graph/core.rs`,
    `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse, and
    `git diff --check` passed for the Graphlib core invariant panic-surface cleanup.
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
  - Verification: `cargo +1.95 fmt -p merman-render`,
    `cargo +1.95 nextest run -p merman-render normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack architecture_svg_handles_deep_icon_text_xhtml_fragment`,
    `rg -n 'rewrite frame should exist' crates/merman-render/src/svg/parity/architecture/foreign_object.rs`,
    and `git diff --check` passed for the Architecture foreignObject rewrite-frame panic-surface
    cleanup.
  - Verification: `cargo +1.95 fmt -p merman-render`,
    `cargo +1.95 nextest run -p merman-render fontawesome`, and
    `rg -n 'Regex|regex::|OnceLock|fontawesome_icon_at|replace_fontawesome_icons' crates/merman-render/src/text/icons.rs crates/merman-render/src/text/tests.rs`
    passed for the FontAwesome icon-token regex removal; `text/icons.rs` has no regex dependency
    matches.
  - Verification: `cargo +1.95 fmt -p merman-render`,
    `cargo +1.95 nextest run -p merman-render important`, and
    `rg -n 'Regex|regex::|OnceLock|css_important|strip_css_important' crates/merman-render/src/svg/pipeline/builtin/css_override.rs crates/merman-render/src/svg/pipeline/builtin/scoped_css.rs`
    passed for the CSS `!important` regex removal; `css_override.rs` has no regex dependency
    matches.
  - Verification: `cargo +1.95 fmt --check -p merman-render`,
    `cargo +1.95 nextest run -p merman-render css_sanitize resvg_safe`,
    `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs`,
    and `git diff --check` passed for the CSS sanitizer regex removal; `css_sanitize.rs` has no
    regex dependency matches.
  - Verification: `cargo +1.95 fmt -p merman-render`,
    `cargo +1.95 nextest run -p merman-render attr_sanitize resvg_safe`,
    `cargo +1.95 fmt --check -p merman-render`,
    `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/pipeline/builtin/attr_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_sanitize.rs crates/merman-render/src/svg/pipeline/builtin/css_override.rs`,
    `rg -n "regex::Regex|Regex::new|OnceLock<regex::Regex>|OnceLock\s*<\s*Regex|regex::Captures|Captures<'" crates/merman-render/src -g '*.rs'`,
    and `git diff --check` passed for the SVG attribute sanitizer regex removal. The builtin SVG
    sanitizer files have no regex dependency matches; the precise render-wide regex scan reports
    only `svg/parity/er.rs`.
  - Verification: `cargo +1.95 fmt -p merman-render`,
    `cargo +1.95 nextest run -p merman-render er`,
    `cargo +1.95 nextest run -p merman-render er_label_coordinate_path_decimal_rounding_without_regex`,
    `cargo +1.95 nextest run -p merman-render --test er_svg_test`,
    `cargo +1.95 fmt --check -p merman-render`,
    `rg -n 'Regex|regex::|OnceLock' crates/merman-render/src/svg/parity/er.rs`,
    `rg -n "regex::Regex|Regex::new|OnceLock<regex::Regex>|OnceLock\s*<\s*Regex|regex::Captures|Captures<'" crates/merman-core/src crates/merman-render/src -g '*.rs'`,
    and `git diff --check` passed for the ER decimal regex removal. The precise production
    `merman-core/src` plus `merman-render/src` regex compile/cache scan now reports no matches;
    `merman-render` keeps `regex` only as a test dependency.
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

- Production `merman-core/src` now has no `regex::Regex`, `Regex::new`, or `OnceLock<Regex>`
  matches. Remaining `OnceLock` use is for generated/default config, family ID lists, theme data,
  or static sanitizer allowlist sets rather than regex compilation.
- A small number of `unwrap/expect` in renderer internals:
  - most are on index/iterator operations that are guarded by bounds checks, but they are worth
    auditing because they can become input-reachable if assumptions drift.
- `dugong-graphlib::Graph::set_edge_named(...)` still panics for named edges on non-multigraph
  simple graphs. That is currently preserved as a source-backed Graphlib throw mapping, not treated
  as an internal invariant panic.
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
