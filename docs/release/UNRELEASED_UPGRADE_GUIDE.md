# Unreleased Upgrade Guide

> This guide applies only to source revisions after `v0.8.0-alpha.5`. It does not describe the
> published alpha.5 artifacts. The next release version has not been selected.

## Rust analysis and editor migration

The unreleased branch deliberately removes prerelease compatibility shims. Migrate source and
generated bindings together.

| Alpha.5 or development-snapshot API | Unreleased replacement |
| --- | --- |
| Options JSON schema `2` with `presentation`, `raster.background`, `pdf.background`, or general-binding raw CSS | Options JSON schema `3`; use top-level typed `theme`, `raster.matte`, and `pdf.page_paint`. Trusted Rust/native CLI hosts keep explicit postprocessing/CSS escape hatches outside the general binding contract. Regenerate SDK projections and deploy them with a runtime catalog that advertises schema `3`; schema `2` is rejected rather than partially translated. |
| Development-snapshot public `FilterRegion`, `EffectGraph::region()`, `EffectGraph::new(id, region, primitives)`, or `theme.spec.effects[].region` | `FilterRegion` and `EffectGraph::region()` are removed. Use `EffectGraph::new(id, primitives)` and omit `region` from binding JSON. The consuming family derives each terminal filter region from final paint geometry and admits it against the effective session theme-resource policy. The closed schema rejects the removed field; there is no compatibility decoder. Filter-region magnitude rejections now report phase `effect_materialize` instead of the former compile-time `effect_compile`, because the final geometry is not known until family materialization. |
| `RenderEnvironment::begin_session*` returning only `RuntimePolicyError`, or an in-context overload returning `RenderSession` directly | Every session constructor now returns `Result<RenderSession, RenderEnvironmentError>`. Handle runtime-policy failures, cooperative cancellation/deadline expiry, and retained theme-resource rejection; in-context callers must propagate or classify the new error instead of assuming session creation is infallible. Caller-owned cancellation is checked before retained-resource validation and again before text-layout preparation. |
| CLI capability contract `4` and human-only failures from `--ascii-report` requests | CLI contract `5`; read the schema-1 `ascii` subcontract before rendering, and parse schema-1 Plain error JSON from stderr when report-mode invocation or rendering fails |
| Direct UniFFI binding API `3`, `4`, `5`, or `6` generated Swift/Python plus the matching native library | Regenerate against UniFFI binding API `7`, replace the old version probe with `binding_api_version_v7` / `bindingApiVersionV7`, keep generic requests on `MermanOperationRequestV4`, and deploy the generated projection and native library together. API 7 preserves the API 6 ASCII capability/output-plan contract and additionally protects the merged `MermanDiagramFamilyCapability.family_id` record shape. |
| `HeadlessRenderer`, `HeadlessAsciiRenderer`, root `render_svg*` functions, or CPU-bound render `async fn` wrappers | `Renderer` with one typed `RenderRequest` / `RenderTarget`; retain an `OperationControl` clone when the host must cancel stale synchronous work |
| `HeadlessAsciiError` | Match the canonical `RenderError`; use target-neutral `TerminalDiagnostic` for parser display, `TerminalRuntimePolicyError` for runtime-policy display, and `ascii::AsciiDiagnostic` only for ASCII target-local failures |
| `RenderError::Parse(merman::Error)` or raw `merman::Error` display in a terminal host | `RenderError::Parse(TerminalDiagnostic)`; direct parser hosts should wrap an error with `TerminalDiagnostic::from(error)` and read `terminal_diagnostic_details()` for bounded code/span/field/diagram-type context |
| `RenderError::RuntimePolicy(RuntimePolicyError)` | `RenderError::RuntimePolicy(TerminalRuntimePolicyError)`; capability classification remains available through `missing_capability()`, while display/debug output is bounded and terminal-safe |
| Exhaustive matching on `merman_core::Error` or its `merman::Error` facade re-export | The error enum is non-exhaustive. Keep a wildcard arm. Low-frequency engine faults use `Error::Internal(InternalFailure)` rather than an authored Mermaid parse error. Treat the message as diagnostic context, not a stable subsystem identifier. |
| `ascii::AsciiDiagnosticDetails` or parse codes under `merman.ascii.*` | `TerminalDiagnosticDetails`; parser diagnostics now use the target-neutral `merman.parse.*` namespace, while ASCII target-local codes remain under `merman.ascii.*` |
| `AsciiRenderOptions::resources`, `with_resource_policy(...)`, `with_resource_profile(...)`, or `with_resource_limit(...)` | Keep presentation settings in `AsciiRenderOptions`; set `AsciiRequest::resources` for facade rendering, or pass an explicit `AsciiResourcePolicy` as the fourth argument to `AsciiRenderer::render_model` |
| `ClassRelation::relation_title_1` / `relation_title_2` as `String`, with `"none"` meaning no endpoint label | `Option<String>`; use `None` for an absent label and `Some("none".into())` for authored text. Mermaid compatibility JSON still projects absence as `"none"`, so use the typed model rather than compatibility JSON for lossless round trips |
| `PreparedSemantic`, public SVG `PreparedRender`, or SVG-owned `HeadlessOperation` | Format-neutral `SemanticArtifact`, consumed once by a typed SVG, ASCII, layout, or export target |
| `ParseControl`, `ParseCancelled`, or `ParseControlResult` | `OperationControl`, `OperationCancelled`, and `OperationControlResult`; analysis may keep its domain token but it shares the same operation state |
| Web transport API `3` or `4` wrapper/WASM pairs | Web transport API `5`; use top-level `timeout_ms`, introduced in API `4`, for a cooperative monotonic deadline, remove calls to the semantic-token exports dropped in API `5`, and use Tree-sitter for syntax highlighting. Ignore stale results after return, and use a Worker or process boundary when hard termination is required. |
| Web `DiagramFamilyCapability.metadata_id: DiagramType | null` | Treat `metadata_id` as the open `DiagramMetadataId` string domain. Call `tryAsKnownDiagramType()` before passing an ID to a current-package execution API; additive future IDs remain discoverable in the catalog while `supportedDiagrams()` continues to return only executable `DiagramType` values. |
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

