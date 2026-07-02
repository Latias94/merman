# merman-analysis

`merman-analysis` owns the diagnostics-first JSON contract and the richer parser-backed analysis
result for Merman lint, validation, document/fence source mapping, binding payloads, and editor
projections.

The crate intentionally starts below FFI, UniFFI, WASM, CLI, and render wrappers. It provides
stable JSON payload types, `AnalysisResult` syntax facts, `DocumentSource` extraction for plain
Mermaid / Markdown / MDX, source-position mapping helpers, and the canonical policy for merging
parser diagnostics with recovered editor facts.

Diagnostic ownership is intentionally narrow:

- `merman-core` emits structured parse diagnostics with exact spans, insertion points, or explicit
  fallback locations.
- `merman-analysis` maps those parser facts into stable rule ids, metadata, Markdown ranges, and
  duplicate/recovery policy.
- Editor-core, LSP, and VS Code project analysis payloads without adding semantic deduplication or
  rewriting recovered-parser messages.

Editor-facing ownership is layered:

- `AnalysisResult` carries document-level diagnostics plus per-diagram syntax facts.
- `AnalysisFactsPayload` is the serializable facts contract for bindings. It includes the
  diagnostics summary, document/fence spans, parser fact provenance, semantic items, outline items,
  expected syntax, references, and the first typed Flowchart projection.
- `FenceTextIndex` preserves parser-complete, parser-recovered, or text-scan provenance for
  semantic facts and expected syntax.
- `merman-editor-core` owns protocol-neutral completion, hover, symbols, navigation, rename,
  selection ranges, folding ranges, and semantic-token queries over snapshots projected from
  analysis facts.
- LSP, WASM, and VS Code convert those protocol-neutral results into host surfaces.

See `docs/adr/0070-diagnostics-first-analysis-contract.md` for the accepted architecture decision
and `docs/adr/0072-lint-rule-governance.md` for rule-origin, profile, and authoring-governance
policy.
