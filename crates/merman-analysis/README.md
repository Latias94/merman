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
- Editor-core consumes the typed `AnalysisResult`; LSP and VS Code project typed diagnostics and
  editor results without adding semantic deduplication or rewriting recovered-parser messages.

Editor-facing ownership is layered:

- `AnalysisResult` carries document-level diagnostics plus per-diagram syntax facts.
- `AnalysisFactsPayload` is the serializable facts contract for bindings. It includes the
  diagnostics summary, document/fence spans, parser fact provenance, semantic items, outline items,
  expected syntax, references, and the first typed Flowchart projection.
- `FenceTextIndex` preserves parser-complete, parser-recovered, or explicit-unavailable provenance
  for semantic facts and expected syntax.
- `merman-editor-core` owns protocol-neutral completion, hover, symbols, navigation, rename,
  selection ranges, folding ranges, and semantic-token queries over snapshots built directly from
  `AnalysisResult` and `FenceTextIndex`.
- LSP, WASM, and VS Code convert those protocol-neutral results into host surfaces.

## Rust API Migration Notes

### Analysis payload versioning

The diagnostics-only `AnalysisPayload` and richer `AnalysisFactsPayload` are separate serialized
contracts with separate version constants. `AnalysisPayload` remains version 1. The parser-only
`AnalysisFactsPayload` is the sole version 2 facts contract; readers reject facts v1 at the
version boundary before attempting to decode its body.

Facts v2 uses `fact_source: "unavailable"` when parser-backed body semantics do not exist and does
not manufacture body semantic items. Current writers include `rename_policy` on every
`semantic_items[]` entry so consumers can enforce the owning diagram family's identifier grammar.
Readers accept older v1 entries that omit this additive field and conservatively decode them as
`"none"`; missing metadata must never enable a rename operation.

Merman `0.8.0-alpha.3` exposed a superseded TextScan-capable alpha shape with the same numeric
discriminator. That implementation is deleted: there is no legacy decoder, executor, deprecated
alias, or dual projection path. Consumers of the alpha shape must update to the current v1 schema
and cannot infer compatibility from the version number alone. This schema version is independent
from LSP document revisions, Mermaid ids such as `flowchart-v2`, and native/WASM ABI versions.

`DocumentDiagram::text`, `AnalyzedDiagram::text`, and editor `FenceSnapshot::text` use
`SharedTextSlice` instead of owned `String` buffers. The slice shares the immutable document text
and stores UTF-8 byte bounds, so Markdown/MDX fence snapshots no longer copy every Mermaid body.
Consumers that only read the source should use `as_str()` or `AsRef<str>`. Consumers that need an
owned buffer can call `to_owned_text()`.

See `docs/adr/0070-diagnostics-first-analysis-contract.md` for the accepted architecture decision
and `docs/adr/0072-lint-rule-governance.md` for rule-origin, profile, and authoring-governance
policy.
