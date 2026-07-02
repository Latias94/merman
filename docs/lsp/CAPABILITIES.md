# Mermaid LSP Capability Matrix

This matrix records the current product readiness bar for Mermaid families and editor features.
It is intentionally conservative: if a capability depends on text scanning instead of parser-backed
facts, it is not considered mature.

This table is the maturity contract for first-class LSP families. The parser and render registries
also include additional diagram types, but they are not treated as mature LSP commitments unless
they appear here.

Families outside this table can still be parser-backed in the core engine. That is useful for
rendering and compatibility, but it is not enough to count as a mature editor contract.

## Ownership Boundary

`merman-lsp` is a protocol projection over `merman-analysis` and `merman-editor-core`, not a
separate lint engine or preview product. Analysis owns diagnostics, rule metadata, source/fence
mapping, and internal projection failures. Editor-core owns protocol-neutral completion, hover,
symbols, navigation, rename, folding, and semantic-token facts. LSP owns request lifecycle,
capability advertising, URI/range conversion, token delta encoding, and client cache state.

The LSP document store keeps lazy editor snapshots, but those snapshots are built from the same
active analyzer configuration used for diagnostics. Diagnostic-only lint rule changes refresh
diagnostics without invalidating editor snapshots or semantic-token result state. Snapshot-affecting
changes such as parse options, site config, fixed date/time, resource limits, or source descriptor
changes clear snapshot-dependent state.

External lint and preview tools can integrate with Merman analysis, coexist beside it, or ignore it.
Merman language intelligence does not require a host to replace VS Code built-in Mermaid preview,
third-party preview extensions, markdownlint/remark/textlint rules, or `mermaid-lint`-style CI
policy.

## Family Coverage

| Family | Parser-backed facts | Recoverable input | Completion | Hover / Symbols | Semantic Tokens | Definition / References / Rename | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Flowchart | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node ids, subgraphs, directive prefixes, payload roles, and parser-backed authoring hints when enabled. |
| Sequence | Yes | Yes | Yes | Yes | Yes | Yes | Mature for participants, actors, message endpoints, notes, boxes, directive payloads, and interaction payload prefixes. |
| State | Yes | Yes | Yes | Yes | Yes | Yes | Mature for state ids, references, outlines, and role-aware payloads. |
| Class | Yes | Yes | Yes | Yes | Yes | Yes | Mature for class ids, members, annotations, directives, and style payload roles. |
| ER | Yes | Yes | Yes | Yes | Yes | Yes | Mature for entities, relationships, attributes, and directive payload roles. |
| Mindmap | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node ids, explicit labels, directives, and role-separated payloads. |
| Gantt | Yes | Yes | Yes | Yes | Yes | Yes | Mature for task ids, dependency refs, click targets, section outlines, directives, and accessibility payloads. |
| Architecture | Yes | Yes | Yes | Yes | Yes | Yes | Mature for groups, services, junctions, edges, and accessibility/title payloads. |
| GitGraph | Yes | Yes | Yes | Yes | Yes | Yes | Mature for commits, branches, merges, cherry-picks, and accessibility/title payloads. |
| Kanban | Yes | Yes | Yes | Yes | Yes | Yes | Mature for sections, items, icons, classes, and role-separated payloads. |
| Radar | Yes | Yes | Yes | Yes | Yes | Yes | Mature for axes, curves, options, and accessibility/title payloads. |
| Treemap | Yes | Yes | Yes | Yes | Yes | Yes | Mature for sections, leaves, class defs, values, and accessibility/title payloads. |
| Block | Yes | Yes | Yes | Yes | Yes | Yes | Mature for block ids, nested composites, edges, class/style targets, arrow directions, and role-separated payload spans. |
| C4 | Yes | Yes | Yes | Yes | Yes | Yes | Mature for C4 aliases, boundaries, relations, style/update targets, layout values, and role-separated title/accessibility/payload spans. |
| ZenUML | Yes | Yes | Yes | Yes | Yes | Yes | Mature for the supported headless ZenUML subset, with source-mapped participants, messages, calls, assignments, titles, and payload spans. |
| Journey | Yes | Yes | Yes | Yes | Yes | Yes | Mature for section outlines, task rows, scores, and actor payloads. |
| Info | Yes | Yes | Yes | Yes | Yes | Yes | Mature for free-form metadata payloads and directive prefixes. |
| Timeline | Yes | Yes | Yes | Yes | Yes | Yes | Mature for titles, accessibility text, section outlines, and event payloads. |
| Pie | Yes | Yes | Yes | Yes | Yes | Yes | Mature for title and slice payloads. |
| Packet | Yes | Yes | Yes | Yes | Yes | Yes | Mature for title, accessibility text, and bit-field payloads. |
| Sankey | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node and link payloads. |
| Tree View | Yes | Yes | Yes | Yes | Yes | Yes | Mature for tree node ids, labels, and structural outline roles. |
| Ishikawa | Yes | Yes | Yes | Yes | Yes | Yes | Mature for effect/cause ids, outline entries, and parser-backed payload spans. |
| Event Modeling | Yes | Yes | Yes | Yes | Yes | Yes | Mature for timeline entities, time frames, and event payloads. |
| Quadrant Chart | Yes | Yes | Yes | Yes | Yes | Yes | Mature for quadrant labels, axes, and point payloads. |
| Requirement | Yes | Yes | Yes | Yes | Yes | Yes | Mature for requirements, elements, relationships, and traced payloads. |
| Venn | Yes | Yes | Yes | Yes | Yes | Yes | Mature for set ids, unions, text nodes, and styling payloads. |
| XY Chart | Yes | Yes | Yes | Yes | Yes | Yes | Mature for titles, axes, and series payloads. |

