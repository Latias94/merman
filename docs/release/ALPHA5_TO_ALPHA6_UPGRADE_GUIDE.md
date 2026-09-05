# Upgrade from 0.8.0-alpha.5 to 0.8.0-alpha.6

> [!IMPORTANT]
> Alpha.6 is published for the workspace crates and CLI/LSP archives from immutable tag `v0.8.0-alpha.6`. Web, Node.js, Flutter, Python, Apple, Android, and Typst remain independent publication tracks; verify the matching channel before installing a generated binding or native/WebAssembly artifact.

Alpha.6 is intentionally breaking across the Rust rendering, analysis/editor, and native transport surfaces. The migration is organized by contract owner so a host can update one boundary at a time without relying on compatibility aliases that no longer exist.

For a symbol-by-symbol compatibility table and longer Rust examples, see the [detailed alpha.6 migration reference](UNRELEASED_UPGRADE_GUIDE.md).

## Rust rendering and ASCII API

- Replace `HeadlessRenderer`, `HeadlessAsciiRenderer`, root `render_svg*` helpers, public SVG `PreparedRender` stages, and CPU-bound render `async fn` wrappers with one operation-scoped `Renderer` and typed `RenderRequest` / `RenderTarget` values; retain a cloneable `OperationControl` when the host must cancel stale synchronous work.
- Replace `PreparedSemantic` and SVG-owned preparation handles with the format-neutral `SemanticArtifact`, which is consumed once by a typed SVG, ASCII, layout, or export target.
- Move ASCII resource ceilings out of `AsciiRenderOptions`; configure `AsciiRequest::resources` for facade rendering or pass an explicit `AsciiResourcePolicy` to `AsciiRenderer::render_model`.
- Replace the former single grid ceiling with independent limits for grid cells, layout work, document cells, encoded output bytes, grapheme bytes, and nesting depth; select a named profile with `AsciiResourcePolicy::for_profile(...)` and override one limit with `with_limit(...)`.
- Replace `AsciiError::RenderLimitExceeded` with `AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded)` and match its typed `limit`, `actual`, `max`, `profile`, and `phase()` fields; post-admission allocation failures use `AsciiError::AllocationFailed`.
- Source-to-output ASCII rendering now uses `Renderer::render(RenderRequest::ascii(...))`; typed-model callers must supply `OperationControl`, `OperationContext`, and `AsciiResourcePolicy` explicitly.
- `AsciiRenderOptions` carries an explicit `terminal_width_profile`; use the constructors/builders rather than assuming Unicode and CJK display widths are interchangeable.
- Ordinary Flowchart node labels wrap before layout at the default terminal width of `40` display cells; tune the behavior with `with_flowchart_node_label_wrap_width(...)` or the binding JSON aliases `ascii.flowchart_node_label_wrap_width` and `ascii.flowchartNodeLabelWrapWidth`.
- Flowchart, Class, and ER viewport fallback now rolls back speculative primary document-cell charges while retaining the layout work used to prove overflow; resource-limited fallback callers should refresh exact-boundary tests.
- Long Flowchart label wrapping observes operation cancellation between measurement probes; cancellation remains distinct from resource exhaustion and never returns partial output.

## Typed render models

- `FlowEdge` now carries independent `start_marker`, `end_marker`, `stroke_kind`, and `visibility` fields in addition to the authored arrow; Rust struct literals must initialize the complete shape.
- `ErDiagramRenderModel::{classes, entities}` use declaration-ordered `indexmap::IndexMap` instead of `BTreeMap`; update annotations, constructors, and helper signatures.
- `GanttRenderTask` carries typed `start_constraint` and `end_constraint` fields; initialize them directly with `GanttTaskStartConstraint` and `GanttTaskEndConstraint` values instead of recovering constraints from compatibility `raw` strings.
- `SequenceDiagramRenderModel` carries `actor_lifecycles` aligned with `actor_order`; direct models normally use `None`, while parser-produced models preserve the signal that consumed each pending create or destroy request.
- `MindmapDiagramRenderNode::id` is the internal topology key and `node_id` is the authored identity disclosed in StructuredText; every direct model must provide a unique, non-empty `node_id`.
- `TimelineRenderTask`, `JourneyRenderTask`, and `GanttRenderTask` carry an optional `section_index` occurrence owner; set it when repeated section labels are ambiguous, and expect undeclared or empty sections to remain visible in structured output.

## Analysis, editor, and parser migration

