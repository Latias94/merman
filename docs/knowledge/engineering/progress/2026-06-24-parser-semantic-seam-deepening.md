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
  `EditorSemanticKind`, and `SourceSpan` as the parser-backed editor semantic contract.
- Flowchart is the first tracer bullet: its lexer/LALRPOP AST now preserves node id spans and
  subgraph header/selection spans, and editor fact extraction preserves original input byte
  offsets even when directives/frontmatter/comments/accessibility statements are masked away for
  parsing.
- `merman-analysis::FenceTextIndex::from_core_facts` projects core editor facts into the shared
  LSP/lint migration index, including directive prefixes used by completion.
- `merman-lsp::DocumentStore` now tries parser-backed core facts for known diagram types and falls
  back to the centralized text index only when parser-backed facts are unavailable or fail.
- Verified with `cargo fmt --all`, `cargo nextest run -p merman-core parse_flowchart_editor_facts`,
  `cargo nextest run -p merman-analysis editor::tests`, and `cargo nextest run -p merman-lsp`.

# Next Action

Choose the next parser seam slice deliberately: either add recoverable partial editor facts for
incomplete flowchart buffers, or migrate the next high-value family (`sequence`, `state`, or
`class`) to `EditorSemanticFacts`. Do not add new heuristic parsing in LSP for covered flowchart
symbols; extend core facts instead.

# Citations

- [Parser and semantic seam plan](../../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Editor parser/semantic seam ADR](../../../adr/0071-editor-parser-semantic-seam.md)
- [editor index seam](../../../../crates/merman-analysis/src/editor.rs)
- [analysis LSP helpers](../../../../crates/merman-analysis/src/lsp.rs)