## Coverage Boundary

The matrix above is intentionally narrower than the full parser/render registry. The following
entries are still outside the first-class LSP product-family contract:

| Family | Status | Why |
| --- | --- | --- |
| Error | Internal only | Fallback diagram only; not a product-family commitment. |

Payload-first first-class families deserve a separate note: Info, Pie, Packet, and XY Chart are
intentionally sparse on rename/reference targets. They still belong in the first-class contract
because completion, hover, diagnostics, and semantic indexing are wired, but the family itself
does not expose many entity-bearing spans.

## Semantic Fact Provenance

Editor features are backed by `merman-editor-core` query results. Those results expose
`FenceTextIndexSource` provenance so callers can distinguish first-class parser facts from fallback
behavior:

| Provenance | Meaning | Product status |
| --- | --- | --- |
| `ParserComplete` | Semantic facts came from a successful family parser/editor-facts path. | Mature when covered by the family row and editor-core tests. |
| `ParserRecovered` | Semantic facts came from parser recovery after an incomplete or invalid edit buffer. | Mature for incomplete-buffer editing when tests cover the family and feature. |
| `TextScan` | Semantic facts came from the bounded text-scan fallback. | Fallback only; not a mature family capability and must stay visible to callers. |

The matrix above requires parser-backed complete or recovered provenance for first-class feature
claims. Text-scan fallback may still support source-start headers/templates and conservative
directive prefixes, but it must not be counted as mature body completion, navigation, rename, or
semantic-token support.

## Parser Diagnostic Span Coverage

Core parser diagnostics use explicit span classes before they reach analysis:

- Exact spans underline parser-known invalid tokens, directive values, or arguments.
- Insertion points mark missing syntax at a parser-known byte offset.
- Fallback spans are visible parser capability gaps. Analysis attaches fallback related information
  instead of silently projecting line-zero or unlabelled whole-source ranges.

Current parser-family diagnostic span matrix:

