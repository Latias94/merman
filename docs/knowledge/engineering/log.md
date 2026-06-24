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
