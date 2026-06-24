---
type: Work Progress
status: active
---

# Log

## 2026-06-24
- Continued the LSP completion foundations follow-up on `feat/diagnostics-analysis-contract`.
- Verified the new `merman-lsp` crate compiles and its focused tests pass, including completion, diagnostics, and document-store coverage.
- Recorded durable engineering memory for the current plan so later sessions can resume without rereading the full chat.
- Extracted `merman-analysis::document::analyze_document` as the shared plain/markdown document-analysis seam, and switched CLI lint plus LSP publishing to it so fence scanning and diagnostic mapping stay centralized.
- Re-verified `cargo fmt --all --check`, `cargo check -p merman-analysis -p merman-cli -p merman-lsp`, and `cargo test -p merman-analysis -p merman-cli -p merman-lsp --tests` after the shared seam landed.
- Added a `DocumentStore` regression proving newer versions replace older snapshots while keeping the latest fence metadata and diagram type.
- Re-verified `cargo fmt --all --check` and `cargo test -p merman-lsp --tests` after the versioned snapshot regression landed.
- Reframed the next LSP slice from pure completion polish toward a shared structure layer for hover/documentSymbol so the same snapshot seam can feed future symbol-oriented features.
- Added a first-pass `merman-lsp::structure` module to explore hover/documentSymbol on top of the existing snapshot seam, then tightened it after an initial compile check surfaced interface mismatches.
- Updated the durable engineering memory to say the current follow-up is the shared structure layer rather than only completion metadata.
- Extended the same fence-local structure layer beyond hover/documentSymbol to cover `textDocument/definition`, `textDocument/references`, `textDocument/prepareRename`, and `textDocument/rename` with shared snapshot-driven navigation facts.
- Re-verified `cargo fmt --all`, `cargo check -p merman-lsp`, and `cargo test -p merman-lsp --tests` after the navigation surface landed.
- Confirmed that the next slice should not force a repository-wide parser rewrite. The follow-up is now a new parser/semantic seam plan plus ADR, so later LSP and lint work can consume span-rich parser facts instead of raw-text heuristic scans.
- Centralized the current editor-facing fence structure into `merman-analysis::FenceTextIndex`, removed the separate LSP completion/navigation scan implementations, and re-verified `cargo fmt --all` plus `cargo test -p merman-analysis -p merman-lsp`.
- Added the initial `merman-core::EditorSemanticFacts` contract and migrated flowchart node/subgraph editor symbols to parser-backed original-text spans; `merman-lsp` now prefers those facts through `FenceTextIndex::from_core_facts` while preserving directive-prefix completion state.
- Re-verified `cargo fmt --all`, `cargo nextest run -p merman-core parse_flowchart_editor_facts`, `cargo nextest run -p merman-analysis editor::tests`, and `cargo nextest run -p merman-lsp` after the flowchart parser-backed editor facts landed.
- Added `EditorSemanticCompleteness` and `FenceTextIndexSource` provenance so recovered parser facts can be distinguished from complete parser facts and old text scans.
- Flowchart editor fact extraction now recovers symbols from the masked lexer token stream when LALRPOP parsing fails on incomplete buffers, and LSP has a regression proving incomplete flowcharts use `ParserRecovered` rather than `TextScan`.
- Migrated Sequence as the second family onto `EditorSemanticFacts`: actor/participant/message/note/box symbols now come from the sequence lexer token stream with complete/recovered provenance, and LSP has regressions proving sequence fences use parser facts instead of text scans.
- Migrated State as the third family onto `EditorSemanticFacts`: `StateStmt` now carries source spans, state grammar preserves spans for parser-backed state symbols, incomplete buffers recover from the state lexer token stream, and LSP regressions prove state fences use `ParserComplete`/`ParserRecovered` instead of text scans.
- Recorded the current fearless-refactor rule: for families with deterministic lexer/parser seams, old raw-text editor scans are only a migration fallback; future class/ER/state-reference work should extend core facts rather than adding LSP heuristics.
- Migrated Class as the fourth family onto `EditorSemanticFacts`: class/namespace/relation/member-owner/directive-target/interaction-target symbols now come from the class lexer token stream with LALRPOP complete/recovered provenance, and LSP regressions prove class fences use parser facts instead of text scans.
- Recorded the next class-specific deepening opportunities: member-level spans, annotation payload spans, and directive payload reference spans should be modeled in core facts before improving product-grade rename/lint for those constructs.
- Migrated ER as the fifth family onto `EditorSemanticFacts`: `IdList` now preserves per-id spans, entity/relation/attribute/class/style/classDef facts come from the ER lexer token stream, and LSP regressions prove ER fences use `ParserComplete`/`ParserRecovered` instead of text scans.
- Fixed ER incomplete attribute block recovery so the lexer emits the EOF error once and exits block mode, preventing editor fact recovery from hanging on partial buffers.
- Migrated Mindmap as the first hand-written-family tracer bullet onto `EditorSemanticFacts`: its line parser now produces a shared event stream for DB/render semantics and editor facts, preserving node spans, class/icon directives, inline-header spans, multiline labels, and recovered incomplete-delimiter facts.
- Fixed `merman-lsp` feature profile drift by giving the LSP crate default `core-full`/`core-host` feature passthroughs, so product LSP detection includes mindmap and no longer silently uses the tiny core registry.
- Recorded the next high-return fearless-refactor candidates: `gantt` should follow the mindmap event-stream pattern, while class/ER/state/mindmap can be deepened with payload spans and recovered diagnostics before product-grade lint/rename work.
- Migrated Gantt onto `EditorSemanticFacts`: task ids, `after`/`until` dependency references, `click` targets, and directive prefixes now come from the Gantt parser statement rules with complete/recovered provenance.
- Exposed Gantt relative-reference ranges from the date parser helper so editor facts reuse the same Mermaid-backed dependency matcher as render semantics.
- Kept Gantt `section` as a directive prefix rather than a node id to avoid polluting task-id completion; future section document-symbol support should use role-aware or outline-only facts.
- Made Gantt editor completeness tolerant of original-source YAML front matter and Mermaid init directives, preserving complete provenance while still using original byte spans.
- Added Gantt LSP regressions proving complete and incomplete documents use `ParserComplete`/`ParserRecovered`, and added `gantt` diagram-header completion.
- Re-verified `cargo fmt --all`, `cargo nextest run -p merman-core editor_facts --no-fail-fast`, `cargo nextest run -p merman-core gantt --no-fail-fast`, `cargo nextest run -p merman-analysis --no-fail-fast`, and `cargo nextest run -p merman-lsp --no-fail-fast`.
- Added `EditorSemanticRole` to the core editor semantic contract so parser facts can be projected as entity, outline-only, or payload-only symbols.
- Updated `merman-analysis::FenceTextIndex::from_core_facts` to respect semantic roles, keeping payload facts out of completion/navigation while still projecting entity and outline facts.
- Deepened ER editor facts so attribute names are outline-only symbols and attribute type/key/comment spans are preserved as payload facts with accurate source spans.
- Added regressions proving ER payload facts do not pollute completion ids while core facts still preserve the spans for future lint consumers.
- Deepened Class editor facts so class-body members and inline `Class: member` entries are outline-only symbols, while annotation names are payload-only spans for future lint/semantic consumers.
- Added LSP regressions proving class member outline facts and annotation payload facts do not pollute completion ids, and re-verified class/core editor facts plus analysis/LSP suites.

