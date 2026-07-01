# merman-analysis

`merman-analysis` owns the diagnostics-first contract for Merman lint, validation, document/fence
source mapping, binding payloads, and LSP diagnostics.

The crate intentionally starts below FFI, UniFFI, WASM, CLI, and render wrappers. It provides
stable JSON payload types, `DocumentSource` extraction for plain Mermaid / Markdown / MDX,
source-position mapping helpers, and the canonical policy for merging parser diagnostics with
recovered editor facts.

Diagnostic ownership is intentionally narrow:

- `merman-core` emits structured parse diagnostics with exact spans, insertion points, or explicit
  fallback locations.
- `merman-analysis` maps those parser facts into stable rule ids, metadata, Markdown ranges, and
  duplicate/recovery policy.
- Editor-core, LSP, and VS Code project analysis payloads without adding semantic deduplication or
  rewriting recovered-parser messages.

Editor-facing ownership is separate:

- `FenceTextIndex` preserves parser-complete, parser-recovered, or text-scan provenance for
  semantic facts.
- `merman-editor-core` owns protocol-neutral completion, hover, symbols, navigation, rename, and
  semantic-token queries over document snapshots.
- LSP, WASM, and VS Code convert those protocol-neutral results into host surfaces.

See `docs/adr/0070-diagnostics-first-analysis-contract.md` for the accepted architecture decision
and `docs/adr/0072-lint-rule-governance.md` for rule-origin, profile, and authoring-governance
policy.
