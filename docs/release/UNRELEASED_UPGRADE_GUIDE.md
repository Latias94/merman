# Unreleased Upgrade Guide

> This guide applies only to source revisions after `v0.8.0-alpha.5`. It does not describe the
> published alpha.5 artifacts. The next release version has not been selected.

## Rust analysis and editor migration

The unreleased branch deliberately removes prerelease compatibility shims. Migrate source and
generated bindings together.

| Alpha.5 or development-snapshot API | Unreleased replacement |
| --- | --- |
| `HeadlessRenderer`, `HeadlessAsciiRenderer`, root `render_svg*` functions, or CPU-bound render `async fn` wrappers | `Renderer` with one typed `RenderRequest` / `RenderTarget`; retain an `OperationControl` clone when the host must cancel stale synchronous work |
| `HeadlessAsciiError` | Match the canonical `RenderError`; use `ascii::AsciiDiagnostic` only when projecting parse, runtime-policy, or target failures onto an untrusted terminal surface |
| `AsciiRenderOptions::resources`, `with_resource_policy(...)`, `with_resource_profile(...)`, or `with_resource_limit(...)` | Keep presentation settings in `AsciiRenderOptions`; set `AsciiRequest::resources` for facade rendering, or pass an explicit `AsciiResourcePolicy` as the fourth argument to `AsciiRenderer::render_model` |
| `PreparedSemantic`, public SVG `PreparedRender`, or SVG-owned `HeadlessOperation` | Format-neutral `SemanticArtifact`, consumed once by a typed SVG, ASCII, layout, or export target |
| `ParseControl`, `ParseCancelled`, or `ParseControlResult` | `OperationControl`, `OperationCancelled`, and `OperationControlResult`; analysis may keep its domain token but it shares the same operation state |
| Direct UniFFI binding API `3` generated Swift/Python plus the matching native library | Regenerate against UniFFI binding API `4`, replace `binding_api_version` / `bindingApiVersion` with `transport_api_version` / `transportApiVersion`, rename generic requests to `MermanOperationRequestV4`, and deploy the generated projection and native library together; API 4 lint rule records require `tags` and generic requests may carry `MermanOperationControl` |
| Web transport API `3` one-shot options | Web transport API `4`; use top-level `timeout_ms` for a cooperative monotonic deadline, ignore stale results after return, and use a Worker or process boundary when hard termination is required |
| Analysis facts schema `1` with Flowchart-only graph facts and the former semantic-role set | Analysis facts schema `2` with generic parser/editor facts and the explicit `entity`, `class_definition`, `reference`, `outline`, and `payload` roles; update exhaustive role handling, while diagnostics remain schema `1` |
| `FenceCursorCompletionKind`, `FenceCursorContext`, or `CompletionContext` | `completion_for_snapshot` over parser-backed typed facts |
| Adapter-owned completion trigger lists | `COMPLETION_TRIGGER_CHARACTERS` |
| `EditorCompletionCandidate`, `EditorCompletionVocabulary`, `EditorSemanticFacts::completion_vocabulary`, or `with_completion_vocabulary(...)` | Keep parser evidence in `EditorSemanticFacts::family_semantics` / `expected_syntax`; call editor-core `completion_for_snapshot` for candidate labels, details, snippets, and edits |
| `EditorExpectedSyntaxKind::Operator` | `EditorExpectedSyntaxKind::FlowchartOperator` |
| `EditorExpectedSyntaxKind::DirectionValue` | Use the owning family slot: `FlowchartDirectionValue`, `CardinalDirectionValue`, or `BlockDirectionValue`; new typed authoring slots also include `Directive`, `Frontmatter`, `ClassName`, `StyleValue`, and `InteractionAction` |
| Case-folded profile/severity values or `warn` | Exact lowercase analysis-owned values, including `warning` |
| `merman/configSchema` response version `1` | Response version `2` with mandatory typed `constraints`; clients that only understand version `1` must decline the response rather than partially parsing it |
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

## Rust rendering migration

`Renderer` is the only source-to-output operation owner. Target-local service configuration stays
inside `SvgRequest` or `AsciiRequest`, while runtime policy, input admission, cancellation, and the
monotonic deadline belong to the renderer/request operation. Resource exhaustion and cancellation
remain distinct errors and neither returns partial output.

```rust
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

let control = OperationControl::new();
let host_control = control.clone();
let output = Renderer::new().render(RenderRequest::svg(
    "flowchart TD\nA --> B",
    control,
    SvgRequest::default(),
))?;

let RenderOutput::Svg(Some(svg)) = output else {
    return Err("no Mermaid diagram found".into());
};
println!("{}", svg.svg());

// Another thread or task may call this while the synchronous render is running.
host_control.cancel();
# Ok::<(), Box<dyn std::error::Error>>(())
```

Cancellation is cooperative. Merman checks the same control through parse, semantic projection,
layout adapters, ASCII/SVG emission, postprocessing, and export boundaries. An opaque host callback
or third-party encoder may finish its current call before the next checkpoint.

ASCII resource limits belong to the request or caller-owned typed-model operation, not to reusable
render options:

```rust
use merman::ascii::{AsciiRenderOptions, AsciiResourcePolicy};
use merman::{AsciiRequest, OperationControl, RenderRequest, Renderer};

let request = AsciiRequest {
    options: AsciiRenderOptions::ascii(),
    resources: AsciiResourcePolicy::default(),
};
let output = Renderer::new().render(RenderRequest::ascii(
    "flowchart TD\nA --> B",
    OperationControl::new(),
    request,
))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