| Family / Parser Path | Span Support | Coverage Evidence | Remaining Gap |
| --- | --- | --- | --- |
| LALRPOP-backed parse wrappers | Exact token spans and EOF insertion points | `lalrpop_parse_diagnostic_preserves_token_span`, `lalrpop_parse_diagnostic_preserves_eof_insertion_point` | User errors are explicit fallbacks. |
| XY Chart | Exact invalid plot values; insertion points for missing plot syntax | `xychart_invalid_plot_number_reports_exact_token_span`, `xychart_comment_after_plot_does_not_merge_next_statement` | Broader render-validation failures still use named fallback helpers. |
| Gantt | Exact directive-value spans for `weekday` / `weekend` validation | `gantt_weekday_rejects_unknown_values` | Date parsing and cross-statement semantic failures still use visible fallbacks unless their parser helper preserves a narrower span. |
| GitGraph | Exact unknown command-token spans | `gitgraph_unknown_command_reports_exact_command_span` | Deeper repository-state semantic failures remain fallback diagnostics. |
| Timeline | Insertion points for missing event text separators | `timeline_event_missing_space_reports_insertion_point` | Generic section/title validation still uses fallback constructors where no token span is preserved. |
| C4 | Insertion points for missing relation/style macro arguments | `c4_missing_relation_target_reports_local_insertion_point`, `c4_missing_relation_style_target_reports_local_insertion_point` | Other render parser validation remains fallback until spanned macro validation covers it. |
| Architecture | Insertion points for missing ids/ports; exact spans for invalid directions, trailing statement tokens, duplicate ids, unknown parents, and unknown edge endpoints | `architecture_invalid_service_id_reports_insertion_point`, `architecture_invalid_edge_direction_reports_exact_token_span`, `architecture_duplicate_service_reports_exact_id_span`, `architecture_unknown_parent_reports_exact_reference_span`, `architecture_unknown_edge_endpoint_reports_exact_reference_span` | Some deeper group-boundary semantic validation is only exact where the offending edge endpoint is preserved. |
| Kanban | Insertion points for unterminated node/metadata syntax; exact spans for trailing node input and invalid metadata blocks | `kanban_unterminated_node_delimiter_reports_insertion_point`, `kanban_trailing_node_input_reports_exact_span`, `kanban_unterminated_metadata_reports_eof_insertion_point`, `kanban_invalid_shape_metadata_reports_exact_metadata_span` | Inline metadata fields are reported at the metadata block span until the inline-object parser exposes field-level spans. |

Remaining fallback ledger:

- Architecture render parse errors use exact or insertion-point spans for local line syntax and for
  semantic checks that carry declaration/reference spans into the DB. Remaining fallback use should
  be treated as a parser capability gap, not a message-scraping opportunity.
- Kanban render parse errors use exact or insertion-point spans for local node syntax, metadata
  blocks, invalid metadata shapes, and hierarchy validation when the offending node span is known.
  Field-level metadata spans need inline-object parser support before they can graduate beyond the
  block span.

## Feature Gates

- Diagnostics: shared `merman-analysis` payloads only. Core parser errors carry structured
  metadata when the family can prove an exact token span or insertion point. Analysis owns merge
  and fallback policy: recovered parser facts may improve the primary span, but matching recovery
  errors must not create a duplicate user-visible diagnostic. Whole-source spans are reserved for
  source-wide conditions such as no diagram, unsupported family, resource limits, or genuinely
  unlocatable parser failures.
- LSP diagnostic projection: `Diagnostic.source` is `merman`; the visible `Diagnostic.code` is the
  stable string rule id such as `merman.parse.diagram_parse`, not the numeric analysis status.
  Numeric `code` / `code_name`, category, diagram type, help text, and fix metadata remain in
  diagnostic `data` for compatibility and code actions. Editor-core and LSP do not keep a
  number-or-string compatibility enum and do not deduplicate projected diagnostics; they preserve
  analysis payload cardinality. Document pull diagnostics are enabled only when the client
  advertises `textDocument.diagnostic`; `workspace_diagnostics` is not advertised and
  `workspace/diagnostic` is not implemented because unopened workspace-file scanning is not
  implemented. Push diagnostics are cleared on `didClose`, and `workspace/diagnostic/refresh` is
  sent only to invalidate pull diagnostic caches when the client advertises
  `workspace.diagnostic.refreshSupport`.
- Lint rule discovery: clients should use the shared rule catalog metadata for rule ids,
  evidence references, profiles, origins, configurability, and fixability instead of duplicating
  LSP-local rule tables. The server advertises `merman/ruleCatalog` under
  `ServerCapabilities.experimental.merman.requests`.
- Configuration discovery: clients should use `merman/configSchema` for editor settings completion,
  validation hints, available lint profiles, diagnostic severities, configurable rule-id enums, and
  the accepted direct/`merman`/`analysis` settings roots. The schema describes the same analysis
  options accepted by initialization options and `workspace/didChangeConfiguration`.
