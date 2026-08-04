# Upgrade from 0.8.0-alpha.3 to 0.8.0-alpha.4

> [!IMPORTANT]
> This guide describes the alpha.4 source contract. Package registries and release channels can
> trail the repository, so verify the installed version before relying on an alpha.4 API or
> capability. Final release benchmarks must be regenerated against the tagged release commit.

Alpha.4 is a broad prerelease upgrade, not a drop-in patch. It expands the Mermaid baseline to
11.16, admits all 35 diagram families, replaces implementation-oriented feature bundles with
observable capabilities, splits the browser SDK into standalone packages, and moves native hosts
to ABI 3.

The practical upgrade rule is:

1. choose the user workflow first;
2. select the package or binary that owns that workflow;
3. enable only the capabilities that workflow needs; and
4. query the installed artifact when optional capability availability matters at runtime.

## Who needs to change something

| If you use... | Required action |
| --- | --- |
| Default `merman` Rust features | Update the version and retest. The default remains the complete SVG product through `complete-svg`. |
| Explicit Cargo features | Replace removed alpha.3 names with alpha.4 capability leaves. There are no compatibility aliases. |
| `merman-cli` root `-i/-o` flags | Existing scripts still route to the compatibility parser, but new scripts should choose `render`, `batch`, or `mmdc` explicitly. |
| `@mermanjs/web/<subpath>` or `@mermanjs/web/pkg/**` | Replace the import with one standalone browser package. Subpaths and raw WASM files are no longer public API. |
| Native C, Flutter, or Android bindings | Rebuild or upgrade the complete host package and migrate from ABI 2 to ABI 3. Reject an ABI mismatch during initialization. |
| Python or Apple bindings | Upgrade the generated UniFFI wrapper and matching native artifact together; do not mix alpha.3 and alpha.4 components. |
| Analysis, editor, or LSP APIs | Follow the [Rust and embedding API migration](#rust-and-embedding-api-migration) section for exact type, method, ownership, and capability replacements. |
| Node.js or SSR | Continue to invoke `merman-cli` as a subprocess. No in-process Node package is admitted for alpha.4. |
| Typst | Treat it as an independent release track. The published `@preview/merman:0.2.0` package is not an alpha.4 artifact. |

## Choose the alpha.4 surface

| Workflow | Use | Selection and tradeoff |
| --- | --- | --- |
| Parse Mermaid into Rust models | `merman-core` | Smallest foundational API; no rendering or diagnostics product surface. |
| Lint, diagnose, or scan Markdown/MDX in Rust | `merman-analysis` | Analysis without renderer, layout, export, icon, or network dependencies. |
| Render complete SVG in Rust | `merman` | Use the default or `complete-svg`; includes SVG, Cytoscape, ELK, and math. |
| Render basic deterministic SVG in Rust | `merman` | Disable defaults and select `svg`; optional layout engines and math remain absent. |
| Convert, export, lint, or batch from a shell | `merman-cli` | The release binary is the complete product; source builds can select narrower feature leaves. |
| Run a language server | `merman-lsp` | Use the release binary, or build the explicit `stdio` transport. |
| Render in a browser | `@mermanjs/web-render` | Complete SVG/layout/math without analysis, editor, or ASCII APIs. |
| Analyze in a browser | `@mermanjs/web-analysis` | Diagnostics, facts, and detection without rendering. |
| Provide browser editor intelligence | `@mermanjs/web-editor` | Analysis plus parser-backed editor APIs; intended for a dedicated Worker. |
| Render ASCII in a browser | `@mermanjs/web-ascii` | ASCII/Unicode only; family support is capability-graded. |
| Need all browser capabilities in one realm | `@mermanjs/web` | Full browser SDK; avoid combining it with duplicate slim packages. |
| Embed a prebuilt native SDK | The Python, Flutter, Android, or Apple package | The alpha.4 release contract defines one complete SKU per surface, not a full/slim prebuilt matrix. |
| Embed the C ABI | `merman-ffi` | Build the source crate; there is no downloadable C binary SDK. |
| Render from Node.js or SSR | `merman-cli` subprocess | The private Node candidate is not a supported release surface. |

See [Package Surfaces](PACKAGE_SURFACES.md) for delivery channels and the exact release evidence
required by each surface.

## Cargo feature migration

Alpha.4 Cargo features name observable results. Features remain additive, so use
`default-features = false` when absence matters and remember that another dependency can re-enable
a leaf through Cargo feature unification.

| Alpha.3 feature | Alpha.4 selection |
| --- | --- |
| `render` | `svg` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `raster` | Select `png`, `jpeg`, and/or `pdf` independently. |
| `core-host` | Select `system-clock`, `system-timezone`, `system-random`, and/or `system-timing`. |
| `core-full` | No direct replacement. Select only the output and host capabilities the product needs. |
| Historical `full` or `tiny` profiles | Use an exact artifact profile, or disable defaults and select direct leaves. |

A complete Rust renderer can move from the old implementation bundle to the result-named
convenience feature:

```toml
# 0.8.0-alpha.3
merman = { version = "=0.8.0-alpha.3", default-features = false, features = [
  "render",
  "cytoscape-layout",
  "elk-layout",
  "ratex-math",
] }

# 0.8.0-alpha.4
merman = { version = "=0.8.0-alpha.4", default-features = false, features = [
  "complete-svg",
] }
```

A basic SVG-only embedding becomes:

```toml
merman = { version = "=0.8.0-alpha.4", default-features = false, features = ["svg"] }
```

Use [Choosing Merman capabilities](../FEATURES.md) for package-specific examples and the complete
implication table. `complete-svg` is a facade convenience; lower-level crates and release artifact
profiles select direct leaves.

## CLI migration

Alpha.4 gives native and compatibility workflows separate command trees:

| Existing spelling | Preferred alpha.4 spelling |
| --- | --- |
| `merman-cli -i diagram.mmd -o diagram.svg` | `merman-cli mmdc -i diagram.mmd -o diagram.svg` |
| Native one-file rendering through shared flags | `merman-cli render diagram.mmd --output diagram.svg` |
| Native Markdown conversion | `merman-cli batch README.md --output-dir README.merman` |
| `merman-cli render diagram.mmd -e png` | `merman-cli render diagram.mmd -f png` |

Root `-i/-o` invocations remain hidden compatibility aliases. Native `render -e` and `batch -e`
are deprecated aliases for `-f/--format` during `0.8.x` and are scheduled for removal in 0.9.0.
The `mmdc -e/--outputFormat` spelling remains part of the pinned Mermaid CLI compatibility
contract.

Automation should run `merman-cli capabilities --json` and check the reported CLI contract when
it depends on compiled commands, outputs, or optional runtime adapters. See the
[CLI reference](../../crates/merman-cli/README.md) for installation and command details.

## Browser package migration

Alpha.3 published one `@mermanjs/web` package with capability subpaths and raw `pkg/**` exports.
Alpha.4 publishes a lockstep package group. Each package exports only its root and owns exactly one
WASM artifact.

| Alpha.3 import | Alpha.4 choice |
| --- | --- |
| `@mermanjs/web` or `@mermanjs/web/full` | `@mermanjs/web` for the complete browser SDK. |
| `@mermanjs/web/render` or `@mermanjs/web/render-only` | `@mermanjs/web-render` for the supported complete SVG renderer; this is a capability expansion, not an identity-preserving rename. |
| `@mermanjs/web/ascii` | `@mermanjs/web-ascii`. |
| `@mermanjs/web/core` | No identity-preserving replacement. Choose `web-analysis`, `web-editor`, or the full package by workflow. |
| `@mermanjs/web/pkg/**` | No replacement. Import the selected public package root. |

For example:

```ts
// 0.8.0-alpha.3
import * as merman from "@mermanjs/web/render";

// 0.8.0-alpha.4
import * as merman from "@mermanjs/web-render";
```

Do not combine the full package with a slim package in the same realm unless the application
deliberately wants two independent WASM runtimes. See the
[browser package guide](../../platforms/web/README.md) for initialization, Worker, and packaging
details.

## Native ABI migration

Alpha.4 C, Flutter, and Android hosts use ABI 3. Python and Apple use generated UniFFI bindings
from the matching native artifact. Upgrade each language package and native artifact together; do
not mix an alpha.3 generated wrapper with an alpha.4 library. ABI 3 hosts must validate the ABI and
generated runtime capability catalog before requesting optional outputs, resources, or host text
measurement.

Follow the [ABI 3 migration guide](../bindings/ABI3_MIGRATION.md) and the surface-specific Python,
Flutter, Android, or Apple documentation. A channel listed in the repository is not proof that the
alpha.4 artifact has already been published there.

## Rust and embedding API migration

The main changelog groups alpha.4 by user outcome. Source and embedding integrations should also apply the detailed replacements below; no deprecated aliases are retained unless this guide explicitly says otherwise.

### Runtime date and time types

- Replace `chrono::NaiveDate` values passed to `Engine::with_fixed_today`, `RuntimePolicy::with_fixed_today`, `AnalysisOptions::with_fixed_today`, and their local-midnight variants with `merman::time::CivilDate` or `merman_core::time::CivilDate`. Construct one with `CivilDate::new(year, month, day)` or parse its strict ISO representation.
- `OperationContext::today_local`, `RenderSession::local_date`, `RenderSessionReport::local_date`, and `RenderOperationReport::local_date` now return `CivilDate`. Merman no longer exposes Chrono types or requires Chrono in downstream dependency graphs.
- `CivilDate` uses the proleptic Gregorian calendar with a signed 32-bit year, including Mermaid's `+10000` and `-10000` boundaries. Its canonical text syntax uses four unsigned digits for years `0000` through `9999`, a leading `+` for later years, and `-` plus at least four digits for negative years; signed years reject unnecessary leading zeroes. Conversion to an `i64` millisecond instant remains checked; use `try_with_fixed_today_at_local_midnight` when the configured date must also become the operation clock.
- Lower-level time integrations can use `CivilDateTime`, `UtcOffset`, and `OffsetDateTime` from the same module. Named system-zone resolution remains feature-gated by `system-timezone` and is backed internally by Jiff; these project-owned types keep Mermaid's wider calendar domain independent of Jiff's civil-year limits.
- Replace `LocalTimeZone::datetime_from_naive_local(naive)` with a `CivilDateTime` construction followed by `LocalTimeZone::resolve_local(local)`. Replace `LocalTimeZone::datetime_to_local_fixed(datetime)` with `LocalTimeZone::at_instant(datetime.timestamp_millis())`, and replace `utc_fixed_offset()` with `UtcOffset::UTC`. The resolved value is now `OffsetDateTime`; use `local_datetime()` or `utc_datetime()` when a civil representation is required.

### Analysis capture and ownership

- Rename Rust `AnalysisResult` to `AnalysisGeneration`, `Analyzer::analyze_result` to `Analyzer::analyze_generation`, and `analyze_document_result{,_shared}` to `analyze_document_generation{,_shared}`. Use `generation.project(&policy)` instead of direct `payload()` or `diagnostics()` access; `Analyzer::analyze()` remains the diagnostics-only payload path, and `Analyzer::analyze_facts()` owns serialized facts.
- Replace `Analyzer::with_engine_and_options(engine, options)` with `Analyzer::with_engine(engine, options)`. Registry customizations already installed on `engine` are preserved, while the site configuration and runtime policy in `options` are the final parser authority.
- Rich capture returns `AnalysisCaptureOutcome::Ready` or `Rejected`. Use `as_ready()`, `into_ready()`, or `rejection()` rather than assuming a generation exists. A rejection exposes its canonical diagnostics through `payload()` / `into_payload()` and its typed admission reason through `resource_limit()`; inspect the latter with `AnalysisResourceLimit::{id, observed, maximum}`.
- Cancellable shared-source entry points now require caller-owned `Arc<str>`, including `Analyzer::analyze_generation_shared_cancellable` and `analyze_document_generation_shared_cancellable`. Promote borrowed input before calling them so cancellation never hides an uninterruptible full-source copy. `Analyzer::analyze_generation(&str)` remains the synchronous, non-cancellable convenience entry point.
- Obtain sealed `AnalysisGeneration` and `AnalyzedDiagram` values from Analyzer or document-analysis entry points instead of assembling fields, and use `source()`, `source_map()`, `diagrams()`, and `text()` accessors. `AnalysisGeneration::snapshot_policy()` was removed without a replacement: a generation deliberately does not retain the full snapshot policy. `AnalysisOptions::snapshot_policy()` remains available for invalidation decisions.
- `DocumentSnapshot` and `FenceSnapshot` are also accessor-based. Replace direct field access with `uri()`, `version()`, `kind()`, `text()`, `shared_text()`, `source_map()`, `fences()`, and fence accessors as applicable. Construct a snapshot with `DocumentSnapshot::try_from_analysis_generation(version, Arc<AnalysisGeneration>)`; its URI and kind come from the generation's `SourceDescriptor`.
- Match `DocumentAnalysisOutcome` from editor document builders. `DocumentWorkspace::upsert` now returns `Result<DocumentSnapshot, AnalysisRejection>`; cancellable entry points still wrap their outcome in `Result<_, AnalysisCancelled>`.
- Replace public `AnalysisOptions` fields and struct literals with builders and accessors. Remove `parse` / `with_parse_options` because analysis owns strict parser semantics; use `with_source`, `with_site_config`, `with_fixed_today`, `try_with_fixed_local_offset_minutes`, `with_runtime_policy`, `with_max_source_bytes`, `with_max_document_diagrams`, and `with_rule_config` as applicable. Inspect invalidation state through `AnalysisOptions::{snapshot_policy, diagnostic_policy, resource_limits}`, and replace `snapshot_affecting_eq` with `left.snapshot_policy() == right.snapshot_policy()`.
- Update custom `DiagramSemanticParser` overlays to accept `&ParseControl` and return `ParseControlResult<Result<Value>>`. Checkpoint cancellable work, return cancellation through the outer result, return Mermaid failures through the inner result, and handle `merman_core::Error::ParseCancelled` in exhaustive matches.
- Analyzer entry points honor `SourceDescriptor` kind directly: Markdown and MDX inputs use canonical fence extraction and `max_document_diagrams` admission instead of producing one whole-document Mermaid generation.
- Existing `DiagnosticFix::new` calls continue to work, but direct struct literals must account for `edits: Arc<[DiagnosticFixEdit]>`. `AnalysisRuleConfig::with_rule_enabled`, `with_rule_disabled`, and `with_rule_severity` return `Result<Self, AnalysisRuleConfigError>` and reject unknown or non-configurable rule ids.

### Shared text, source maps, and Options JSON

- Replace `SourceMap::line_starts()` with behavioral queries: use `line_count()` to iterate, `line_start(index)` for one start offset, and `line_bounds(index)` for the content bounds of one line. `SourceMap::new(Arc<str>)` is a synchronous convenience constructor; there is no public cancellable `SourceMap` constructor.
- Replace `SourceMap::source_arc()` with `SourceMap::shared_source()`. `SharedTextSlice` retains one `Arc<str>` plus validated UTF-8 bounds; `whole`, `from_range`, `as_ref` / deref, and `source_arc()` do not copy source text. Use `to_owned_text()` only when an owned `String` is intentionally required.
- Decode shared configuration through `AnalysisOptionsJson`. The root object and its `lint` object ignore unknown fields for forward compatibility. Alpha.4 makes direct `LintOptionsJson`, `LintRuleSeverityOverrideJson`, and `ResourceOptionsJson` decoding strict; alpha.3 ignored unknown fields in those nested shapes. `resources` is strict even when nested below the permissive root.
- Replace alpha.3 `resources.max_source_bytes` with `resources.limits.max_source_bytes`, and place the new analysis-only fence admission at `resources.limits.max_document_diagrams`. Alpha.3 `resources.profile`, SVG, Flowchart, Class, and label-limit fields do not belong to alpha.4 `ResourceOptionsJson`; configure render resource policy at the rendering boundary instead. `ParseOptionsJson` and the `parse` member were removed without replacements.

### Editor facts and semantic-token planning

- Rename Rust `workspace_symbols` to `search_document_symbols` and one-shot Web/WASM `editorWorkspaceSymbols` to `editorSearchDocumentSymbols`; browser editor sessions expose `searchDocumentSymbols`. Remove uses of `workspace_symbols_for_snapshots`, `DocumentWorkspace::build_snapshot*`, and `DocumentWorkspace::snapshots`; use `upsert`, `DocumentWorkspace::build_analysis_context_with_shared_text`, or explicit per-document search according to the workflow.
- Replace `Engine::parse_diagram_with_editor_facts_sync` with `parse_diagram_snapshot_sync` or `parse_diagram_snapshot_with_type_sync`. `parse_metadata{,_sync}` no longer accepts `ParseOptions` or returns `Option`; `Engine::parse` and the VS Code `merman.analysis.parse.suppress_errors` setting are removed.
- Treat parser/editor fact structs as non-exhaustive. Construct `EditorSemanticSymbol`, `EditorSemanticFacts`, `FenceSemanticItem`, and `FenceReferenceGroup` through their constructors, `Default`, and `with_*` methods instead of struct literals.
- Migrate semantic-token callers using this exact mapping:

  | Alpha.3 | Alpha.4 |
  | --- | --- |
  | `SemanticToken` | `PlannedToken`; `modifier` becomes `modifier_bits` / `has_modifier()`, and `fact_source` is removed |
  | `SemanticTokenKind` | `PlannedTokenKind` |
  | `SemanticTokenModifier` | `PlannedTokenModifier` |
  | `SemanticTokenLegend` / `semantic_token_legend()` | `SemanticTokenDescriptor` / `semantic_token_descriptor()`; `token_types` and `token_modifiers` become `token_kinds` and `modifiers` descriptor slices |
  | `semantic_tokens_for_snapshot(snapshot)` | `plan_semantic_tokens_for_snapshot(snapshot)` |
  | `semantic_tokens_for_snapshot_range(snapshot, start_line, end_line)` | `plan_semantic_tokens_for_snapshot_range(snapshot, range)` |
  | `token_type_index(kind)` | `kind.code()` and generated descriptor data |
  | `token_modifier_index(modifier)` | `modifier.index()` / `modifier.bit()` and generated descriptor data |

  Both planner entry points return `Result<SemanticTokenPlan, TokenPlanError>`. Range planning accepts editor-core's protocol-neutral `Range`, not separate start/end line integers. Packed output and LSP names come from the generated descriptor contract rather than parallel handwritten index tables.
- Delete TextScan compatibility code; there is no alpha.4 replacement for `ShapeObjectValuePrefix`, `shape_object_value_prefix`, `FenceTextIndex::from_text`, `FenceTextIndex::merge_text_scan_node_ids`, `FenceTextIndexSource::TextScan`, `FenceTextIndexSource::is_text_scan`, or `AnalysisSyntaxFacts::text_scan`.
- The serialized `AnalysisFactsPayload` remains schema `1` but is parser-only: `fact_source: "text_scan"` is removed, unavailable bodies use `"unavailable"`, every semantic item includes `rename_policy`, and unsupported version discriminators are rejected before decoding the body.

### LSP embedding

- `MermanLanguageServer::service()` now returns `(MermanLspService, MermanClientSocket)` instead of tower-lsp's `(LspService<MermanLanguageServer>, ClientSocket)`. Drive the ordered service through `tower::Service<Request>`, call `MermanClientSocket::split()` once, and concurrently drive the returned `MermanRequestStream` and `MermanResponseSink`.
- Replace direct `MermanLanguageServer::new(Client)` construction with `MermanLanguageServer::service()`; the new socket is Merman-owned and no public constructor accepts tower-lsp's client handle.
- Replace the `tower-lsp` dependency with `tower-lsp-server`. Update `tower_lsp::jsonrpc` to `tower_lsp_server::jsonrpc`, `tower_lsp::lsp_types` to `tower_lsp_server::ls_types`, and `tower_lsp::ExitedError` to `tower_lsp_server::ExitedError`.
- Enable the `stdio` feature explicitly when installing or building the bundled `merman-lsp` executable with `--no-default-features`. `stdio_server()` returns Merman's bounded `StdioServer`; replace tower-lsp's `.concurrency_level(...)` with `.ordinary_concurrency_level(...)`, or call `serve_stdio(...)` when the caller needs the explicit `StdioTermination` result.
- Rename `LSP_HANDLER_CONCURRENCY` to `LSP_ORDINARY_HANDLER_CONCURRENCY`. Remove `StdioService`, `LSP_CONTROL_HANDLER_CONCURRENCY`, and `LSP_TOTAL_HANDLER_CONCURRENCY` uses without replacement. Custom transports drive `MermanLspService` through `tower::Service<Request>` and own their scheduling. The public stdio tuning surface otherwise retains `LSP_REQUEST_BYTE_BUDGET` and `LSP_MAX_MESSAGE_BYTES`; the bundled transport internally owns a 96-token retained-deferred budget and four ordinary consumers.
- Update exhaustive `StdioTermination` matches for `InputOverloaded`. Only legal, ID-less, valid-parameter cancel/exit messages whose encoded body is at most 4 KiB use the private immediate-control path. Ordinary request overload returns JSON-RPC `-32099` and continues only when that error can enter the bounded output lane. Notification overload, an unretainable overload error, or exhausted overload-output budget terminates with `InputOverloaded`; once input integrity is lost, later cancel/exit frames are not promised. `OutputClosed` wins when stdout failure races another termination.
- Send `RULE_CATALOG_METHOD` and `CONFIG_SCHEMA_METHOD` through the ordered service instead of calling removed `MermanLanguageServer::rule_catalog()` or `config_schema()` helpers. Rust-only static consumers can call `RuleCatalogResponse::current()` and `ConfigSchemaResponse::current()`.

### Retained wire names

- The Rust type `AnalysisResult` is removed, but the TypeScript/WASM wire-level `AnalysisResult` name remains unchanged.
- `AnalysisPayload` and `AnalysisFactsPayload` continue to use independent schema version `1` contracts.
- Mermaid diagram and compatibility identifiers such as `flowchart-v2` remain wire names; Rust type or planner renames do not rewrite them.

### Rendering and option contracts

- Replace public low-level `merman-render` `layout_parsed*`, `render_layouted_svg`, raw semantic/layout SVG helpers, debug wrappers, and per-family pass-through functions with `merman::svg::HeadlessRenderer`, `prepare_render_sync`, `layout_json_sync`, or `render_svg_sync`. Direct low-level integrations can use `merman_render::family::prepare` with one `RenderSession`.
- Import ELK configuration and guarded pipeline entry points from the `merman-elk-layered` crate root; phase modules are private and require operation-seed resolution.
- Configure text measurement, math, icons, clock, randomness, and resource policy through `RenderEnvironment`. Binding and Web JSON use `presentation.theme` for semantic host colors, top-level `site_config` for raw Mermaid `themeVariables`, and `environment.text_measurement` / `environment.math_renderer` for rendering services. The removed `host_theme` group and the old `layout.text_measurer` / `layout.math_renderer` fields are rejected.
- Replace `HostThemeProfile`, `CompiledHostTheme`, `HostThemeProfileBuilder`, `with_host_theme`, `with_compiled_host_theme`, `render_svg_with_host_theme_sync`, and `render_svg_with_compiled_host_theme_sync` with one immutable `Presentation`. Build semantic colors through `HostTheme`, select `PresentationProfile::MermanModern` independently when wanted, apply Mermaid overrides through `with_site_config`, and select cleanup/background/scoped CSS through an explicit `SvgPipeline` or `SvgOutputPolicy::pipeline()`. Replace flat `supported_host_theme_presets*` calls with `theme_preset_descriptors()` in Rust or the artifact-aware `presentation-catalog` metadata payload in bindings; the old helpers are deleted rather than deprecated.
- Replace field-based `RenderResourceLimits` with sealed `RenderResourcePolicy`. Select `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input`, then apply validated overrides by stable limit id.
- Implement custom wrapped text measurement through `measure_wrapped` using the complete `TextStyle`, including `font_style`. The `measure_wrapped_raw` and heuristic-only `wrap_text_lines_px` APIs are removed; use `wrap_text_lines_measurer` when callers need explicit wrapping.
- Rename `LayoutOptions.viewport_width` / `viewport_height` and matching binding/Web fields to `container_width` / `container_height`; Typst uses `container-width` / `container-height`. CLI users continue to use `--width` / `--height`.
- `LayoutOptions` is now non-exhaustive. Start with `LayoutOptions::default()` or `LayoutOptions::headless_svg_defaults()`, then use `with_container_size(...)`, `with_screen_available_width(...)`, or direct field mutation. Browser hosts should pass `screen.availWidth` for C4 parity; headless hosts can omit it, which falls back to `container_width`.
- Remove `LayoutOptions::use_manatee_layout`; select the `layout-cytoscape` capability instead. Remove `FlowchartElkBackend`; Flowchart ELK always uses the Mermaid adapter and Eclipse ELK layered implementation.
- Use documented kebab-case binding values such as `resvg-safe`, `strip-existing-important`, `trusted-native`, and `unbounded-for-trusted-input`; underscore and shorthand aliases are removed.

### Web editor and measurement APIs

- Replace `createBrowserTextMeasurer()` with `createBrowserTextMeasurementSession()`, retain the returned `measure` callback for the session lifetime, and call `dispose()` when the browser realm or session ends.
- Rename `editorSemanticTokenLegend()` to `editorSemanticTokenDescriptor()` and decode the packed `Uint32Array` returned by `editorSemanticTokens()` against that generated descriptor.
- Remove `selectedRegistryProfile()`, `bindingCapabilities()`, and any assumption that package identity or exported function names determine callable operations. Query `runtimeCatalog()` from the initialized artifact.

### Smaller Rust renames

| Alpha.3 | Alpha.4 |
| --- | --- |
| `merman_analysis::FenceDelimiter::len()` | `marker_len()` |
| `merman_core::diagrams::flowchart::FlowchartV2Model` | `FlowchartModel` |

The Mermaid `flowchart-v2` diagram id and compatibility layout JSON `FlowchartV2` variant key are unchanged.

## What the refactor changes for users

The alpha.4 candidate expands primary SVG admission from 27 to all 35 Mermaid 11.16 families. It
also makes analysis, editor intelligence, layouts, math, exports, icons, network access, Markdown
parallelism, and system runtime adapters independently selectable where the owning product exposes
them.

The historical checkpoint against alpha.3 found a clear win for analysis-only CLI builds, while
complete products became broader rather than uniformly smaller or faster:

| Historical checkpoint | Alpha.3 | Measured alpha.4 candidate | Interpretation |
| --- | ---: | ---: | --- |
| Primary SVG admission records | 27 | 35 | Broader Mermaid 11.16 coverage. |
| Lint/analysis CLI binary | 25,477,648 bytes | 8,166,352 bytes | 67.95% smaller for the measured lint workflow. |
| Lint normal dependency identities | 333 | 123 | 63.06% fewer resolved normal dependencies. |
| Default CLI binary | 32,194,272 bytes | 36,925,360 bytes | 14.70% larger, but the default capability contract also changed. |
| Minimal same-capability native SVG | baseline | 1.12x median latency | A historical 32-fixture checkpoint, not a universal performance improvement. |

These measurements compare alpha.3 with candidate commit `d2698d0a3` on one Apple M4 Pro. Later
focused work removed duplicate Requirement label measurement, accelerated ordinary Mindmap labels,
and accepted a smaller Kanban label-preparation improvement. Those adjacent fixes do not replace a
fresh alpha.3-versus-release A/B run.

Use the [detailed evidence report](ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md) for recipes and historical
measurements. Use the [performance plan](../performance/PERF_PLAN.md) for the rolling optimization
status.

## What remains unproven before release

- The final alpha.4 target commit is not fixed until the release tag is created.
- Final same-host alpha.3 A/B measurements still need to refresh the complete and minimal SVG
  lanes, including Class, Sequence, Requirement, and Mindmap attribution.
- Browser-WASM throughput has not been compared with browser Mermaid.js under one equivalent
  browser contract.
- The private Node candidate lacks reproducible all-target admission and is not a supported package.
- Package availability must be verified at each registry or GitHub Release; repository manifests
  describe the intended contract, not live publication state.

## Further reading

- [Alpha.3 to Alpha.4 evidence report](ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md)
- [Changelog](../../CHANGELOG.md)
- [Capability guide](../FEATURES.md)
- [Package surfaces](PACKAGE_SURFACES.md)
- [Performance plan](../performance/PERF_PLAN.md)
- [ABI 3 migration](../bindings/ABI3_MIGRATION.md)
