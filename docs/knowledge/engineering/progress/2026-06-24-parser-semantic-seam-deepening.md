---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

The next fearless-refactor slice targets the parser/semantic seam. Parser technology stays
family-local, but editor-facing consumers now move toward span-rich semantic facts and recoverable
partial parse results instead of raw-text heuristic scans.

# Details

- The current `merman-lsp` baseline remains intact.
- `merman-analysis::FenceTextIndex` is now the shared migration seam for editor-facing fence
  structure. LSP no longer owns separate completion, outline, navigation, and rename scans.
- The LSP snapshot layer now stores the shared index and projects it into protocol types; it does
  not define its own editor semantics.
- Heuristic fence-local structure scans are now a centralized migration shim, not the target design.
- The new plan keeps the parser choice where it already belongs: inside each family.
- A new ADR records the seam so later lint and LSP work can use the same contract.
- `merman-core` now exposes `EditorSemanticFacts`, `EditorSemanticSymbol`,
  `EditorSemanticKind`, `EditorSemanticRole`, `EditorSemanticCompleteness`, and `SourceSpan` as
  the parser-backed editor semantic contract.
- `EditorSemanticRole` separates entity facts from outline-only and payload-only facts. This keeps
  deep parser spans available for lint/future consumers without turning every parser fact into a
  graph-node completion candidate.
- Flowchart is the first tracer bullet: its lexer/LALRPOP AST now preserves node id spans and
  subgraph header/selection spans, and editor fact extraction preserves original input byte
  offsets even when directives/frontmatter/comments/accessibility statements are masked away for
  parsing.
- Flowchart editor fact extraction now recovers from incomplete editor buffers: when full
  LALRPOP parsing fails, it uses the same masked input and lexer token stream to return
  `EditorSemanticCompleteness::Recovered` facts for already recognized node ids, subgraph headers,
  and directive prefixes.
- Sequence is the second migrated family: its lexer token stream now emits parser-backed
  participant, actor, message-endpoint, note-actor, and box facts, while the existing LALRPOP
  parser result determines whether facts are `Complete` or `Recovered`.
- State is the third migrated family: `StateStmt` now carries parser source spans, state grammar
  productions preserve spans for state ids, relation endpoints, nested states, note-bound states,
  and typed fork/join/choice states, and incomplete editor buffers recover symbols from the state
  lexer token stream rather than from free-form text scanning.
- Class is the fourth migrated family: editor facts now come from the class lexer token stream with
  LALRPOP complete/recovered provenance, covering class declarations, namespaces, relation
  endpoints, member owners, annotation targets, style/classDef/cssClass targets, and click/link/
  callback targets without returning to raw-text scans.
- Class member facts are now role-aware: class-body members and inline `Class: member` entries are
  outline-only symbols, while annotation names are payload-only spans. This preserves data needed
  by future lint/semantic consumers without adding members or annotations to node-id completion.
- ER is the fifth migrated family: `IdList` is now span-rich internally, editor facts cover
  entities, relationship endpoints, attribute names, inline classes, class/style/classDef targets,
  and incomplete ER buffers recover from the ER lexer token stream instead of raw-text scans.
- ER attribute names now project as outline-only facts, while attribute type, key, and comment
  tokens are preserved as payload-only span facts for future lint and semantic consumers.
- ER incomplete attribute blocks no longer make the lexer repeatedly emit the same EOF error; the
  lexer reports the block EOF once, exits block mode, and lets editor fact recovery finish.
- Mindmap is the first migrated hand-written family: its line parser now returns an internal event
  stream for nodes, class directives, and icon directives. The same events drive DB/render model
  construction and editor facts, so LSP/lint consumers get parser-backed node spans and recovered
  incomplete-delimiter facts without breaking class/icon decoration behavior.
