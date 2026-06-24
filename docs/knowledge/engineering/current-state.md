---
type: Current State
status: active
---

# Current State

- Goal: 完成 `docs/plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md` 对应的开发与无畏重构，把 `merman-core` 的解析/语义边界做成 LSP 与 lint 可直接消费的 span-rich seam，同时继续保留 family-local parser 选择，并在需要时做可回滚的增量提交，持续到 2026-06-24 10:00。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-24 (`cargo fmt --all`, `cargo nextest run -p merman-core class`, `cargo nextest run -p merman-analysis editor::tests`, `cargo nextest run -p merman-lsp`)
- Done: `merman-lsp` now exists as a dedicated crate; diagnostics are published from `merman-analysis`; Markdown fence diagnostics are remapped; plain Mermaid documents also get a snapshot fence; shared LSP mapping helpers now live in `merman-analysis`; snapshot now carries diagram type facts for each fence; completion now covers diagram headers, directions, operators, directives, shapes, and local node IDs with snapshot-derived replacement edits; `merman-analysis::document::analyze_document` now gives CLI lint and LSP one shared document-analysis seam; `server_smoke` proves initialize/open/change/save publish the current diagnostics version; `document_store` now validates both plain Mermaid and Markdown fence snapshot facts and proves newer versions replace older snapshots; hover/documentSymbol/definition/references/prepareRename/rename now consume the shared `merman-analysis::FenceTextIndex`; LSP snapshot no longer owns its own completion/structure scan; flowchart fences now prefer parser-backed `merman-core::EditorSemanticFacts` with original-text byte spans for node ids, subgraph headers, and directive prefixes; incomplete flowchart buffers now produce `EditorSemanticCompleteness::Recovered` facts from the same lexer token stream and LSP records `FenceTextIndexSource::ParserRecovered` instead of falling back to text scan; sequence fences now emit parser-backed actor/participant/message-endpoint/note/box facts with complete/recovered provenance; state fences now emit parser-backed state/reference/fork/join/choice facts from AST spans and recover incomplete buffers from the state lexer token stream; class fences now emit parser-backed class/namespace/relation/member-owner/directive-target/interaction-target facts from the class lexer token stream with complete/recovered provenance; Python compatibility still resolves `python` to `python3` on macOS.
- In progress: the next parser seam slice is either migrating another high-value family or deepening recovered facts with parse diagnostics. `FenceTextIndex` remains the shared migration projection, but flowchart, sequence, state, and class no longer depend on heuristic text scans for covered symbol facts in complete or incomplete buffers.
- Blocked: none
- Next action: migrate the next high-value parser-generator family (`er`) to the same core editor-facts contract, deepen class member/annotation/directive payload spans, deepen state directive payload spans for rename/references, or expose recovered parser diagnostics alongside recovered flowchart/sequence/state/class facts.

# Citations

- [LSP completion foundations plan](../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [Parser and semantic seam plan](../../plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md)
- [Diagnostics-first session handoff](sessions/2026-06-23-diagnostics-first-analysis-plan-handoff.md)
- [Diagnostics-first analysis ADR](../../adr/0070-diagnostics-first-analysis-contract.md)
- [Editor parser/semantic seam ADR](../../adr/0071-editor-parser-semantic-seam.md)
- [merman-analysis crate](../../../crates/merman-analysis/src/lib.rs)
- [FFI protocol](../../bindings/FFI_PROTOCOL.md)
- [FFI binding strategy ADR](../../adr/0066-ffi-binding-strategy.md)
- [WASM package surface ADR](../../adr/0069-wasm-package-surface-semantics.md)
