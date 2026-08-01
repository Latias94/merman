# Mermaid LSP Capability Matrix

This matrix records the current product readiness bar for Mermaid families and editor features.
It is intentionally conservative: only parser-backed facts count as mature body semantics.

This table is the explicit maturity contract for 35 admitted public diagram types. Every public
type in the current release is admitted, but the LSP contract remains independent from the render
catalog: a future type returned by `merman_core::supported_diagrams()` requires its own editor-evidence
review before joining this matrix. The lower-level family catalog also contains syntax aliases,
layout variants, and the internal error fallback. Aliases and layout variants inherit their public
product row instead of creating a second LSP maturity claim.

## Ownership Boundary

`merman-lsp` is a protocol projection over `merman-analysis` and `merman-editor-core`, not a
separate lint engine or preview product. Analysis owns diagnostics, rule metadata, source/fence
mapping, and internal projection failures. Editor-core owns protocol-neutral completion, hover,
symbols, navigation, rename, folding, and semantic-token facts. LSP owns request lifecycle,
capability advertising, URI/range conversion, token delta encoding, and client cache state.

Semantic-token codes, modifier bits, legend indices, and the five-word relative UTF-16 record are
owned by `editor-language/token-descriptor-v1.json` and generated into Rust, Web, and the VS Code
extension. LSP initialization publishes the descriptor digest and packed encoding under
`capabilities.experimental.merman.editorLanguage`; the extension fails closed when that identity or
the standard LSP legend differs. The same capability publishes the descriptor-owned rename-policy
list, which the extension also validates exactly before enabling language intelligence. The same
descriptor projects custom VS Code token declarations,
theme supertypes, source-owned TextMate fallback scopes, and Mermaid semantic-highlighting defaults;
standard VS Code types and modifiers are not redeclared. Editor-only theme metadata is excluded from
the packed-protocol digest and guarded by the generated manifest drift check instead, so a scope or
description change cannot create a false LSP/WASM incompatibility.
`editor-language/token-equivalence-v1.json` records the exact planner output for the 35-public-type
baseline plus malformed recovery, and LSP, Web WASM, Monaco, and VS Code gates consume that one
generated evidence artifact without transport-local sorting or token name lookup.

The private `LanguageSession` keeps weighted lazy editor generations built from the same active
analyzer environment used for diagnostics. Diagnostic-only lint rule changes reproject diagnostics
without invalidating editor snapshots or semantic-token result state. Snapshot-affecting changes
such as site config, fixed date/time, resource limits, or source descriptor changes clear
snapshot-dependent state.

Typed session operations capture document/configuration generations before running projection work
outside the session mutex. Semantic-token responses only commit cached token state while the captured
snapshot is still current; stale previous result ids fall back to full tokens after snapshot-affecting
configuration changes. Push diagnostics re-check currentness immediately before publishing and
suppress contexts that are already stale. Pull diagnostics use a bounded retry loop, recomputing
from the latest context up to three times when stale analyzer output is detected. This is a bounded
LSP adapter contract, not a cancellation framework for notifications already handed to the client
transport.

External lint and preview tools can integrate with Merman analysis, coexist beside it, or ignore it.
Merman language intelligence does not require a host to replace VS Code built-in Mermaid preview,
third-party preview extensions, markdownlint/remark/textlint rules, or `mermaid-lint`-style CI
policy.

## Typed Snapshot and Analysis Facts Wire Contracts

LSP language behavior is projected from typed `DocumentSnapshot` / `FenceTextIndex` data built
directly from `AnalysisGeneration`; the server does not round-trip serialized facts JSON. The separately
exposed `AnalysisFactsPayload` version 1 is the equivalent parser-only wire contract for binding
consumers:

- `fact_source: "text_scan"` is removed;
- `fact_source: "unavailable"` means that no body semantic facts were produced;
- every semantic item emitted by current writers has a family-owned `rename_policy`; and
- parser-backed and recovered provenance remain explicit; parser-backed facts always carry exact
  original-source spans. The compatibility field `source_mapped_spans` is `true` for those facts
  and `false` only when the body fact source is unavailable.

The TextScan-capable prerelease shape is deleted rather than retained behind a decoder, executor, alias,
or dual projection path. The final parser facts contract is schema 1 and rejects every other
version discriminator at the boundary. The diagnostics-only `AnalysisPayload` is a separate
contract and independently remains version 1.