- Gantt is the second migrated hand-written family: task ids, `after`/`until` dependency
  references, `click` targets, and directive prefixes now come from the Gantt statement parser
  rules with complete/recovered provenance. The relative-reference matcher now has a range
  projection so editor facts reuse the same Mermaid-backed dependency grammar as render semantics.
- Gantt section titles are now outline-only parser facts. They feed document-symbol/hover style
  structure through the shared index without becoming task-id completion candidates.
- Gantt keyword directive values for `title`, `dateFormat`, `axisFormat`, `tickInterval`,
  `includes`, `excludes`, `todayMarker`, `weekday`, and `weekend` are now payload-only parser
  spans. They are retained for future lint/semantic consumers but do not project into completion or
  outline.
- The existing Gantt click parser is now span-aware. Click task ids remain entity references, while
  `href` URLs, callback names, and callback args are payload-only parser spans for future lint and
  semantic consumers.
- Gantt editor completeness is tolerant of original-source YAML front matter and Mermaid init
  directives, so parser-backed facts preserve original byte spans without misclassifying valid
  editor buffers as recovered.
- Gantt `section` is deliberately recorded as a directive prefix, not as a node id. Future section
  document-symbol support should extend the semantic contract with role-aware or outline-only facts
  instead of adding section names to task-id completion.
- `merman-analysis::FenceTextIndex::from_core_facts` projects core editor facts into the shared
  LSP/lint migration index, including directive prefixes used by completion. It now respects
  semantic roles: entity facts feed completion/navigation/outline, outline facts feed outline only,
  and payload facts are intentionally not projected into LSP completion/navigation.
- `FenceTextIndex` now records whether its source is `TextScan`, `ParserComplete`, or
  `ParserRecovered`, giving tests and future lint/LSP logic a way to prove it did not silently
  return to heuristic scans.
- `merman-lsp::DocumentStore` now tries parser-backed core facts for known diagram types and falls
  back to the centralized text index only when parser-backed facts are unavailable or fail; LSP
  regressions cover flowchart, sequence, state, class, ER, mindmap, and gantt complete/recovered
  provenance.
- `merman-lsp` now exposes default `core-full` and `core-host` feature passthroughs so product LSP
  builds use the full detector/parser profile. This fixed a silent mindmap regression where LSP
  detection used the tiny registry and fell back to `TextScan`.
- `merman-lsp` now offers the `gantt` diagram header completion.
- Verified with `cargo fmt --all`, `cargo nextest run -p merman-core parse_flowchart_editor_facts`,
  `cargo nextest run -p merman-core parse_sequence_editor_facts`,
  `cargo nextest run -p merman-core parse_state_editor_facts`,
  `cargo nextest run -p merman-core parse_class_editor_facts`,
  `cargo nextest run -p merman-core parse_er_editor_facts`,
  `cargo nextest run -p merman-core state`, `cargo nextest run -p merman-core class`,
  `cargo nextest run -p merman-core er`, `cargo nextest run -p merman-core mindmap`,
  `cargo nextest run -p merman-core gantt`,
  `cargo nextest run -p merman-core gantt_editor_facts_preserve_parser_symbol_spans`,
  `cargo nextest run -p merman-core editor_facts`, `cargo nextest run -p merman-analysis`,
  and `cargo nextest run -p merman-lsp`.

# Next Action

Choose the next parser seam slice deliberately: deepen class/state/mindmap directive payload spans,
add Gantt accessibility payload spans for lint, or expose recovered parser diagnostics alongside
recovered facts. Do not add new heuristic parsing in LSP for covered
flowchart/sequence/state/class/ER/mindmap/gantt symbols; extend core facts instead.

# Citations

- [Parser and semantic seam plan](../../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Editor parser/semantic seam ADR](../../../adr/0071-editor-parser-semantic-seam.md)
- [editor index seam](../../../../crates/merman-analysis/src/editor.rs)
- [analysis LSP helpers](../../../../crates/merman-analysis/src/lsp.rs)
