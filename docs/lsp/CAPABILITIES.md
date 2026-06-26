# Mermaid LSP Capability Matrix

This matrix records the current product readiness bar for Mermaid families and editor features.
It is intentionally conservative: if a capability depends on text scanning instead of parser-backed
facts, it is not considered mature.

## Family Coverage

| Family | Parser-backed facts | Recoverable input | Completion | Hover / Symbols | Semantic Tokens | Definition / References / Rename | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Flowchart | Yes | Yes | Yes | Yes | Yes | Yes | Mature for node ids, subgraphs, directive prefixes, payload roles, and parser-backed authoring hints when enabled. |
| Sequence | Yes | Yes | Yes | Yes | Yes | Yes | Mature for participants, actors, message endpoints, notes, boxes, directive payloads, and interaction payload prefixes. |
| State | Yes | Yes | Yes | Yes | Yes | Yes | Mature for state ids, references, outlines, and role-aware payloads. |
| Class | Yes | Yes | Yes | Yes | Yes | Yes | Mature for class ids, members, annotations, directives, and style payload roles. |
| ER | Yes | Yes | Yes | Yes | Yes | Yes | Mature for entities, relationships, attributes, and directive payload roles. |
| Mindmap | Yes | Yes | Yes | Yes | Yes | Partial | Mature for node/event spans; rename and lint still need more payload depth. |
| Gantt | Yes | Yes | Yes | Yes | Yes | Partial | Mature for task ids, section outlines, directives, click payloads, and accessibility payloads. |

## Feature Gates

- Diagnostics: shared `merman-analysis` payloads only.
- Lint rule discovery: clients should use the shared rule catalog metadata for rule ids,
  evidence references, profiles, origins, configurability, and fixability instead of duplicating
  LSP-local rule tables. The server advertises `merman/ruleCatalog` under
  `ServerCapabilities.experimental.merman.requests`.
- Configuration discovery: clients should use `merman/configSchema` for editor settings completion,
  validation hints, available lint profiles, diagnostic severities, configurable rule-id enums, and
  the accepted direct/`merman`/`analysis` settings roots. The schema describes the same analysis
  options accepted by initialization options and `workspace/didChangeConfiguration`.
- Completion: semantic roles must exclude payload-only spans.
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
  an explicit direction.