These schema versions do not rename Mermaid grammar ids such as `flowchart-v2`,
`stateDiagram-v2`, or `classDiagram-v2`. They are also unrelated to LSP
`textDocument.version` document revisions and to FFI, WASM, UniFFI, or platform ABI versions.

For supported rows below, completion and refactoring still use complete or tested recovered parser
facts. The intentional behavior change is on unavailable input: unknown, unsupported, or
unrecoverable body text no longer receives guessed node ids, symbols, references, rename edits, or
semantic tokens. Legal source-start header and template completion remains catalog-backed.

## Family Coverage

Rows follow the catalog-owned public diagram-type order. `Yes` means the feature is correctly wired
to parser-backed facts, including returning no target when a valid position has no applicable
entity. It does not mean that every family grammar exposes renameable entities at every position.

| Family | Public diagram type | Parser-backed facts | Recoverable input | Completion | Hover / Symbols | Semantic Tokens | Definition / References / Rename | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Architecture | `architecture` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for groups, services, junctions, edges, and accessibility/title payloads. |
| Block | `block` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for block ids, nested composites, edges, class/style targets, arrow directions, and role-separated payload spans. |
| C4 | `c4` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for C4 aliases, boundaries, relations, style/update targets, layout values, and role-separated title/accessibility/payload spans. |
| Class | `class` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for class ids, members, annotations, directives, and style payload roles. |
| Cynefin | `cynefin` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for domain outlines, transitions, titles, and source-mapped payloads; the current grammar intentionally exposes no addressable rename group. |
| ER | `er` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for entities, relationships, attributes, and directive payload roles. |
| Event Modeling | `eventmodeling` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for timeline entities, time frames, and event payloads. |
| Flowchart | `flowchart` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node ids, subgraphs, directive prefixes, payload roles, and parser-backed authoring hints when enabled. |
| Gantt | `gantt` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for task ids, dependency refs, click targets, section outlines, directives, and accessibility payloads. |
| GitGraph | `gitgraph` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for commits, branches, merges, cherry-picks, and accessibility/title payloads. |
| Info | `info` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for free-form metadata payloads and directive prefixes. |
| Ishikawa | `ishikawa` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for effect/cause ids, outline entries, and parser-backed payload spans. |
| Journey | `journey` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for section outlines, task rows, scores, and actor payloads. |
| Kanban | `kanban` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for sections, items, icons, classes, and role-separated payloads. |
| Mindmap | `mindmap` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node ids, explicit labels, directives, and role-separated payloads. |
| Packet | `packet` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for title, accessibility text, and bit-field payloads. |
| Pie | `pie` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for title and slice payloads. |
| Quadrant Chart | `quadrantchart` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for quadrant labels, axes, and point payloads. |
| Radar | `radar` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for axes, curves, options, and accessibility/title payloads. |
| Railroad IR | `railroad` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for rule definitions, nonterminal references, expression constructors, titles, comments, and IR-specific rename validation. |
| Railroad ABNF | `railroadAbnf` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for ABNF rules, references, repetitions, alternatives, comments, and ABNF-specific rename validation. |
| Railroad EBNF | `railroadEbnf` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for EBNF rules, references, choices, optional/repeated terms, comments, and EBNF-specific rename validation. |
| Railroad PEG | `railroadPeg` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for PEG rules, references, predicates, suffix operators, comments, and PEG-specific rename validation. |
| Requirement | `requirement` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for requirements, elements, relationships, and traced payloads. |
| Sankey | `sankey` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node and link payloads. |
| Sequence | `sequence` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for participants, actors, message endpoints, notes, boxes, directive payloads, and interaction payload prefixes. |
| State | `state` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for state ids, references, outlines, and role-aware payloads. |
| Swimlane | `swimlane` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for lane/subgraph structure, node ids, edges, payload roles, and the independent Swimlane layout/config identity. |
| Timeline | `timeline` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for titles, accessibility text, section outlines, and event payloads. |
| Tree View | `treeView` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for tree node ids, labels, and structural outline roles. |
| Treemap | `treemap` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for sections, leaves, class defs, values, and accessibility/title payloads. |
| Venn | `venn` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for set ids, unions, text nodes, and styling payloads. |
| Wardley | `wardley` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for anchors, components, pipelines, links, evolution references, notes, and source-mapped coordinates. |
| XY Chart | `xychart` | Yes | Yes | Yes | Yes | Yes | Yes | Mature for titles, axes, and series payloads. |
| ZenUML | `zenuml` | Yes | Yes | Yes | Yes | Yes | Yes | Grammar-derived family facts cover source-mapped participants, groups, messages, creation, calls, assignments, returns, references, fragments, titles, and payload spans. |

