# Mermaid LSP Capability Matrix

This matrix records the current product readiness bar for Mermaid families and editor features.
It is intentionally conservative: if a capability depends on text scanning instead of parser-backed
facts, it is not considered mature.

This table is the maturity contract for first-class LSP families. The parser and render registries
also include additional diagram types, but they are not treated as mature LSP commitments unless
they appear here.

Families outside this table can still be parser-backed in the core engine. That is useful for
rendering and compatibility, but it is not enough to count as a mature editor contract.

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

## Feature Gates

- Diagnostics: shared `merman-analysis` payloads only. Editor projections deduplicate identical
  diagnostics and humanize recovered parser messages so editor clients do not show raw parser
  internals such as recovery wrappers or token enum dumps.
- Lint rule discovery: clients should use the shared rule catalog metadata for rule ids,
  evidence references, profiles, origins, configurability, and fixability instead of duplicating
  LSP-local rule tables. The server advertises `merman/ruleCatalog` under
  `ServerCapabilities.experimental.merman.requests`.
- Configuration discovery: clients should use `merman/configSchema` for editor settings completion,
  validation hints, available lint profiles, diagnostic severities, configurable rule-id enums, and
  the accepted direct/`merman`/`analysis` settings roots. The schema describes the same analysis
  options accepted by initialization options and `workspace/didChangeConfiguration`.
- Completion: semantic roles must exclude payload-only spans. Static authoring templates and
  context-sensitive helper inserts may use LSP snippet completion items, while semantic target
  reuse such as node identifiers and class names stays plain text. Directive-aware completion
  offers node targets for `style`, `class`, `cssClass`, `click`, `link`, and `callback` target
  slots; class names for class-reference slots; and snippets for style properties, interaction
  actions, icon nodes, frontmatter config, and `themeCSS`.
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
  modifiers preserve role categories. Configuration changes ask the client to refresh semantic
  tokens when refresh support is advertised, and delta requests reuse cached previous token state
  when the result id matches.
- Text-scan fallback: may record directive prefixes for unmigrated paths, but must not project
  payload-only directive lines such as `click`, `linkStyle`, `accTitle`, `accDescr`, or `title`
  into node IDs or outline entries. Parser-backed payload facts must likewise remain outside
  completion IDs and outline entries unless their role explicitly permits it.
- Flowchart lint: parser-backed warning facts flow through the shared analysis contract, starting
  with a recommended-profile authoring hint and preferred quickfix for flowchart headers that omit
  an explicit direction, plus a core compatibility warning for `style` targets that would
  auto-create unknown nodes.