### Theme authoring Alpha corrections

Development snapshots briefly exposed `ThemeMaterializer` as a public constructor. It is now a
private deterministic lowering step behind the host-admitted operations below:

| Previous development API | Replacement |
| --- | --- |
| `ThemeMaterializer::new().materialize_theme(&definition)` | `materialize_theme(&definition)` |
| `ThemeMaterializer::new().materialize_theme_json(bytes)` | `materialize_theme_json(bytes)` |
| materialization with a caller resource policy | `materialize_theme_with_resource_policy(&definition, &policy)` or `materialize_theme_json_with_resource_policy(bytes, &policy)` |

The replacement functions preserve one shared typed/JSON admission authority before invoking the
pure lowering step. Do not recreate a public materializer wrapper, because that would reopen a path
around host resource admission.

The unreleased `ThemeDefinitionV1` contract is still pre-freeze. Development snapshots that used
`family = "er", target = "requirement"` must use `family = "er", target = "entity"`; the
`requirement` target is now reserved for Requirement diagrams. There is no compatibility alias,
because keeping both target meanings would preserve a second semantic authority.

Development-snapshot Playground hashes that carried `hostThemePreset` must be regenerated from a
current workspace snapshot. Current hashes encode `themePresetId` and `svgPipeline` explicitly;
the unpublished alpha.4 field and the public `migrateLegacyHostTheme` helper were removed rather
than retained as a second theme-selection model.

The candidate token object also removes `subtle_text`, `error`, `warning`, and `success`. After
row-coverage pruning those fields had no generated direct consumer, so accepting them would produce
a shareable definition whose materialized appearance did not change. Use explicit authored rules
for those colors until a later expansion version introduces a proven semantic role.

Expansion version 1 now emits only rows with a direct family consumer and terminal witness: 23
generated rules followed by the `node` and `pie-slice` ordinal palettes. The derived authored-rule
budget is therefore 489, and the 490th authored rule is rejected before materialization. Definitions
persisted from a pre-freeze development snapshot should be re-materialized and must not compare old
development-only materialization digests or generated rule indices as stable identities. The
current version 1 success wire does not publish a materialization digest.

`Renderer` is the only source-to-output operation owner. Target-local service configuration stays
inside `SvgRequest` or `AsciiRequest`, while runtime policy, input admission, cancellation, and the
monotonic deadline belong to the renderer/request operation. Resource exhaustion and cancellation
remain distinct errors and neither returns partial output.