## Coverage Boundary

The matrix above explicitly admits all 35 public types in the current release. Lower-level catalog
ids such as `flowchart-v2`, `flowchart-elk`, `stateDiagram`, and `classDiagram` inherit the matching
public product row. Source headers such as `stateDiagram-v2` and `classDiagram-v2` map to the latter
two catalog ids; they are not separate product types. The following logical family is the only
catalog entry outside the first-class LSP product contract:

| Family | Status | Why |
| --- | --- | --- |
| Error | Internal only | Fallback diagram only; not a product-family commitment. |

Payload- or outline-first first-class families deserve a separate note: Cynefin, Info, Pie, Packet,
and XY Chart are intentionally sparse on rename/reference targets. They still belong in the
first-class contract because completion, hover, diagnostics, and semantic indexing are wired, but
the family itself does not expose many entity-bearing spans.

## Semantic Fact Provenance

Editor features are backed by `merman-editor-core` query results. Those results expose
`FenceTextIndexSource` provenance so callers can distinguish parser facts from explicit
unavailability:

| Provenance | Meaning | Product status |
| --- | --- | --- |
| `ParserComplete` | Semantic facts came from a successful family parser/editor-facts path. | Mature when covered by the family row and editor-core tests. |
| `ParserRecovered` | Semantic facts came from parser recovery after an incomplete or invalid edit buffer. | Mature for incomplete-buffer editing when tests cover the family and feature. |
| `Unavailable` | No parser-backed body facts are available. | No body completion, hover, symbols, navigation, rename, or semantic tokens are projected. |

Preprocessing owns a composable edit map. Facts are mapped independently through it, so an
unrepresentable span is omitted only for that fact and produces a recovery diagnostic; unrelated
facts retain their exact original-source coordinates. There is no parser-input coordinate mode or
whole-document degraded fallback.

The matrix above requires parser-backed complete or recovered provenance for first-class feature
claims. Source-start headers and templates come directly from the static Diagram Family catalog;
they remain available without constructing or claiming body semantics.

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

- Diagnostics: analysis and lint diagnostics come only from shared `merman-analysis` payloads. Core
  parser errors carry structured metadata when the family can prove an exact token span or insertion
  point. Analysis owns merge and fallback policy: recovered parser facts may improve the primary
  span, but matching recovery errors must not create a duplicate user-visible diagnostic.
  Whole-source spans are reserved for source-wide conditions such as no diagram, unsupported family,
  resource limits, or genuinely unlocatable parser failures. The sole LSP-owned exception is
  `merman.lsp.document_sync_lost`, a protocol-integrity diagnostic emitted when an invalid
  incremental edit or a ranged edit after source discard leaves no authoritative server text; it is
  not an analysis rule or rule-catalog entry.
- LSP diagnostic projection: `Diagnostic.source` is `merman`; the visible `Diagnostic.code` is the
  stable string rule id such as `merman.parse.diagram_parse`, not the numeric analysis status.
  When diagnostic data is negotiated, it contains only the diagnostic id and current document
  version used to validate a code-action request; fix plans and auxiliary rule metadata stay
  server-owned. Editor-core and LSP do not keep a number-or-string compatibility enum and do not
  deduplicate projected diagnostics; they preserve analysis payload cardinality. Document pull diagnostics are enabled only when the client
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
  projection, and same-name entities with different semantic kinds do not collide. Rename
  validation uses the parser-owned policy carried by each entity, including qualified names and
  Event Modeling frame ids; the LSP adapter does not impose a second identifier grammar.
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
  `flowchart.htmlLabels` without an automatic quickfix, while
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
  advertised and clear cached token state. Diagnostic-only lint configuration changes refresh
  diagnostics without invalidating semantic-token state. Delta requests reuse cached previous token
  state only when the result id matches state from the current snapshot generation; otherwise they
  return full tokens.
- Unavailable facts: source-start headers/templates are catalog-backed, while unknown or
  unsupported body text produces no semantic items. Parser-backed payload facts remain outside
  completion IDs and outline entries unless their role explicitly permits it.
- Flowchart lint: parser-backed warning facts flow through the shared analysis contract, starting
  with a recommended-profile authoring hint and preferred quickfix for flowchart headers that omit
  an explicit direction, plus a core compatibility warning for `style` targets that would
  auto-create unknown nodes.
