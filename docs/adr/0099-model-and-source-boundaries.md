# ADR-0099: Model and source consumer boundaries

## Status

Accepted as the development architecture direction on 2026-10-30, with explicit maintainer
authorization for breaking Rust source changes. Implementation was merged in
[#258](https://github.com/Latias94/merman/pull/258) on 2026-10-31. Acceptance of this decision alone
does not establish dependency, package-size or runtime improvements, or qualify later platform
artifacts.

This decision amends the Rust package/API ownership clauses of [ADR-0010](0010-semantic-model-boundary.md), [ADR-0011](0011-semantic-model-versioning.md), [ADR-0073](0073-family-owned-diagram-architecture.md), [ADR-0091](0091-selectable-diagram-families.md) and [ADR-0098](0098-semantic-presentation-capabilities.md). Their single semantic construction, pinned Mermaid behavior, deep lifecycle, source admission, requested projections and operation contracts remain in force.

[ADR 0103](0103-vertical-workspace-crate-boundaries.md) amends the family-crate and
publication boundaries for the v0.11 vertical extraction. It preserves the model and source
consumer boundaries in this decision.

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
line-ending, HTML attribute, and Mermaid entity rewrites. It also scans the shared Langium string
and declaration terminals and owns the bounded YAML arena, value materialization, and YAML-local
source ranges. Core's preprocessing orchestrator combines the frontmatter and directive modules,
which admit configuration and map the YAML evidence to the diagram source. Core also owns
`PreprocessedSource` and the projection of parser diagnostics and editor facts back to the
original source.

Move pure value/model operations with the types. Keep source sanitization, detector effects,
preprocessing orchestration, source errors, and engine initialization in Core. Replace
source-specific inherent methods with owner functions. Preserve managed deep-value ownership,
public model facts, intentional serde differences and actual full payloads. Retire
compatibility-only full wrappers and obsolete import paths.

### Markup safety remains one implementation

Introduce `merman-markup` for the existing HTML/URL/sanitizer implementation, shared CSS structural predicates and Khroma-compatible color operations used by source ingestion and SVG output. This includes `style.rs` predicates used by Class construction and Packet rendering, and `theme_color.rs` operations/local errors used by core themes and SVG. It depends on model/config/control values and never on core or Render. The model package retains only the entity/text helpers its data/terminal consumers use. Output-only navigation/representation helpers and Render's stricter token-aware CSS policy remain Render-owned.

Core and Render use the same markup implementation. This avoids putting expensive source/output sanitization into every model consumer, a core-to-renderer dependency, or duplicated security algorithms. Render and export have no production path back to core. They still use the required HTML/XML/CSS, math and font processing for their own outputs.

### Route painting is independent of diagram families

`merman-render-core` owns layout coordinates, the shared node, edge, cluster, and label types, and
the geometry that turns a solved route into an SVG path, including marker setback, bounds, and
obstacle clearance. Diagram families bind semantic data and project it into the layout solver.
The shared layout adapters connect each solver's work and interruption hooks to the same
operation meter. Render Core projects shared measured graphs into Shoal and maps solved geometry
back into the route point types used by the path builder. The renderer selects the operation's
placement policy and owns layout dispatch.

Render Core also owns shared font stacks, label tokenization and line breaking, SVG XML structure
admission, and Iconify pack admission and lookup. `merman-render` selects the text measurement
provider, performs terminal resource and font checks, and applies the operation's output policy
to icon fragments before they enter a diagram.

### Flow diagrams

`merman-core-flow` owns Flowchart and Swimlane tokenization, predictive parsing, and model
construction, along with the shared presentation lexer used by Agentflow. `merman-core` owns
source mapping, diagnostics, and editor recovery. It observes the syntax before the provider
consumes it to build the model.

`merman-render-flow` owns the shared layout settings and shape geometry for Flowchart, Swimlane,
and Agentflow. It resolves family defaults, typography and density overlays, and the Agentflow
configuration projection before layout. It also resolves shape aliases and image and icon
dimensions, and supplies organic outlines and BPMN connector intersections. It parses authored
styles and prepares the operation-local class styles shared by layout and paint. Its Dagre adapter
owns compound graph assembly, cluster anchor selection and extraction, and the associated work
accounting. `merman-render` owns diagram traversal, text measurement, asset policy, layout output
projection, and final CSS and SVG emission.

### Structured diagrams

`merman-core-structured` owns Info and Ishikawa syntax, Packet field scanning and bounded row
expansion, Railroad dialect parsing and recovery, Sankey CSV parsing and graph construction, and
Treemap row parsing and hierarchy construction. It also owns Timeline lexing, parsing and model
construction, using the shared line cursor in `merman-core-preprocess`. `merman-core` prepares
the source, builds Ishikawa's semantic tree, supplies config-dependent sanitization, and maps
models and syntax diagnostics into public parse results with optional editor facts.

`merman-render-structured` owns the typed layout and complete SVG emission for Cynefin, Info,
Ishikawa, Packet, and Railroad. It uses the shared root SVG, CSS,
`TextMeasurer`, and controlled Rough SVG contracts in `merman-render-core`. `merman-render`
dispatches through the family features, selects the operation's measurement and viewport
policies, and owns final canvas and SVG admission.

### Graph syntax

`merman-core-graph` owns Class, ER, and State tokenization, parsing, and model construction.
It reuses the controlled token cursor and pinned Mermaid Unicode rules in
`merman-core-preprocess`. `merman-core` owns source mapping, diagnostic projection, resource
admission, and editor and compatibility projections.

`merman-render-graph` owns ER label preparation, styling, and complete SVG emission.
`merman-render` selects the layout backend, supplies the route geometry and operation context,
and applies terminal SVG policies.

### Sequence diagrams

`merman-core-sequence` owns Sequence tokenization, grammar replay, and semantic model construction.
Core captures editor facts from the same lexical stream before handing ownership to the Sequence
parser, then maps diagnostics to the original source and applies the shared admission policies.

`merman-render-sequence` owns Sequence preparation, typography, layout, and complete SVG emission
for both Mermaid parity and the opt-in NativeCompact profile. It uses the shared text, math, and
bounded SVG contracts from `merman-render-core`. `merman-render` supplies the paired semantic
model and operation context, finalizes diagram IDs, and owns terminal output admission. This
follows the vertical renderer boundary in [ADR-0103](0103-vertical-workspace-crate-boundaries.md).

### Chart rendering

`merman-render-charts` owns preparation, typed layout, style binding, and complete SVG emission
for Pie, Quadrant Chart, Radar, Sankey, Treemap, Venn, Wardley, and XY Chart. It uses the shared
text measurement, CSS, XML escaping, and theme paint writers in `merman-render-core`.
`merman-render` supplies the operation's selected theme, measurement, work meter, and viewport
context and applies terminal SVG policy.

`merman-render-structured` owns the Gantt task and time-axis layout, style and font admission,
and complete SVG emission. The facade supplies the time and time-zone services from the render
session and retains the public layout model paths and terminal SVG policy.

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
