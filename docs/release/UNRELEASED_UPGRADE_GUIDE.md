# Unreleased Upgrade Guide

> This guide applies only to source revisions after `v0.8.0-alpha.5`. It does not describe the
> published alpha.5 artifacts. The next release version has not been selected.

## Rust analysis and editor migration

The unreleased branch deliberately removes prerelease compatibility shims. Migrate source and
generated bindings together.

| Alpha.5 or development-snapshot API | Unreleased replacement |
| --- | --- |
| Analysis facts schema `1` with Flowchart-only graph facts | Analysis facts schema `2` with generic parser/editor facts; diagnostics remain schema `1` |
| `FenceCursorCompletionKind`, `FenceCursorContext`, or `CompletionContext` | `completion_for_snapshot` over parser-backed typed facts |
| Adapter-owned completion trigger lists | `COMPLETION_TRIGGER_CHARACTERS` |
| Case-folded profile/severity values or `warn` | Exact lowercase analysis-owned values, including `warning` |
| `DocumentWorkspace::upsert(...)` | `analyze_document_snapshot_with_shared_text(...)` and caller-owned document storage |
| `DocumentWorkspace::build_analysis_context_with_shared_text(...)` | `analyze_document_context_with_shared_text(...)` |
| `DocumentAnalysisOutcome` | `Result<DocumentAnalysisContext, AnalysisRejection>` |

The one-shot editor functions accept caller-owned `Arc<str>` source text. Standalone Mermaid,
Markdown, and MDX inputs still use their corresponding analysis pipelines, but editor-core no
longer owns a URI map, analyzer replacement, or document CRUD lifecycle.

Cancellation and admission rejection remain intentionally distinct:

```rust
use merman_analysis::{AnalysisCancellationToken, Analyzer};
use merman_editor_core::{
    DocumentKind, analyze_document_context_with_shared_text_cancellable,
};
use std::sync::Arc;

let cancellation = AnalysisCancellationToken::new();
let operation = analyze_document_context_with_shared_text_cancellable(
    &Analyzer::new(),
    "file:///workspace/diagram.mmd",
    1,
    Arc::from("flowchart TD\nA --> B\n"),
    DocumentKind::Diagram,
    &cancellation,
);

match operation {
    Err(_) => eprintln!("cancelled"),
    Ok(Err(rejection)) => eprintln!("rejected: {:?}", rejection.resource_limit()),
    Ok(Ok(context)) => println!("{}", context.snapshot().uri().as_str()),
}
```

Do not add a local compatibility wrapper that restores `DocumentWorkspace`. Hosts that need
stateful document management should keep it alongside their own revision, cancellation, and
transport state and store the immutable `DocumentSnapshot` or `DocumentAnalysisContext` returned
by the one-shot call.
