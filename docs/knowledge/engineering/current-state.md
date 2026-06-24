---
type: Current State
status: active
---

# Current State

- Goal: 完成 `docs/plans/2026-06-24-002-refactor-parser-semantic-seam-plan.md` 对应的开发与无畏重构，把 `merman-core` 的解析/语义边界做成 LSP 与 lint 可直接消费的 span-rich seam，同时继续保留 family-local parser 选择，并在需要时做可回滚的增量提交，持续到 2026-06-24 10:00。
- Branch: feat/diagnostics-analysis-contract
- Last verified: 2026-06-24 (`cargo fmt --all`, `cargo nextest run -p merman-core gantt_editor_facts_preserve_parser_symbol_spans --no-fail-fast`, `cargo nextest run -p merman-core gantt --no-fail-fast`, `cargo nextest run -p merman-core editor_facts --no-fail-fast`, `cargo nextest run -p merman-analysis --no-fail-fast`, `cargo nextest run -p merman-lsp --no-fail-fast`)
- Done: `merman-lsp` now exists as a dedicated crate; diagnostics are published from `merman-analysis`; Markdown fence diagnostics are remapped; plain Mermaid documents also get a snapshot fence; shared LSP mapping helpers now live in `merman-analysis`; snapshot now carries diagram type facts for each fence; completion now covers diagram headers, directions, operators, directives, shapes, and local node IDs with snapshot-derived replacement edits; `merman-analysis::document::analyze_document` now gives CLI lint and LSP one shared document-analysis seam; `server_smoke` proves initialize/open/change/save publish the current diagnostics version; `document_store` now validates both plain Mermaid and Markdown fence snapshot facts and proves newer versions replace older snapshots; hover/documentSymbol/definition/references/prepareRename/rename now consume the shared `merman-analysis::FenceTextIndex`; LSP snapshot no longer owns its own completion/structure scan; flowchart fences now prefer parser-backed `merman-core::EditorSemanticFacts` with original-text byte spans for node ids, subgraph headers, and directive prefixes; incomplete flowchart buffers now produce `EditorSemanticCompleteness::Recovered` facts from the same lexer token stream and LSP records `FenceTextIndexSource::ParserRecovered` instead of falling back to text scan; sequence fences now emit parser-backed actor/participant/message-endpoint/note/box facts with complete/recovered provenance; state fences now emit parser-backed state/reference/fork/join/choice facts from AST spans and recover incomplete buffers from the state lexer token stream; class fences now emit parser-backed class/namespace/relation/member-owner/directive-target/interaction-target facts plus class-body and inline member outline spans, while annotation names are preserved as payload spans, all from the class lexer token stream with complete/recovered provenance; ER fences now emit parser-backed entity/relationship endpoint/attribute/class/style/classDef facts with complete/recovered provenance, with attribute names projected as outline-only facts and attribute type/key/comment spans preserved as payload facts; incomplete attribute blocks no longer loop EOF recovery; mindmap now emits parser-backed node facts from a shared parser event stream, preserves class/icon decoration semantics, and LSP records `ParserComplete`/`ParserRecovered` instead of falling back to `TextScan`; Gantt now emits parser-backed task id, section outline, directive payload, click payload, single-line accessibility payload, `after`/`until` dependency, `click` target, and directive-prefix facts with complete/recovered provenance, while keeping `section`, directive values, accessibility text, click URLs, callbacks, and callback args out of node-id completion; `merman-lsp` now opts into core full/host features so product LSP detection includes mindmap rather than the tiny registry; Python compatibility still resolves `python` to `python3` on macOS.
- In progress: the next parser seam slice is deeper directive payload spans/recovered diagnostics for already migrated families, or migration of the next line-driven family that exposes editor-visible structure. `FenceTextIndex` now respects semantic roles for completion/navigation/outline projection, but flowchart, sequence, state, class directive payloads, ER, mindmap, and Gantt multiline accessibility payloads still need more payload-depth if lint/rename is to become fully parser-backed.
- Blocked: none
- Next action: deepen class/state/mindmap directive payload spans, design cross-line payload spans for Gantt multiline `accDescr`, or expose recovered parser diagnostics alongside recovered parser facts.

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
