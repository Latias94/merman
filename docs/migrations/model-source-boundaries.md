# Model, source, and output migration

This guide describes the development branch after published Merman `0.9.3`. Upgrade coupled Rust packages and generated wrappers together. These breaking Rust changes belong to the next pre-1.0 minor release; the published `0.9.x` maintenance line retains its existing APIs.

## Choose the packages your application consumes

| Application input and output | Packages |
| --- | --- |
| Construct, inspect, or exchange canonical Rust data | `merman-model` |
| Existing models to terminal text | `merman-model` and `merman-ascii` |
| Existing presentation data to layout or SVG | `merman-model` and `merman-render` |
| An admitted SVG to PNG, JPEG, or PDF | `merman-export` with the required export features |
| Mermaid source to canonical meaning | `merman-core` |
| Mermaid source to diagnostics | `merman-analysis` |
| Mermaid source to terminal or graphical output | `merman` |

The model, ASCII, Render, and Export production dependency graphs exclude Mermaid source ingestion. Graphical output still includes its actual HTML, URL, CSS, XML, color, layout, math, or font work when selected. Export retains its dependency on Render's SVG and measurement contracts.

All lower-level packages have empty default features. Select the required `diagram-*` data, source parsers, and output handlers at their respective owners. `all-diagrams` selects every family available at that owner. Selecting a model does not enable a source language or an output handler. Model feature union cannot widen a host's declared capabilities.

Select `merman-model/presentation-data` when constructing or receiving presentation data directly. A full source engine instead requires `presentation` on `merman-core` or the facade. `merman-model/compat-json` also enables `presentation-data`; retaining presentation facts alone does not require the JSON projector.

The `merman` facade keeps its existing default SVG and layout-engine selections. Use `default-features = false` for a deliberately selected source-to-output build. Source-to-ASCII support is the intersection of the source parsers and terminal handlers; direct model consumers use `merman_ascii::ascii_capabilities()`.

## Update Rust imports

Use the defining package for shared data and contracts. The facade exposes the same package as `merman::model` and keeps explicit workflow exports, including `Renderer`, `SemanticParser`, `MermaidEngine`, `ParseOptions`, `MermaidConfig`, and operation control.

| Previous import | Current import |
| --- | --- |
| `merman_core::diagrams::<family>` model types | `merman_model::families::<family>` |
| `merman_core::models::class_diagram` | `merman_model::families::class` |
| `merman_core::SemanticDiagramModel` | `merman_model::SemanticDiagramModel` |
| `merman_core::ParsedSemanticDiagram` | `merman_model::ParsedSemanticDiagram` |
| `merman_core::MermaidConfig` | `merman_model::config::MermaidConfig` |
| `merman_core::operation` | `merman_model::operation` |
| `merman_core::runtime` | `merman_model::runtime` |
| `merman_core::resources` shared policy and complexity | `merman_model::resources` |
| `merman_core::time` | `merman_model::time` |
| `merman_core::ManagedSemanticJson` | `merman_model::ManagedJson` |
| `merman_core::RenderSemanticModel` | `merman_model::presentation::RenderSemanticModel` |
| `merman_core::PresentationMetadata` | `merman_model::presentation::PresentationMetadata` |
| `PacketRenderBlock` | `merman_model::families::packet::PacketBlock` |

Canonical module spellings include `error`, `event_modeling`, `git_graph`, `quadrant_chart`, and `xy_chart`. Flowchart and Swimlane share `families::flowchart`; Agentflow retains its distinct `families::agentflow` model. Journey, Timeline, Info, Pie, Packet, and Radar use their canonical payload directly in the full-model enum; code that moved fields through a redundant wrapper's `semantic` field should use the payload itself.

Shared HTML/URL safety, structural CSS predicates, and Khroma-compatible color operations live in `merman-markup`. Select `html`, `url`, and `color` only when needed. Output-specific navigation helpers live in `merman_render::svg_security`.

## Separate source configuration from frozen output values

`SourcePresentationConfig` is the mutable full-source configuration accepted by `MermaidEngine::with_site_config`. Semantic parsing continues to accept `MermaidConfig` directly:

```rust
use merman::{MermaidConfig, MermaidEngine, SourcePresentationConfig};

let config = MermaidConfig::empty_object();
let engine = MermaidEngine::new()
    .with_site_config(SourcePresentationConfig::from_config(config));
```

Custom full parser callbacks now receive `&SourcePresentationMetadata`. Completed parse results expose shared `PresentationMetadata` with immutable `PresentationConfig`. Inspect its values and ownership through read-only queries. A completed configuration is not a mutable source builder; change the source engine configuration and parse again when the source environment changes.