- Completion: availability is decided before item projection. Diagram headers and static diagram
  templates are offered only at legal document or fence starts. Semantic roles must exclude
  payload-only spans. Parser expected-syntax spans and parser-backed directive slots may expose
  direction, shape, operator, node identifier, class name, style, interaction, frontmatter config,
  and `themeCSS` completions. Semantic target reuse such as node identifiers and class names stays
  plain text. Unsupported body positions and parser-controlled payload spans intentionally return
  no items instead of generic diagram headers or broad node-id guesses.
- Completion resolve: completion items carry Merman-owned `data`, and `completionItem/resolve`
  fills Markdown documentation without changing `insertText`, `textEdit`, filtering, or sorting
  fields.
- Definition / References / Rename: entity-only semantic item queries keyed by typed reference
  groups. Payload and outline-only items are excluded unless a future role explicitly allows
  projection, and same-name entities with different semantic kinds do not collide.
- Code actions: quickfix provider is wired; only diagnostics with `DiagnosticFix` metadata are
  eligible, and diagnostics without explicit safe fixes produce no action. Recommended-profile
  authoring rules include `merman.authoring.config.prefer_init_directive`,
  `merman.authoring.config.prefer_frontmatter_config`, and the parser-backed
  `merman.authoring.flowchart.explicit_direction` insertion fix when the `recommended` lint
  profile or explicit rule enablement is active. The frontmatter-config rule carries a
  migration quickfix that rewrites init/initialize directive config into YAML frontmatter.
- VS Code source actions: Mermaid files and Markdown/MDX Mermaid fences expose low-noise
  source-scoped CodeLens actions for `Preview` and `Export / Copy`. The action target carries the
  stable source id for Markdown fences, so cursor movement after the CodeLens is created does not
  retarget the operation. Export and copy commands remain available through `Export / Copy`, the
  editor/context commands, and preview output controls. These actions are local-only and do not
  include AI, account, sync, pin, or remote-rendering controls.
- VS Code preview diagnostics: Problems, editor underlines, hover, and the VS Code quick-fix
  lightbulb own detailed diagnostics and fixes. The preview shows only a compact diagnostic status
  for the active source and can navigate to the first diagnostic; it does not render a second
  Problems list or per-diagnostic quick-fix buttons.
- Config lint: Mermaid-backed compatibility warnings can be enabled in the core profile when
  upstream emits or documents the same warning.
  `merman.compatibility.config.deprecated_flowchart_html_labels` reports deprecated
  `flowchart.htmlLabels` and carries a preferred quickfix to move it to root `htmlLabels`, while
  `merman.compatibility.config.deprecated_external_diagram_loading` reports deprecated
  `lazyLoadedDiagrams` / `loadExternalDiagramsAtStartup` directive config. Both intentionally
  remain source-backed compatibility warnings.
- Semantic index: parser-backed payload facts are retained as semantic items even when they are
  not projected into completion, outline, or rename surfaces.
- Semantic tokens: the full-document, range, and delta providers are wired from parser-backed
  entity/outline/payload semantic items. Token types derive from `EditorSymbolKind`; token
  modifiers preserve role categories. The LSP semantic-token legend is derived from the editor-core
  legend so token ordering stays tied to the protocol-neutral semantic contract. Snapshot-affecting
  configuration changes ask the client to refresh semantic tokens when refresh support is
  advertised, and delta requests reuse cached previous token state when the result id matches.
- Text-scan fallback: may support source-start headers/templates and record directive prefixes for
  unmigrated paths, but must not assert body completion availability. It must not project
  payload-only directive lines such as `click`, `linkStyle`, `accTitle`, `accDescr`, or `title`
  into node IDs, completion IDs, or outline entries. Parser-backed payload facts must likewise
  remain outside completion IDs and outline entries unless their role explicitly permits it.
- Flowchart lint: parser-backed warning facts flow through the shared analysis contract, starting
  with a recommended-profile authoring hint and preferred quickfix for flowchart headers that omit
  an explicit direction, plus a core compatibility warning for `style` targets that would
  auto-create unknown nodes.
