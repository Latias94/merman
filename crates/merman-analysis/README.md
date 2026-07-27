# merman-analysis

[![Crates.io](https://img.shields.io/crates/v/merman-analysis.svg)](https://crates.io/crates/merman-analysis) [![Documentation](https://docs.rs/merman-analysis/badge.svg)](https://docs.rs/merman-analysis) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Parser-backed Mermaid diagnostics, lint metadata, and document source mapping without SVG or export dependencies.

Use `merman-analysis` directly when a Rust application needs to validate Mermaid, inspect diagnostics, analyze Markdown/MDX fences, or build editor tooling. Use [`merman`](https://crates.io/crates/merman) for rendering, [`merman-cli`](https://crates.io/crates/merman-cli) for command-line linting, or [`merman-lsp`](https://crates.io/crates/merman-lsp) for an editor protocol.

## Quick Start

<!-- BEGIN GENERATED RELEASE README ANALYSIS_INSTALL -->

```sh
cargo add merman-analysis --git https://github.com/Latias94/merman
```

<!-- END GENERATED RELEASE README ANALYSIS_INSTALL -->

Analyze one Mermaid diagram:

```rust
use merman_analysis::{AnalysisOptions, Analyzer, SourceDescriptor};

fn main() {
    let options = AnalysisOptions::default().with_source(
        SourceDescriptor::diagram().with_path("diagram.mmd"),
    );
    let result = Analyzer::with_options(options)
        .analyze_result("flowchart TD\n  A[Start] -->");

    for diagnostic in result.diagnostics() {
        println!("{}: {}", diagnostic.id, diagnostic.message);
    }

    assert!(!result.payload().valid);
}
```

`AnalysisResult` retains typed per-diagram syntax facts. Call `into_payload()` when a diagnostics-only result is sufficient, or `to_facts_payload()` when a binding or editor needs the serializable facts contract.

## Analyze Markdown And MDX

`analyze_document_result` extracts Mermaid fences and maps every diagnostic back to the enclosing document:

````rust
use merman_analysis::{
    Analyzer, analyze_document_result, source_descriptor_for_markdown_path,
};

let markdown = "```mermaid\nflowchart TD\n  A -->\n```\n";
let result = analyze_document_result(
    markdown,
    &Analyzer::new(),
    source_descriptor_for_markdown_path(Some("README.md")),
);

assert_eq!(result.diagrams().len(), 1);
assert!(!result.diagnostics().is_empty());
````

Use `analyze_document` for the smaller diagnostics payload and `analyze_document_facts` for the versioned binding payload.

## What This Crate Owns

- Stable diagnostic IDs, severities, metadata, fixes, and source ranges.
- Plain Mermaid, Markdown, and MDX document extraction.
- UTF-8 byte, line/column, and LSP-compatible source mapping.
- Parser-backed semantic items, outline items, references, expected syntax, and provenance.
- Rule catalogs and the policy for merging parser diagnostics with recovered editor facts.
- Deterministic analysis options and explicit source-size limits.

The layer boundaries are deliberate:

- `merman-core` emits structured parser facts and exact spans.
- `merman-analysis` turns those facts into diagnostics and document-level results.
- `merman-editor-core` queries typed analysis snapshots for editor behavior.
- LSP, WASM, CLI, FFI, and UniFFI only project those results into their host protocols.

## Runtime And Features

The crate has no default features and does not depend on SVG, raster, PDF, or editor protocol implementations. Analysis is deterministic by default.

The optional `system-clock`, `system-timezone`, `system-random`, and `system-timing` features only make their matching runtime adapters available. They do not change Mermaid language coverage or select ambient host state automatically.

## Payload Contracts

The diagnostics-only `AnalysisPayload` and richer `AnalysisFactsPayload` are independent, versioned JSON contracts. Their current public versions are both `1`; consumers must validate the version belonging to the payload they decode.

Facts use `fact_source: "unavailable"` when parser-backed body semantics do not exist. They do not invent body symbols, references, or rename targets. Current writers include `rename_policy` on each semantic item; older additive readers that do not see it must treat the item as non-renamable.

`DocumentDiagram::text`, `AnalyzedDiagram::text`, and editor `FenceSnapshot::text` use `SharedTextSlice`, which shares immutable document storage instead of copying every fence body. Use `as_str()` or `AsRef<str>` for borrowed access and `to_owned_text()` only when an owned buffer is required.

## Related Documentation

- [Diagnostics architecture](https://github.com/Latias94/merman/blob/main/docs/adr/0070-diagnostics-first-analysis-contract.md)
- [Lint rule governance](https://github.com/Latias94/merman/blob/main/docs/adr/0072-lint-rule-governance.md)
- [Editor and LSP capabilities](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [Project compatibility status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