Compiled themes expose `DiagramTheme::compatibility_recipe()`. A low-level source integration can create `merman_core::presentation::ThemeCompatibilityPlan::from_recipe(recipe.clone())` and install it with `MermaidEngine::with_theme_plan(&plan)`. Retain the plan for repeated initialization. The facade handles this when a request selects a theme.

## Pass presentation data to graphical consumers

`merman_render::family::prepare` accepts `&merman_model::presentation::PresentationDiagram`. Core's successful source result remains closed to external construction and provides the complete shared input:

```rust
let parsed = engine
    .parse_diagram_for_render_model_sync(source, parse_options)?
    .ok_or("no diagram")?;
let artifact = merman_render::family::prepare(
    parsed.presentation(),
    &layout_options,
    session,
)?;
```

Keep the presentation owner alive while borrowing `FamilyRenderArtifact<'_>`. `ParsedDiagramRender::into_presentation()` transfers all shared data and output context. `into_parts()` retains its narrower metadata/model projection and intentionally omits output context.

An application that already owns typed data can use `PresentationDiagram::try_new` with `AuthoredPresentationMetadata`, a full typed model, `PresentationContext`, construction policy, and operation control. The checked frozen route is `try_from_frozen`. These constructors validate declared relationships and observe construction budgets and cancellation. They do not create a core parsed result, authenticate source history, or replace the output owner's admission checks.

Use `PresentationContext` for irrecoverable family facts such as explicit empty output, Flowchart label/style order, and Class style precedence. Manual values default to their ordinary model projection. A nonempty Mindmap requires an explicit output identity; source producers capture it once and repeated outputs reuse it. Mermaid's internal Mindmap node identities form a single-parent tree; repeated authored names and label text remain valid.

Direct producers must also preserve the full model's structural representation. Output admission
returns typed errors for invalid data, including these family requirements:

- Class, State, and ER containment must stay acyclic after parent normalization and mapping to
  layout identities. Display labels do not identify parents.
- Each reachable Block child record has one parent. Column-span totals must fit the model's
  integer range; separate records may retain equal authored IDs or labels.
- Packet fields must already be split at the configured `bitsPerRow` boundaries. Each block has
  nonnegative, ordered, representable inclusive endpoints and fits within its row.
- Kanban presentation data lists each section immediately before its visible cards, whose
  `parent_id` matches that section. Repeated section IDs retain their own expanded card occurrences;
  do not regroup an already-expanded sequence by ID. Non-group entries before the first section
  and detached entries without a parent retain their non-rendering behavior.
- Treemap weights must produce finite aggregate values and coordinates. Extremely large finite
  inputs can still overflow intermediate geometry and return `InvalidModel`.

Request compatibility JSON on the complete presentation value when its frozen output facts matter. A raw model's compatibility projector uses the ordinary default context. Enable `merman-model/compat-json` for independent projection; a source producer can retain presentation facts without enabling `merman-core/compat-json`.

## Request fixes explicitly

`merman-analysis/fixes` is a positive, default-off feature. Diagnostic-only builds omit the YAML replacement writer. Compiling fixes does not generate replacements until the diagnostic projection requests them:

```rust
use merman_analysis::{AnalysisOptions, FixGeneration};

let options = AnalysisOptions::default()
    .with_fix_generation(FixGeneration::Generate);
```

Enable `fixes` on the owning package or facade for that example. `FixGeneration::Omit` is the default. Omit and Generate projections reuse the same immutable analysis generation, source maps, and fence identities. Rule filtering and suppression happen before replacement materialization.

The full distributed CLI and editor/native SDK recipes retain fixes. CLI lint/check and LSP diagnostic publication omit replacement generation; the fix command and LSP code actions request it. Diagnostics-only CLI/Web/Typst profiles omit the capability. A host's broad dependency feature union does not grant fixes to an artifact that did not declare them; bindings retain the typed missing-capability response for unavailable operations. Existing capability identities remain stable, with `fixes` appended as bit 21.

The fixed model-to-ASCII consumer's external dependency closure fell from 84 to 28 versions,
with matching consumed outputs. Full source workflows retain their parser dependencies; removing
source ingestion from a graph does not promise the same size or compilation benefit for every
application. See the [measured results](../reviews/2026-10-30-model-source-boundaries-implementation.md#final-independent-consumer-measurements)
for exact features, executable sizes and the separately scoped cold-compilation observations.

See the [implementation record](../reviews/2026-10-30-model-source-boundaries-implementation.md) for actual validation scope, and the [architecture plan](../plans/2026-10-30-model-and-source-boundaries.md) for the remaining platform gates. Published `0.9.3` artifacts are unchanged.
