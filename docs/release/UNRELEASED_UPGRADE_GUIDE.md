# Alpha.6 Detailed Migration Reference (Source Candidate)

> This guide applies to the prepared `0.8.0-alpha.6` source candidate after `v0.8.0-alpha.5`. It does not describe the published alpha.5 artifacts. No registry package, tag, or platform artifact is implied until the exact release source passes preflight.

Start with the concise [alpha.5 to alpha.6 upgrade guide](ALPHA5_TO_ALPHA6_UPGRADE_GUIDE.md). This document retains the exhaustive symbol mapping and worked Rust examples for integrations that need a deeper migration reference.

## Rust analysis and editor migration

The alpha.6 candidate deliberately removes prerelease compatibility shims. Migrate source and generated bindings together.

| Alpha.5 or development-snapshot API | Alpha.6 replacement |
| --- | --- |
| CLI capability contract `4` and human-only failures from `--ascii-report` requests | CLI contract `5`; read the schema-1 `ascii` subcontract before rendering, and parse schema-1 Plain error JSON from stderr when report-mode invocation or rendering fails |
| `HeadlessRenderer`, `HeadlessAsciiRenderer`, root `render_svg*` functions, or CPU-bound render `async fn` wrappers | `Renderer` with one typed `RenderRequest` / `RenderTarget`; retain an `OperationControl` clone when the host must cancel stale synchronous work |
| `HeadlessAsciiError` | Match the canonical `RenderError`; use target-neutral `TerminalDiagnostic` for parser display, `TerminalRuntimePolicyError` for runtime-policy display, and `ascii::AsciiDiagnostic` only for ASCII target-local failures |
| `RenderError::Parse(merman::Error)` or raw `merman::Error` display in a terminal host | `RenderError::Parse(TerminalDiagnostic)`; direct parser hosts should wrap an error with `TerminalDiagnostic::from(error)` and read `terminal_diagnostic_details()` for bounded code/span/field/diagram-type context |
| `RenderError::RuntimePolicy(RuntimePolicyError)` | `RenderError::RuntimePolicy(TerminalRuntimePolicyError)`; capability classification remains available through `missing_capability()`, while display/debug output is bounded and terminal-safe |
| `ascii::AsciiDiagnosticDetails` or parse codes under `merman.ascii.*` | `TerminalDiagnosticDetails`; parser diagnostics now use the target-neutral `merman.parse.*` namespace, while ASCII target-local codes remain under `merman.ascii.*` |
| `AsciiRenderOptions::resources`, `with_resource_policy(...)`, `with_resource_profile(...)`, or `with_resource_limit(...)` | Keep presentation settings in `AsciiRenderOptions`; set `AsciiRequest::resources` for facade rendering, or pass an explicit `AsciiResourcePolicy` as the fourth argument to `AsciiRenderer::render_model` |
| `ClassRelation::relation_title_1` / `relation_title_2` as `String`, with `"none"` meaning no endpoint label | `Option<String>`; use `None` for an absent label and `Some("none".into())` for authored text. Mermaid compatibility JSON still projects absence as `"none"`, so use the typed model rather than compatibility JSON for lossless round trips |
| `PreparedSemantic`, public SVG `PreparedRender`, or SVG-owned `HeadlessOperation` | Format-neutral `SemanticArtifact`, consumed once by a typed SVG, ASCII, layout, or export target |
| `ParseControl`, `ParseCancelled`, or `ParseControlResult` | `OperationControl`, `OperationCancelled`, and `OperationControlResult`; analysis may keep its domain token but it shares the same operation state |
| Direct UniFFI binding API `3`, `4`, or `5` generated Swift/Python plus the matching native library | Regenerate against UniFFI binding API `6`, replace the old version probe with `binding_api_version_v6` / `bindingApiVersionV6`, keep generic requests on `MermanOperationRequestV4`, and deploy the generated projection and native library together. API 6 capability records add layout/width/encoding/fallback admission arrays, and ASCII output plans use schema `2` with explicit encoding. Earlier prerelease changes also include lint tags, structured diagnostics, and optional operation controls. |
| Web transport API `3` or `4` one-shot options | Web transport API `5`; use top-level `timeout_ms` for a cooperative monotonic deadline, ignore stale results after return, and use a Worker or process boundary when hard termination is required |
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
| `VendoredFontMetricsTextMeasurer`, `TextMeasurementPolicy::parity()`, or `RenderEnvironment::parity()` | `DeterministicTextMeasurer`, `TextMeasurementPolicy::deterministic()`, or `RenderEnvironment::deterministic()`; install a host callback when layout must use the final display stack |
| Options JSON `environment.text_measurement` value `vendored` or `parity` | `deterministic`; the removed names are rejected rather than retained as aliases |
| Runtime text-measurement provider ID `vendored` | `deterministic`; host-capable products also advertise `host-callback`, while Typst advertises deterministic only |
| CLI/xtask `--text-measurer`, `--flowchart-text-measurer`, or `measure-text --measurer` | Remove the option; these command paths always use deterministic measurement |

The binding-result envelope remains version `1` because its JSON shape is unchanged. Consumers
that match `details.diagnostic.code` must update parser-code expectations from `merman.ascii.*` to
`merman.parse.*`; this development-snapshot namespace migration is not a payload-schema change.

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

The built-in deterministic measurer no longer embeds the bounded Headless Chrome 131 font tables.
It uses font-agnostic Unicode-width and wrapping rules, so geometry may change where a previous
table supplied browser-specific advances, kerning, baseline facts, or quantization. Treat this as a
breaking output change: use the host callback when the final font stack is authoritative, and do
not copy browser values back into production lookup tables. Swimlane identifier tie-breaks are now
stable UTF-16 code-unit order; locale-sensitive coordinate differences for mixed-case, accented,
or non-Latin identifiers are a documented browser residual rather than an ICU runtime dependency.

```rust
use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};

let control = OperationControl::new();
let host_control = control.clone();
let render_thread = std::thread::spawn(move || {
    Renderer::new().render(RenderRequest::svg(
        "flowchart TD\nA --> B",
        control,
        SvgRequest::default(),
    ))
});

// A host event, stale-revision check, or another thread may cancel the in-flight render.
host_control.cancel();

match render_thread.join().expect("render thread panicked") {
    Err(RenderError::Cancelled(_)) => {}
    Ok(RenderOutput::Svg(Some(svg))) => println!("{}", svg.svg()),
    Ok(_) => return Err("no Mermaid diagram found".into()),
    Err(error) => return Err(error.into()),
}
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
