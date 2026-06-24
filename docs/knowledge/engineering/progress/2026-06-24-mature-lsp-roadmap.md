---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

The active long-term goal has moved from a parser/semantic seam slice to a product-grade Mermaid
LSP and lint roadmap. The existing LSP completion plan and parser-semantic seam plan remain the
foundation; the new umbrella plan defines the remaining maturity work across semantic facts, lint
rules, LSP protocol features, configuration, packaging, and release gates.

# Details

- `merman-lsp` already covers diagnostics, completion, hover, document symbols, definition,
  references, prepare-rename, rename, full-document semantic tokens, and fix-backed quickfix code
  actions.
- `merman-analysis` already owns canonical diagnostics payloads, Markdown fence analysis, LSP range
  conversion helpers, and CLI lint output.
- Parser-backed editor facts now cover the main current LSP families with complete/recovered
  provenance: flowchart, sequence, state, class, ER, mindmap, and Gantt.
- The next architecture break is no longer just one family payload slice. The roadmap starts by
  making family capability status explicit, then deepens remaining parser facts, then evolves
  `FenceTextIndex` into a richer semantic index.
- External product references helped set the target capability bar: `mermaid-lint` shows a
  diagnostics/lint surface with Mermaid compatibility awareness, while Mermaid Studio Core shows an
  IDE-level feature set including completion, validation, refactoring, usages, highlighting, visual
  workflows, and MCP-oriented integration.
- U1 is complete: `docs/lsp/CAPABILITIES.md` now records the current maturity bar, and
  `crates/merman-lsp/tests/capabilities.rs` proves the parser-backed family matrix against the live
  `DocumentStore`.
- U2 has started: `FenceTextIndex::from_text` still exists as a migration fallback, but it no
  longer projects payload-only directive lines such as `click`, `linkStyle`, `accTitle`,
  `accDescr`, or `title` into node IDs or outline entries. It only records their directive prefixes.
- U2 has also deepened sequence editor facts: sequence `title`, `accTitle`, `accDescr`, message
  text, note text, and `links`/`link`/`properties`/`details` interaction payloads are now
  parser-backed payload-only spans, with directive prefix tracking for the sequence interaction
  statements and LSP regressions proving those payloads do not enter completion IDs or outline
  items.
- U3 has started with a small but important index break: `FenceTextIndex` now retains
  parser-backed `FenceSemanticItem` records with entity/outline/payload roles instead of dropping
  payload facts after deriving completion IDs and outline entries. Existing LSP projections still
  read the role-filtered views, while future lint, semantic-token, and code-action work can inspect
  payload spans without adding transport-local parsing.
- LSP hover has started consuming the semantic-item query path: payload facts such as sequence
  titles can now produce hover content directly from parser-backed payload spans, while completion,
  outline, references, and rename remain role-filtered.
- Definition, references, prepare-rename, and rename now use an entity-only semantic-item query.
  Payload items can feed hover and future lint/semantic-token/code-action consumers, but are not
  navigation or rename targets.
- References and rename now resolve through typed `FenceReferenceGroup` keys based on symbol name
  plus `EditorSymbolKind`, so same-name entities with different semantic kinds no longer collide in
  LSP navigation or edits.
- U6 has its first provider slice: `textDocument/semanticTokens/full` is advertised and served from
  parser-backed `FenceSemanticItem` roles. Token types derive from `EditorSymbolKind`, while
  `mermanEntity`, `mermanOutline`, and `mermanPayload` modifiers preserve the role category.
- Semantic-token coverage includes role classification, Markdown absolute UTF-16 ranges, multiline
  payload splitting, initialize capability wiring, and the full-document handler. Range/delta
  semantic tokens remain future work.
- `AnalysisDiagnostic` now carries optional `DiagnosticFix` metadata. LSP diagnostics preserve
  fixes in `Diagnostic.data`, and `textDocument/codeAction` returns quickfix edits only for
  diagnostics with explicit source-span-backed fixes.
- The first fix-backed lint rule is `merman.config.prefer_init_directive`, which reports the
  Mermaid directive alias `initialize` and offers a preferred source edit to replace it with
  canonical `init`. Markdown fences remap the fix edit back into host-document coordinates.
- `merman-analysis` now exposes stable rule descriptors plus a shared rule-config surface, and
  CLI lint can disable rules or override severities through the same analysis config.
- That shared rule-config surface now also flows through binding `options_json`, so FFI,
  UniFFI, WASM, and future editor adapters can enable/disable rules and override severities
  through the same analysis contract.

# Next Action

Continue U4 by adding more stable lint rule descriptors and source-span-backed `DiagnosticFix`
rules, then continue U2 family fact deepening where the capability matrix still shows partial
rename/lint readiness.

# Citations

- [Mature Mermaid LSP roadmap plan](../../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md)
- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Parser and semantic seam plan](../../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Editor parser/semantic seam ADR](../../../adr/0071-editor-parser-semantic-seam.md)
- [merman-analysis crate](../../../../crates/merman-analysis/src/lib.rs)
- [merman-lsp README](../../../../crates/merman-lsp/README.md)
- [mermaid-lint blog](https://jasonworden.com/blog/introducing-mermaid-lint/)
- [Mermaid Studio Core plugin](https://plugins.jetbrains.com/plugin/30883-mermaid-studio-core)
