# merman-editor-core

[![Crates.io](https://img.shields.io/crates/v/merman-editor-core.svg)](https://crates.io/crates/merman-editor-core) [![Documentation](https://docs.rs/merman-editor-core/badge.svg)](https://docs.rs/merman-editor-core) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Protocol-neutral Mermaid editor intelligence for Rust hosts.

Use [`merman-lsp`](https://crates.io/crates/merman-lsp) when an editor can speak the Language Server Protocol. Use this library, normally through the [`merman`](https://crates.io/crates/merman) facade, when a Rust host needs completion, navigation, rename, and diagnostics in process without LSP, WASM, Monaco, or VS Code types.

## Quick Start

This guide targets the `0.8.0-alpha.7` candidate. Alpha.6 remains the published baseline during preparation and predates the family selectors; use a matching source checkout until alpha.7 is available. Choose `all-diagrams` to retain all parsers or the specific `diagram-*` families your editor accepts:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.7", default-features = false, features = ["all-diagrams", "analysis", "editor"] }
```

```rust
use merman::analysis::Analyzer;
use merman::editor::{
    DocumentKind, Position, analyze_document_snapshot_with_shared_text,
    completion_for_snapshot, search_document_symbols,
};
use std::sync::Arc;

let snapshot = analyze_document_snapshot_with_shared_text(
    &Analyzer::new(),
    "file:///workspace/diagram.mmd",
    1,
    Arc::from("flowchart TD\nA --> B\nB -->"),
    DocumentKind::Diagram,
)
.expect("source is within the configured analysis limit");
let completions = completion_for_snapshot(&snapshot, Position::new(2, 5));
let symbols = search_document_symbols(&snapshot, "B");
```

`analyze_document_snapshot_with_shared_text` returns `Result<DocumentSnapshot, AnalysisRejection>`. The caller owns document storage and passes an `Arc<str>`, so the editor boundary does not clone the complete source. `DocumentKind::Diagram` handles standalone Mermaid files; Markdown and MDX use the parser-backed host-fence pipeline and retain original-document source ranges.

Use `analyze_document_context_with_shared_text` when the initial analysis payload is also needed. Its cancellable counterpart returns `Result<Result<DocumentAnalysisContext, AnalysisRejection>, AnalysisCancelled>`: cooperative cancellation is the outer channel, while a completed resource rejection remains the inner channel.

## Responsibilities

- Construct immutable document, diagram, and Mermaid-fence snapshots through one-shot analysis functions, `DocumentSnapshot`, and `FenceSnapshot`.
- Project parser-backed facts into completion, hover, symbols, folding, definition, references, prepare-rename, rename, selection ranges, and code actions.
- Preserve source provenance with `FenceTextIndexSource`, distinguishing `ParserComplete`, `ParserRecovered`, and `Unavailable` facts.
- Keep exact original-source spans when preprocessing can represent them; omit unrepresentable facts and emit recovery diagnostics.
- Keep all editor results protocol-neutral so adapters can map them to LSP, browser, or native UI types.

`Unavailable` means no body semantics are projected. Header and template suggestions use the static family catalog only for compiled parsers. The crate does not invent body symbols, references, or rename targets without complete or recovered parser facts. Defaults are empty, and parser availability can be widened by Cargo feature union; see the [migration guide](../../docs/FEATURES.md#select-diagram-families).

## Data Contract

Editor queries operate directly on typed snapshots and shared `AnalysisGeneration` storage; they do not serialize and deserialize an analysis payload internally. Bindings expose the separate `AnalysisFactsPayload` schema 2, and reject other schema versions at their boundary.

When a host already owns an `Arc<AnalysisGeneration>`, use `DocumentSnapshot::try_from_analysis_generation(version, generation)`. The snapshot derives its URI and `DocumentKind` from the generation's `SourceDescriptor`, so callers cannot pair parser evidence with a different document identity. Generations without a source path return `DocumentSnapshotError::MissingSourcePath` instead of creating an anonymous editor snapshot.

The former stateful `DocumentWorkspace` map and `DocumentAnalysisOutcome` wrapper were removed. Hosts should keep their own URI/version lifecycle state and store the returned immutable snapshot or context directly; no deprecated alias or hidden document cache remains in editor-core.

The removed TextScan implementation is not maintained in parallel. This does not change LSP document revision numbers or Mermaid's own `*-v2` diagram IDs.

Completion policy is evaluated only by `completion_for_snapshot`. The former public
`CompletionContext` accessor wrapper was removed rather than retained as a compatibility alias.

```compile_fail
use merman_editor_core::CompletionContext;
```

Completion adapters should likewise derive activation characters from
`COMPLETION_TRIGGER_CHARACTERS`; LSP and browser adapters project that one editor-owned list.

Syntax highlighting is intentionally outside this crate. Merman's LSP and Playground adapters use
the canonical `tree-sitter-mermaid` grammar and portable highlight query, while this crate remains
the protocol-neutral owner of strict semantic editor features.

## Boundary

This crate owns semantic editor behavior, not transport policy. URI conversion, protocol request and response types, client capability negotiation, document synchronization, cancellation wiring, and UI behavior belong to adapters such as `merman-lsp` or `@mermanjs/web-editor`.

The optional `system-clock`, `system-timezone`, `system-random`, and `system-timing` features forward the corresponding analysis and parser adapters. They do not add editor features or Mermaid families.

## License

Licensed under either Apache-2.0 or MIT at your option.
