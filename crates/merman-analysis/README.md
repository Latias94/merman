# merman-analysis

[![Crates.io](https://img.shields.io/crates/v/merman-analysis.svg)](https://crates.io/crates/merman-analysis) [![Documentation](https://docs.rs/merman-analysis/badge.svg)](https://docs.rs/merman-analysis) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Parser-backed Mermaid diagnostics, lint metadata, and document source mapping without SVG or export dependencies.

Use `merman-analysis` directly when a Rust application needs to validate Mermaid, inspect diagnostics, analyze Markdown/MDX fences, or build editor tooling. Use [`merman`](https://crates.io/crates/merman) for rendering, [`merman-cli`](https://crates.io/crates/merman-cli) for command-line linting, or [`merman-lsp`](https://crates.io/crates/merman-lsp) for an editor protocol.

## Quick Start

```sh
cargo add merman-analysis@0.8.0-alpha.6
```

Analyze one Mermaid diagram:

```rust
use merman_analysis::{AnalysisOptions, Analyzer, SourceDescriptor};

fn main() {
    let options = AnalysisOptions::default().with_source(
        SourceDescriptor::diagram().with_path("diagram.mmd"),
    )
    .with_max_source_bytes(Some(4 * 1024 * 1024))
    .with_max_document_diagrams(Some(256));
    let analyzer = Analyzer::with_options(options);
    let outcome = analyzer.analyze_generation("flowchart TD\n  A[Start] -->");
    let generation = outcome
        .into_ready()
        .expect("source is within the configured analysis limit");
    let payload = generation.project(analyzer.options().diagnostic_policy());

    for diagnostic in &payload.diagnostics {
        println!("{}: {}", diagnostic.id, diagnostic.message);
    }

    assert!(!payload.valid);
    assert_eq!(analyzer.options().max_source_bytes(), Some(4 * 1024 * 1024));
    assert_eq!(analyzer.options().max_document_diagrams(), Some(256));
}
```

`Analyzer::analyze_generation` returns `AnalysisCaptureOutcome` so callers can distinguish a completed `AnalysisGeneration` from an `AnalysisRejection`. A rejection exposes a typed `AnalysisResourceLimit` and its canonical diagnostics payload. A generation is bound to the parser environment and snapshot policy used for capture, but retains only the opaque environment identity and source metadata needed after parsing. It does not retain the site/runtime policy or an initial diagnostics payload. Call `AnalysisGeneration::project` with a diagnostic policy, use `Analyzer::analyze` for the smaller diagnostics-only path, or use `Analyzer::analyze_facts` when a binding needs the serializable facts contract.

The configured `SourceDescriptor` selects the canonical capture path. `SourceKind::Diagram` analyzes the whole input as one Mermaid diagram, while `SourceKind::Markdown` and `SourceKind::Mdx` extract fences and enforce `max_document_diagrams`; Analyzer entry points cannot create a Markdown identity around whole-document Mermaid facts. The free `analyze_document*` functions remain useful when the source descriptor varies per call.

Caller cancellation is exposed only by the `*_cancellable` entry points and remains outside `AnalysisCaptureOutcome`. Rich cancellable capture consumes caller-owned `Arc<str>` through `Analyzer::analyze_generation_shared_cancellable`; ownership promotion therefore happens before the cancellable operation instead of hiding an uninterruptible full-source copy inside it. Use `Analyzer::analyze_generation_shared` when a non-cancellable caller also wants to retain the same allocation. The borrowed `Analyzer::analyze_generation` entry point remains the non-cancellable convenience API.

A parser-controlled path that returns outer cancellation to a non-cancellable facade violates that facade's contract: core exposes `Error::ParseCancelled`, while analysis projects the protected `merman.internal.parser_contract_violation` diagnostic. Use the cancellable lifecycle whenever cancellation is expected control flow.

## Analyze Markdown And MDX

`analyze_document_generation` extracts Mermaid fences and maps projected diagnostics back to the enclosing document:

````rust
use merman_analysis::{
    Analyzer, analyze_document_generation, source_descriptor_for_markdown_path,
};

let markdown = "```mermaid\nflowchart TD\n  A -->\n```\n";
let analyzer = Analyzer::new();
let outcome = analyze_document_generation(
    markdown,
    &analyzer,
    source_descriptor_for_markdown_path(Some("README.md")),
);
let generation = outcome
    .into_ready()
    .expect("source is within the configured analysis limit");
let payload = generation.project(analyzer.options().diagnostic_policy());

assert_eq!(generation.diagrams().len(), 1);
assert!(!payload.diagnostics.is_empty());
````

Use `analyze_document` for the smaller diagnostics payload and `analyze_document_facts` for the versioned binding payload.

Hosts can bound admission before source maps, fence objects, or parser state are created:

```rust
use merman_analysis::AnalysisOptions;

let options = AnalysisOptions::default()
    .with_max_source_bytes(Some(4 * 1024 * 1024))
    .with_max_document_diagrams(Some(256));
```

`max_document_diagrams` applies only to Markdown and MDX host documents. The canonical fence scanner stops at the first excess Mermaid opener and rejects the whole generation; standalone Mermaid sources are unaffected. Rust callers choose both limits explicitly. The LSP supplies bounded defaults for unconfigured clients.

`DiagnosticFix::edits` is shared immutable storage. Cloning a fix or projecting the same document-wide fix onto multiple diagnostics shares one edit allocation while JSON continues to serialize `edits` as the schema-1 array. Retained memory is therefore linear in the shared edit allocation, but schema-1 wire output still repeats the complete array for every diagnostic that owns the action; removing that wire repetition requires a future schema revision. Iterate or index the slice as before; construct fixes with `DiagnosticFix::new` instead of mutating the edit collection in place.

Init-directive migration fixes are advisory and resource-bounded independently from source admission. Analysis skips the fix, while retaining the diagnostic, when the captured source config overrides exceed their 1 MiB owned-value budget, existing frontmatter input or materialized YAML exceeds 1 MiB, nesting exceeds 64 levels, the action would require more than 128 edits, or the final replacement exceeds 2 MiB. Frontmatter discovery, YAML parsing, rewrite scans, and output writes observe caller cancellation. Serializer-internal scalar formatting is a bounded atomic region constrained by those input, materialization, nesting, and output limits; the 2 MiB value is a final output-length bound rather than an exact peak-allocation claim.

## What This Crate Owns

- Stable diagnostic IDs, severities, metadata, fixes, and source ranges.
- Plain Mermaid, Markdown, and MDX document extraction.
- UTF-8 byte, line/column, and LSP-compatible source mapping.
- Parser-backed semantic items, outline items, references, expected syntax, and provenance.
- Rule catalogs and the policy for merging parser diagnostics with recovered editor facts.
- Deterministic analysis options and explicit source-size and host-document diagram limits.

The layer boundaries are deliberate:

- `merman-core` emits structured parser facts and exact spans.
- `merman-analysis` turns those facts into diagnostics and document-level results.
- `merman-editor-core` queries typed analysis snapshots for editor behavior.
- LSP, WASM, CLI, FFI, and UniFFI only project those results into their host protocols.

## Runtime And Features

The crate has no default features and does not depend on SVG, raster, PDF, or editor protocol implementations. Analysis is deterministic by default.

The optional `system-clock`, `system-timezone`, `system-random`, and `system-timing` features only make their matching runtime adapters available. They do not change Mermaid language coverage or select ambient host state automatically.

## Options JSON

`AnalysisOptionsJson` is the shared forward-compatible configuration root. Unknown fields at the root and inside `lint` are ignored so older configuration transports can read newer additive settings. The `resources` object remains a strict versioned schema even when it is nested under the root.

Direct `serde_json` decoding of `LintOptionsJson`, `LintRuleSeverityOverrideJson`, or `ResourceOptionsJson` is intentionally strict and rejects unknown fields. Decode through `AnalysisOptionsJson` or `analysis_options_json_from_json_value` when forward compatibility is required; decode a nested type directly only when validating that exact nested schema is the goal.

## Source Mapping And Shared Text

`SourceMap` exposes behavior rather than its private adaptive line-index representation. Use `line_count()`, `line_start(index)`, and `line_bounds(index)` together with its byte, Unicode-scalar, and UTF-16 position APIs. `SourceMap::new` is a synchronous convenience constructor that scans the complete source; cancellable construction remains internal to analysis pipelines.

`SharedTextSlice` retains an `Arc<str>` plus validated UTF-8 bounds. `whole` and `from_range` share the caller's allocation, `source_arc()` clones only the `Arc`, and `to_owned_text()` is the explicit copying boundary.

## Payload Contracts

The diagnostics-only `AnalysisPayload` and richer `AnalysisFactsPayload` are independent, versioned JSON contracts. Their current public versions are both `1`; consumers must validate the version belonging to the payload they decode.

Facts use `fact_source: "unavailable"` when parser-backed body semantics do not exist. They do not invent body symbols, references, or rename targets. Current writers include `rename_policy` on each semantic item; older additive readers that do not see it must treat the item as non-renamable.

`DocumentDiagram::text`, `AnalyzedDiagram::text()`, and editor `FenceSnapshot::text()` use `SharedTextSlice`, so fence bodies share immutable document storage. `AnalysisGeneration` and `AnalyzedDiagram` are read-only canonical outputs; obtain them from `Analyzer` or the document-analysis entry points.

## Related Documentation

- [Diagnostics architecture](https://github.com/Latias94/merman/blob/main/docs/adr/0070-diagnostics-first-analysis-contract.md)
- [Lint rule governance](https://github.com/Latias94/merman/blob/main/docs/adr/0072-lint-rule-governance.md)
- [Editor and LSP capabilities](https://github.com/Latias94/merman/blob/main/docs/lsp/README.md)
- [Project compatibility status](https://github.com/Latias94/merman/blob/main/docs/alignment/STATUS.md)
