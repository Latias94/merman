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
  references, prepare-rename, rename, full-document semantic tokens, range/delta semantic tokens,
  and fix-backed quickfix code actions.
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
- U2's text-scan fallback now also treats `init`, `initialize`, and `wrap` as non-symbol
  directives, so those directive lines do not leak into node IDs or outline items when parser
  facts are unavailable.
- U2's completion context now also suppresses the generic `flowchart TD` fallback on directive
  lines, so directive-oriented lines remain on directive completions instead of being mistaken for
  a fresh diagram header prompt.
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
- Semantic-token coverage now includes role classification, Markdown absolute UTF-16 ranges,
  multiline payload splitting, initialize capability wiring, the full-document handler, range
  projection, delta projection, and cached result-id reuse.
- `AnalysisDiagnostic` now carries optional `DiagnosticFix` metadata. LSP diagnostics preserve
  fixes in `Diagnostic.data`, and `textDocument/codeAction` returns quickfix edits only for
  diagnostics with explicit source-span-backed fixes.
- Fix-backed authoring lint rules such as `merman.authoring.config.prefer_init_directive` and
  `merman.authoring.flowchart.explicit_direction` report Merman recommendations as hints only when
  the `recommended` profile or explicit rule enablement is active. Markdown fences remap fix edits
  back into host-document coordinates.
- `merman-analysis` now exposes stable rule descriptors plus origin metadata, lint profiles,
  explicit enable/disable, and severity overrides through the shared rule-config surface.
- That shared rule-config surface now also flows through binding `options_json`, so FFI,
  UniFFI, WASM, and future editor adapters can enable/disable rules and override severities
  through the same analysis contract.
- The shared rule-config surface now rejects unknown or internal rule ids at the JSON and CLI
  boundaries, and the analysis crate exposes a public configurable-rule registry view so future
  completion and lint clients can reuse the same public rule-id list.
- Unknown semantic warning fact ids no longer collapse into the generic semantic warning bucket;
  they now surface as explicit internal rule-registry gaps so missing core warning mappings are
  visible during lint and LSP development.

# Next Action

Continue U4 by adding more stable lint rule descriptors and source-span-backed `DiagnosticFix`
rules, then continue U2 family fact deepening where the capability matrix still shows partial
rename/lint readiness. Keep the public configurable-rule registry aligned with any future rule-id
completion surface.

# Citations

- [Mature Mermaid LSP roadmap plan](../../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md)
- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Parser and semantic seam plan](../../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Editor parser/semantic seam ADR](../../../adr/0071-editor-parser-semantic-seam.md)
- [merman-analysis crate](../../../../crates/merman-analysis/src/lib.rs)
- [merman-lsp README](../../../../crates/merman-lsp/README.md)
- [mermaid-lint blog](https://jasonworden.com/blog/introducing-mermaid-lint/)
- [Mermaid Studio Core plugin](https://plugins.jetbrains.com/plugin/30883-mermaid-studio-core)
