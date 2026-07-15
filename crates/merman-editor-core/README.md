# merman-editor-core

Protocol-neutral editor intelligence for Merman.

This crate is an internal Rust reuse layer shared by protocol adapters such as `merman-lsp` and
browser adapters such as `merman-wasm`. External editors should normally integrate through the LSP
server rather than depending on this crate directly.

## Responsibilities

- Own document snapshots and source/fence lookup through `DocumentWorkspace`, `DocumentSnapshot`,
  and `FenceSnapshot`.
- Query parser-backed semantic facts for completion, hover, document symbols, workspace symbols,
  definition, references, prepare-rename, rename, and semantic tokens.
- Preserve semantic fact provenance with `FenceTextIndexSource` so callers can tell
  `ParserComplete`, `ParserCompleteDegradedSpans`, `ParserRecovered`,
  `ParserRecoveredDegradedSpans`, and `Unavailable` results apart.
- Keep language behavior protocol-neutral: no LSP `Url`, `Range`, `Diagnostic`, or VS Code
  ownership policy lives here.

`ParserCompleteDegradedSpans` and `ParserRecoveredDegradedSpans` remain parser-backed for identity
and outline facts, but callers must treat their spans as unavailable for precise source edits when
analysis reports `source_mapped_spans=false`. `Unavailable` means no body semantics are projected;
source-start headers and templates are read independently from the static family catalog. New
editor behavior should deepen parser-backed semantic facts in `merman-core` / `merman-analysis`
rather than adding protocol-layer scans.

Editor-core builds typed snapshots directly from `AnalysisResult` and `FenceTextIndex`; it does not
serialize or deserialize an analysis payload internally. The separately exposed binding wire is
the current `AnalysisFactsPayload` version 1. The superseded TextScan-capable alpha implementation
is removed rather than supported in parallel. This does not change LSP document revision numbers or
Mermaid's own `*-v2` diagram ids.

For parser-backed families, completion, navigation, and refactoring continue through the same
protocol-neutral queries. The breaking behavior is deliberate on unavailable input: editor-core no
longer invents body symbols, references, or rename targets when a family parser cannot provide
complete or recovered facts.