- Match `DiagramParseOutcome::Parsed { model, warning_facts }` instead of the removed tuple-like `Parsed(Value)` variant, and consume parser-owned typed warning facts rather than decoding `warningFacts` from compatibility JSON.
- Regenerate analysis-facts consumers for schema `2`; the Flowchart-only rich graph field is removed, semantic roles are now the explicit `entity`, `class_definition`, `reference`, `outline`, and `payload` set, and diagnostics remain schema `1`.
- Remove parser-emitted `EditorLexeme*`, mixed token-planner, semantic-token descriptor, packed token-equivalence, and Web/WASM semantic-token APIs; syntax highlighting now belongs to the canonical Tree-sitter grammar and query.
- Replace `FenceCursorCompletionKind`, `FenceCursorContext`, `CompletionContext`, `EditorCompletionCandidate`, and `EditorCompletionVocabulary` with typed snapshots, `completion_for_snapshot`, `EditorFamilySemantics`, and `expected_syntax`; use the editor-owned `COMPLETION_TRIGGER_CHARACTERS` list in adapters.
- Update exhaustive `EditorExpectedSyntaxKind` matches: `Operator` becomes `FlowchartOperator`, `DirectionValue` becomes the owning `FlowchartDirectionValue`, `CardinalDirectionValue`, or `BlockDirectionValue`, and new slots include `Directive`, `Frontmatter`, `ClassName`, `StyleValue`, and `InteractionAction`.
- Replace stateful `DocumentWorkspace` and `DocumentAnalysisOutcome` with `analyze_document_snapshot_with_shared_text(...)` or `analyze_document_context_with_shared_text(...)`; keep URI, revision, cancellation, and document storage in the host and pass caller-owned `Arc<str>` source text.
- Use exact lowercase analysis profile/severity values (`core`, `recommended`, `strict`; `error`, `warning`, `info`, `hint`); the former `warn` alias and case-folded values are rejected.
- Negotiate `merman/configSchema` response version `2` and require its typed `constraints` projection; a version-1-only client must decline rather than partially decode the response.

## UniFFI, WebAssembly, and Android transports

- Regenerate Apple and Python bindings against UniFFI binding API `6`; replace `binding_api_version_v5` / `bindingApiVersionV5` with `binding_api_version_v6` / `bindingApiVersionV6`, and deploy the generated projection with the matching native library. API 6 adds ASCII layout/width/encoding/fallback admission arrays and schema-2 output-plan encoding.
- Keep generic UniFFI requests on `MermanOperationRequestV4`; optional `MermanOperationControl` carries cooperative cancellation and relative deadlines, while cancellation details remain separate from resource-limit details.
- Upgrade Web and WASM artifacts to transport API `5`; transport-dispatched requests accept top-level `timeout_ms`. Same-realm execution is cooperatively cancellable, while hard interruption requires a Worker or process boundary.
- Upgrade Android JNI transport API `1` to API `2` and replace the Kotlin classes and `libmerman_android_jni.so` together; API 2 owns the opaque operation-control registry and exact resource/cancellation detail projections.
- The native C ABI remains the alpha.5 ABI 3 contract; do not mix an older generated header/table with an alpha.6 library even when the frozen prefix happens to load.

Generated bindings and native artifacts are version-coupled. Runtime catalogs and binding probes are expected to reject mixed API versions before decoding changed records; do not add local aliases for the removed probes.

## Flutter, Typst, and package-channel migration

- Flutter now uses Dart `package_ffi` and Native Assets with Dart `3.10` / Flutter `3.38` minimums; legacy plugin registrars, platform wrapper glue, and `openMermanLibrary()` are removed, while `Merman.open()` remains the default facade.
- The default Android, Apple, Python, and Flutter artifacts bundle SVG, Cytoscape/ELK layout, ASCII, analysis, validation, and document analysis, while omitting math, PNG, JPEG, PDF, and native clock/time-zone/random adapters; inspect the runtime catalog before calling optional operations.
- Typst package `0.3.0` is independently versioned and was published to Typst Universe on 2026-09-01 from the alpha.6 source line; its availability is still separate from the workspace tag and must not be inferred for other channels.
- The alpha.6 source README and package changelogs distinguish the published workspace/crates.io/CLI surfaces from independent alpha.5 channels; a registry install on one channel is not evidence that another package has been rebuilt from the alpha.6 source revision.

## Capability and output compatibility

- Read both ASCII capability dimensions: `semantic_coverage` describes semantic completeness and `primary_projection` distinguishes diagrammatic output from structured text; `support_level` is derived, and `summary_fallback` is renamed to `structured_text_fallback`.
- Structured resource diagnostics expose typed limit, phase, cause, observed, and maximum fields; classify failures from those fields rather than parsing display text.
- ASCII snapshots can change even when Mermaid source is unchanged because Flowchart edge markers, graph direction, compound ownership, parallel/self-loop routing, Sequence controls, Class/ER endpoint roles, State notes, XYChart coordinates, and declaration order now follow source-backed semantics.
- Flowchart SVG `diagramPadding` is applied directly, including zero and fractional values; refresh SVG viewBox snapshots that relied on the former family-local one-pixel paint guard.
- Deterministic SVG text measurement is font-agnostic and no longer uses the removed vendored metric tables; use a host text-measurement callback when the final display font stack is authoritative.
- Unsupported diagram families continue to fail explicitly; structured-text projections preserve typed field paths and do not silently truncate authored values.

## Recommended upgrade sequence

1. Update Rust imports and typed model constructors, then run the renderer and analysis tests.
2. Regenerate analysis facts, UniFFI projections, Web/WASM glue, and Android JNI sources from the alpha.6 source revision.
3. Refresh ASCII/SVG snapshots and capability decoders, paying particular attention to viewBox padding and structured-text fallback metadata.
4. Install the matching native/WebAssembly artifact in each host and verify runtime-catalog identity, binding/transport API versions, and resource schemas before enabling optional outputs.
5. Treat registry package versions and independent package workflows as separate publication events; the workspace tag confirms only the workspace/crates.io/CLI release surfaces, not every alpha.6 package channel.