## 2026-06-23
- Confirmed alpha-stage fearless refactor scope for diagnostics-first analysis: canonical `analyze_json`, legacy `validate_json` projection, CLI lint, Markdown fence diagnostics, LSP-ready position mapping, ADR, and engineering memory are in scope.
- Wrote `docs/plans/2026-06-23-002-refactor-diagnostics-first-analysis-plan.md` as the execution plan for the diagnostics-first analysis core.
- Recorded that Mermaid JS should remain parity evidence and fixture/harness input, not a runtime fallback for Rust, CLI, WASM, FFI, or platform hosts.
- Fixed local macOS shell compatibility so `python` resolves to `python3` from non-interactive zsh via `~/.zshenv`.
- Added `docs/adr/0070-diagnostics-first-analysis-contract.md` and updated FFI, UniFFI, options JSON, and README docs so `analyze_json` is the canonical diagnostics payload and `validate_json` is the compatibility projection.
- Added the `merman-analysis` workspace crate with diagnostics payload types, source descriptors, severity/category enums, diagnostic spans, UTF-16 LSP position mapping, and schema/source-map tests.
- Added the render-free `merman-analysis::Analyzer` pipeline, status-code mirror, semantic warning registry, and analyzer tests for no-diagram, parse errors, unsupported diagrams, valid flowcharts, GitGraph duplicate commit IDs, Block width overflow, source byte limits, and panic status mapping.
- Migrated `merman-bindings-core` so canonical `analyze_json` and legacy `validate_json` are both derived from the same analyzer, and threaded `analyze_json` through C FFI, UniFFI, WASM, and platform wrappers while keeping existing `validate` compatibility paths intact.
- Rebuilt the browser package surface so `@mermanjs/web` now exports `analyze()` / `analyzeJson()` in its checked-in `pkg` and `dist` artifacts, and updated Flutter/web/protocol docs to describe diagnostics analysis as present rather than future work.
- Added first-class `merman-cli lint` support on top of `merman-analysis`, with canonical JSON/text output, Markdown/MDX fence scanning, `--stdin-file-name` for stdin linting, fence-related diagnostic remapping, CLI coverage, and README help text.

## 2026-06-18
- Verified source-backed Flowchart ELK probes are green.
- Ported compound parent-end external dummy net-flow handling in `merman-elk-layered` closer to ELK `calculateNetFlow` behavior.
- Added regression coverage for parent-end external dummy net-flow behavior and existing compound metadata tests still pass.
- Ported inside-self-loop handling so ELK `insideSelfLoops.activate` nodes create nested graphs and `inside_self_loops_yo` edges are imported into the source node nested graph.
- Added regression coverage for inside-self-loop nested graph creation and kept source-backed probe coverage green.
- Verified `cargo test -p merman-elk-layered --tests`, `cargo test -p merman-layout-elk --tests`, `cargo run -p xtask -- check-flowchart-elk-source-backed-probes`, and `cargo fmt --all`.
