# merman-editor-core

[![Crates.io](https://img.shields.io/crates/v/merman-editor-core.svg)](https://crates.io/crates/merman-editor-core) [![Documentation](https://docs.rs/merman-editor-core/badge.svg)](https://docs.rs/merman-editor-core) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Protocol-neutral Mermaid editor intelligence for Rust hosts.

Use [`merman-lsp`](https://crates.io/crates/merman-lsp) when an editor can speak the Language Server Protocol. Use this library, normally through the [`merman`](https://crates.io/crates/merman) facade, when a Rust host needs completion, navigation, rename, diagnostics, and semantic-token logic in process without LSP, WASM, Monaco, or VS Code types.

## Quick Start

Use the facade from the repository's default branch:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.5", default-features = false, features = ["analysis", "editor"] }
```

```rust
use merman::editor::{
    DocumentKind, DocumentWorkspace, Position, completion_for_snapshot,
    search_document_symbols,
};

let mut workspace = DocumentWorkspace::new();
let snapshot = workspace
    .upsert(
        "file:///workspace/diagram.mmd",
        1,
        "flowchart TD\nA --> B\nB -->".to_owned(),
        DocumentKind::Diagram,
    )
    .expect("source is within the configured analysis limit");
let completions = completion_for_snapshot(&snapshot, Position::new(2, 5));
let symbols = search_document_symbols(&snapshot, "B");
```

`DocumentWorkspace::upsert` returns `Result<DocumentSnapshot, AnalysisRejection>` and leaves the workspace unchanged when analysis rejects the source. `DocumentKind::Diagram` handles standalone Mermaid files. Markdown-family documents use the same parser-backed fence indexing and retain original-document source ranges.

## Responsibilities

- Own document, diagram, and Mermaid-fence snapshots through `DocumentWorkspace`, `DocumentSnapshot`, and `FenceSnapshot`.
- Project parser-backed facts into completion, hover, symbols, folding, definition, references, prepare-rename, rename, selection ranges, code actions, and semantic tokens.
- Preserve source provenance with `FenceTextIndexSource`, distinguishing `ParserComplete`, `ParserRecovered`, and `Unavailable` facts.
- Keep exact original-source spans when preprocessing can represent them; omit unrepresentable facts and emit recovery diagnostics.
- Keep all editor results protocol-neutral so adapters can map them to LSP, browser, or native UI types.

`Unavailable` means no body semantics are projected. Header and template suggestions can still come from the static family catalog, but the crate does not invent body symbols, references, or rename targets without complete or recovered parser facts.

## Data Contract

Editor queries operate directly on typed snapshots and shared `AnalysisGeneration` storage; they do not serialize and deserialize an analysis payload internally. Bindings expose the separate `AnalysisFactsPayload` schema 2, and reject other schema versions at their boundary.

When a host already owns an `Arc<AnalysisGeneration>`, use `DocumentSnapshot::try_from_analysis_generation(version, generation)`. The snapshot derives its URI and `DocumentKind` from the generation's `SourceDescriptor`, so callers cannot pair parser evidence with a different document identity. Generations without a source path return `DocumentSnapshotError::MissingSourcePath` instead of creating an anonymous editor snapshot.

The removed TextScan implementation is not maintained in parallel. This does not change LSP document revision numbers or Mermaid's own `*-v2` diagram IDs.

## Semantic Token Planning

Use `plan_semantic_tokens_for_snapshot` or `plan_semantic_tokens_for_snapshot_range`. Both return `Result<SemanticTokenPlan, TokenPlanError>`; range planning accepts editor-core's protocol-neutral `Range`. Inspect `SemanticTokenPlan::tokens()` for `PlannedToken` values or `packed()` for the generated five-word LSP-relative UTF-16 representation.

`semantic_token_descriptor()` is the single descriptor for `PlannedTokenKind` codes, `PlannedTokenModifier` bits, LSP legend indices, and packed-field order. Hosts should derive protocol tables from that descriptor rather than maintaining a second legend or numeric mapping.

## Boundary

This crate owns semantic editor behavior, not transport policy. URI conversion, protocol request and response types, client capability negotiation, document synchronization, cancellation wiring, and UI behavior belong to adapters such as `merman-lsp` or `@mermanjs/web-editor`.

The optional `system-clock`, `system-timezone`, `system-random`, and `system-timing` features forward the corresponding analysis and parser adapters. They do not add editor features or Mermaid families.

## License

Licensed under either Apache-2.0 or MIT at your option.