The built-in deterministic measurer no longer embeds the bounded browser font tables removed from
the production closure. It uses font-agnostic Unicode-width and wrapping rules, so geometry may
change where the previous tables supplied browser-specific advances, kerning, baseline facts, or
quantization. Treat this as a breaking output change: use a host callback when the final font stack
is authoritative, and do not copy browser values back into production lookup tables. Swimlane
identifier tie-breaks are stable UTF-16 code-unit order; locale-sensitive coordinate differences for
mixed-case, accented, or non-Latin identifiers are a documented browser residual rather than an
ICU runtime dependency.

Development snapshots briefly exposed renderer-internal theme proof ledgers through types such as
`RootThemeMechanismKey`, `RootThemeMechanismEvidence`, `RootThemeResidual`, `RootThemeReport`, and
`FamilyRenderReport`, and exposed them through output accessors such as `family_report()`. Those
fine-grained types and accessors are no longer public. Read the completed target's `evidence()` and
use `RenderEvidence::theme_evidence()` for the alpha coarse `ThemeEvidenceSummary`; mechanism
keys, selector identities, residual entries, and family-local receipts intentionally have no public
replacement.

`RenderEvidence::execution_path()` was also removed. `Renderer` is now the canonical execution
owner, so callers should use `family_id()`, `operation_context()`, measurement provenance, and the
coarse theme evidence rather than branching on an internal path taxonomy.

Development snapshots that used `FinalizedSvgOutput`, `RenderTarget::FinalizedSvg`,
`RenderOutput::FinalizedSvg`, or `RenderRequest::finalized_svg` must migrate to
`RenderedDocument`, `RenderTarget::Document`, `RenderOutput::Document`, and
`RenderRequest::document`. A document freezes one terminal SVG and can project PNG, JPEG, and PDF
without repeating parse, layout, or SVG finalization. `RasterOutput` and `PdfOutput` no longer
expose mutable `bytes` / `plan` fields or decomposable `into_parts()` tuples; use `bytes()`,
`plan()`, and `admission()` while evidence must remain attached, or the explicitly lossy
`into_bytes()` when only encoded bytes cross the next boundary.

`RenderedDocument` is no longer `Clone`: cloning it copied the entire sealed SVG and made an
apparently cheap evidence snapshot scale with document size. Borrow one document while preparing
or exporting targets. Hosts that must inspect allocation before encoding can call
`prepare_png_export`, `prepare_jpeg_export`, or `prepare_pdf_export`, read the frozen report, then
call `encode()`.

`TargetAdmissionReceipt::reasons()` now returns the non-exhaustive `TargetAdmissionReason` enum
rather than raw strings. Use `TargetAdmissionReason::id()` only at serialization or logging
boundaries. `RenderTarget` and `RenderOutput` are also non-exhaustive; external matches must retain
a wildcard arm so additive targets do not become source-breaking.

Document completion is target-neutral. It always retains the standalone-SVG receipt, even when
that receipt is `HostDependent` or `Rejected`; inspect
`RenderedDocument::standalone_svg_admission()` before
publishing the SVG. PNG, JPEG, and PDF projections independently enforce the request's portability
requirement from their own target-owned receipts, so the SVG decision cannot preempt another
target.

`RenderedDocument::document_digest()` and
`TargetAdmissionReceipt::target_evidence_digest()` are alpha correlation identities. Their
canonical encodings are explicit and deterministic within this API revision, but they are not yet
cross-release cache keys and must not be described as stable before the C7a contract gate.

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

Duplicate raw Flowchart edge IDs are occurrence-safe across the Dagre, ELK, and Swimlane adapters.
Layout transport IDs remain private while the rendered SVG preserves Mermaid's raw edge ID, so
geometry, styles, labels, markers, curves, and line hops stay bound to the semantic occurrence.

The render-only `FlowchartLayout::edge_owner_indices` sidecar is no longer public. Downstream Rust
code must not construct layout structs with or inspect naked semantic indices; treat
`FlowchartLayout` and `SwimlaneLayout` as renderer-owned results and consume their public geometry
fields instead. This is an intentional alpha source correction and does not change serialized JSON.

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
