# ADR-0099: Model and source consumer boundaries

## Status

Accepted as the development architecture direction on 2026-10-30, with explicit maintainer
authorization for breaking Rust source changes. Implementation was merged in
[#258](https://github.com/Latias94/merman/pull/258) on 2026-10-31. Acceptance of this decision alone
does not establish dependency, package-size or runtime improvements, or qualify later platform
artifacts.

This decision amends the Rust package/API ownership clauses of [ADR-0010](0010-semantic-model-boundary.md), [ADR-0011](0011-semantic-model-versioning.md), [ADR-0073](0073-family-owned-diagram-architecture.md), [ADR-0091](0091-selectable-diagram-families.md) and [ADR-0098](0098-semantic-presentation-capabilities.md). Their single semantic construction, pinned Mermaid behavior, deep lifecycle, source admission, requested projections and operation contracts remain in force.

## Context

Independent consumer graphs inspected on 2026-10-30 exposed a remaining ownership problem after
the semantic/presentation and controlled-kernel refactors: models, config values, execution types
and family grammar still shared the core package. A model-to-ASCII application therefore compiled
source ingestion even though it never parsed Mermaid. Diagnostic-only analysis also compiled the
YAML serializer used to generate fixes.

Render and export have related ownership friction. Render consumes core models and frozen facts, but also core sanitization and an engine-installation bridge. Export needs Render's sealed SVG, measurement interface and font policy, plus core operation/resource types. Removing one direct dependency without resolving these paths would leave the same source stack transitively reachable.

The project continues to target precise Mermaid behavior. HTML recovery, URL/Unicode processing, source CSS policy, native font identity and terminal SVG validation have real callers. Removing them or replacing them with string heuristics would change that product. Pele provides useful examples of delivery and host-measurement boundaries, but its custom visual contract is not a substitute for Merman's pinned-source semantics.

## Decision

### Shared data has one independent owner

Introduce `merman-model` for canonical family data and its necessary value, identity, lifecycle, runtime and resource contracts. Default features are empty. Keep the existing positive family vocabulary, plus explicit full presentation data and data-projection features. Avoid separate packages for every family, config object or execution primitive.

Use `merman_model::families::<family>` as the canonical family namespace. Core retains source parser/DB construction privately and depends on these types. ASCII and Render select the shared data and their own handlers without selecting core parsers. Export uses shared execution types and retains its real dependency on Render. The facade owns source-to-output orchestration and may expose an intentional `merman::model` namespace.

`merman-core-preprocess` owns controlled UTF-8 edits, byte-span mapping, and the source-only
line-ending, HTML attribute, and Mermaid entity rewrites. Core retains frontmatter and directive
interpretation, `PreprocessedSource`, and the projection of parser diagnostics and editor facts
back to the original source.

Move pure value/model operations with the types. Keep source sanitization, detector effects,
preprocessing orchestration, source errors, and engine initialization in Core. Replace
source-specific inherent methods with owner functions. Preserve managed deep-value ownership,
public model facts, intentional serde differences and actual full payloads. Retire
compatibility-only full wrappers and obsolete import paths.

### Markup safety remains one implementation

Introduce `merman-markup` for the existing HTML/URL/sanitizer implementation, shared CSS structural predicates and Khroma-compatible color operations used by source ingestion and SVG output. This includes `style.rs` predicates used by Class construction and Packet rendering, and `theme_color.rs` operations/local errors used by core themes and SVG. It depends on model/config/control values and never on core or Render. The model package retains only the entity/text helpers its data/terminal consumers use. Output-only navigation/representation helpers and Render's stricter token-aware CSS policy remain Render-owned.

Core and Render use the same markup implementation. This avoids putting expensive source/output sanitization into every model consumer, a core-to-renderer dependency, or duplicated security algorithms. Render and export have no production path back to core. They still use the required HTML/XML/CSS, math and font processing for their own outputs.

### Route painting is independent of diagram families

`merman-render-core` owns layout coordinates and the geometry that turns a solved route into an
SVG path, including marker setback, bounds, and obstacle clearance. Diagram families retain
semantic bindings, work accounting, and projection into the layout solver. Shoal and the path
builder exchange the same route point types.

### Flow diagrams

`merman-render-flow` owns Flowchart, Swimlane, and Agentflow shape geometry. It resolves shape
aliases and image and icon dimensions, and supplies organic outlines and BPMN connector
intersections. `merman-render` owns traversal, work accounting, asset policy, and SVG emission.

### Structured diagrams

`merman-core-structured` owns Packet field scanning and bounded row expansion, along with the
Railroad lexer, four dialect parsers, and rule recovery. `merman-core` prepares the source and
maps each parser's model, diagnostics, and optional syntax spans into the public parse result.

`merman-render-structured` owns the typed layout and SVG emission for Packet and Railroad. It
consumes canonical model data and uses the shared CSS and text contracts in `merman-render-core`.
`merman-render` dispatches to it through the existing `diagram-packet` and `diagram-railroad`
features and supplies the operation's text measurement to Railroad.

### Graph syntax

`merman-core-graph` owns Class, ER, and State tokenization and parsing. Its outputs are Class
grammar actions, the canonical ER model, and the nested State syntax document. It reuses the
controlled token cursor and pinned Mermaid Unicode rules in `merman-core-preprocess`.
`merman-core` owns Class and State model construction, source mapping, error messages, resource
admission, and the editor and compatibility projections.

`merman-render-graph` owns ER relationship marker geometry, names, and SVG definitions.
`merman-render` resolves source styles and provides the operation's bounded SVG sink.

### Sequence diagrams

`merman-core-sequence` owns tokenization and grammar actions for Sequence. Core projects editor
facts from the same lexical stream, builds the semantic model, and maps diagnostics to source.

`merman-render-sequence` owns participant geometry, glyphs, and lifelines. `merman-render` supplies
the bounded SVG sink and diagram ID, places the labels, and retains traversal and operation checks.

### Chart rendering

`merman-render-charts` owns typed layout, style binding, and SVG content for Pie. It uses the
shared CSS, XML escaping, and theme paint writers in `merman-render-core`. `merman-render`
supplies text metrics and assembles the chart into a scoped SVG document under the render
operation's output policy. For XY Chart, the chart crate owns typed layout, plot geometry, and
bar label placement. For Sankey, it owns the typed layout and configuration lookup. `merman-render`
supplies text metrics and emits the SVG for both families.

### Frozen presentation is distinct from engine initialization

Shared presentation data includes the pure full payloads, exact render facts, normalized theme recipe and frozen resolved configuration/ownership used by terminal or graphical output. Core retains mutable detector journals, the actual history-based freeze computation, initialization caches, theme evaluation and source construction. Equal final configuration values do not prove equal authored authority.

Render consumes a shared immutable `PresentationDiagram`; core's private-construction parsed-result wrapper contains it and retains the actual source-origin guarantee. The shared value is data, distinct from the facade's operation-owning `PresentationArtifact`. Its authored constructor checks family/model/context pairing, IDs/ordinals and the complete normalized theme recipe, treats supplied config as caller-owned and infers no source-derived residual from equal values.

Core freezes its real history and uses a bounded frozen-data constructor to validate the declared output facts' path, recipe and model/context relationships. Independent producers may also use that API, explicitly as declared data; it authenticates no parse history and grants no source-admitted or trusted state. No public fields, unchecked constructor or fingerprint-only proof substitutes for data checks. Caller declarations cannot manufacture core's closed parsed result. Render checks complete recipe equality and real model/output admission for both routes; source CSS admission stays with its source owner.

The facade installs the renderer's normalized theme recipe on core; Render does not take or return a `MermaidEngine`. Pure compatibility JSON projectors use shared immutable evidence and a projection-specific error. Irrecoverable empty/presence/order facts live under `presentation-data` and full core construction freezes them independently of `compat-json`; that feature enables serialization/routing rather than capture. Manual models default to their declared full-model projection rather than an inferred parsed-empty history. Parsing/routing and source-error adaptation stay core. No source replay or JSON conversion is used to reconstruct a built-in model or lost context.

`ResvgCompatibleSvg`, its terminal validator and seals remain Render-owned. Export retains its usvg/paint representation checks, native font acquisition/shaping/cache and measurement-to-painting identity. Do not make the SVG seal forgeable or treat a filter/theme receipt as broader safety authority. The native backend version migration remains separate from this ownership refactor.

### Availability follows the consumer

One declaration owns pinned family identity/alias facts; executable callbacks stay at their implementation owner. Model types, core parser availability and ASCII/graphical handler availability are separate observations. A source-to-output facade reports the intersection it can execute. Selecting shared Flowchart data for another family does not enable the Flowchart or Swimlane language.

Artifact parser-family expectations retain their source meaning; they are not inferred from visible Rust model variants. Validate exact built-product capabilities and host declarations under feature union using the existing descriptors, artifact checks and isolated consumers. Do not add another resolver or require a universal callback registry.

### Fix generation is explicit and requested

Add a positive, default-off `fixes` capability to analysis and forward it through relevant owners. Cargo chooses whether the serializer/generators exist; the Rust diagnostic projection chooses whether this request generates edits. Default diagnostic projection omits generation. Rule discovery, suppression and severity projection do not eagerly serialize replacements.

Reuse immutable generations, buffers, maps and provenance. Retain bounded repair evidence without retaining parse-only site configuration. Preserve complete-action coordinate remapping and shared edit identity. Full CLI/editor/LSP/SDK recipes explicitly retain fixes; diagnostics products omit them and document the migration. Host declarations determine exposed behavior even when another dependency compiles fixes.

Fix DTOs and code-action conversion remain usable without generators. The new capability is described through the existing feature/catalog contract without renumbering established IDs. Wire payloads, options and ABI versions do not change merely because Rust package ownership changes; an actual schema or operation change requires its own migration.

### Breaking source cleanup is intentional

The maintainer-authorized development migration may remove Rust module paths, convenience APIs, compatibility-only wrappers and feature/default behavior within the same pinned upstream baseline. ADR-0011's semantic stability continues to protect accepted meaning and existing serialization/transport contracts; it is not used to retain every internal-looking but public Rust path during this explicit migration.

Publish the resulting source breaks on an appropriate next pre-1.0 minor line with a concrete migration guide. Do not ship them as a 0.9.x maintenance patch or rewrite published artifacts. No new release version, publication or tag is selected by this ADR.

## Alternatives considered

| Alternative | Decision |
| --- | --- |
| Continue adding features inside core | Insufficient for callers needing model/output types independently of that package; it also leaves capability ownership tied to parser selection. |
| Put models, parser, sanitizer and output contracts in one low-level package | Recreates the measured minimum graph and mixed responsibilities. |
| Give every family/config/runtime/text concept its own crate | No demonstrated independent consumer need for that many package interfaces; use internal modules initially. |
| Move markup into Render or copy it into both consumers | The first reverses the parser/output dependency direction; the second duplicates correctness and security ownership. |
| Preserve old Rust imports and wrappers through aliases/adapters | Rejected for this authorized cleanup. Migrate consumers and remove obsolete paths at closeout. |
| Drop corner cases, normalize URLs with strings, or disable source admission | Changes the precise product; it does not repair dependency ownership. |

## Consequences and evidence

Model/terminal and diagnostics consumers gain independently enforceable smaller production graphs. Graphical/export consumers can omit Mermaid source parsers while retaining shared markup and their native output costs. Full source-to-SVG still needs those source semantics; this decision does not promise a large reduction there.

Two source packages join the coupled publish order and generated legal ownership. All in-repository Rust consumers, artifact profiles, host catalogs and examples migrate. Complete model/terminal/serde/compatibility and constructed presentation boundaries need review because former crate-private storage becomes a real package interface.

Acceptance comes from independent compiled consumers, exact normal/build exclusions and the current semantic, lifecycle, terminal, feature, sanitizer, CSS/color, SVG/seal, measurement and installed-product owners. Include core presentation without its JSON serializer feeding Render projection, source-history authority controls and conservative authored construction. Artifact size and cold compilation are separate controlled measurements with honest inconclusive outcomes. Historical performance evidence is not rebound to the new implementation by editing its hashes.

The [migration guide](../migrations/model-source-boundaries.md) owns current consumer migration.
Current platform delivery and qualified release scopes belong to
[package surfaces](../release/PACKAGE_SURFACES.md) and the
[publication record](../release/PUBLISH_ORDER.md).

### Platform qualification boundary

Release Preflight's `flutter-dry-run` owns the complete Flutter iOS/macOS native slices,
artifact-matrix checks, and macOS Dart ABI 3, example, and theme consumers. Its existing workflow
and package responsibilities are documented by the [release guide](../release/RELEASING.md) and
[Flutter binding guide](../bindings/FLUTTER_DART_FFI.md). Apple Swift/UniFFI smoke success and
ordinary Ubuntu binding CI do not qualify this separate scope. Dispatching a workflow without
completed execution is not qualification evidence.

The required evidence covers slice builds, symbols, packaging, and Dart execution on the macOS
runner's host architecture. It does not establish iOS device execution, runtime execution on both
macOS architectures, or installation from pub.dev. Use the existing platform owner against the
final candidate; historical correctness or measurement results do not transfer that qualification
to later source or authorize publication.
