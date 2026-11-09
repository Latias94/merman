# Diagram Theme Coverage

Use the loaded runtime's discovery and the result of the actual render to assess theme support.
The [authoring guide](custom-diagram-themes.md) explains recipes and source precedence;
[preset qualification](preset-qualification.md) defines the bounded visual checks. This page
explains those evidence boundaries. Machine contracts and runtime source own the capability inventory.

## Coverage across diagram families

The complete runtime has 34 selectable logical diagram families. Railroad's four syntaxes bring
the selectable diagram-type count to 37. A preset or imported recipe can be selected for every
family, but this does not mean every family consumes the same styles. New diagram registration
does not automatically create a Merman theme adapter.

| Families | Current compiled-theme scope |
| --- | --- |
| Flowchart, Agentflow, Swimlane | Node, label, connector and container paint; inherited fonts and family-specific label typography. Agentflow keeps its own kind palette and source precedence. Effects retain the shared shape/look restrictions below. |
| Usecase | Usecase/JSON bodies, actors, notes, system boundaries, connectors, labels, markers and titles; inherited and role fonts. Bounded body shadows have their own Classic-look admission. |
| Venn, Ishikawa, Railroad (including EBNF/ABNF/PEG), Event Modeling | Paint for actual diagram bodies and their labels. Ishikawa/Railroad/Event Modeling also consume connector and marker paint; Event Modeling consumes lane paint. Static solid/transparent paint is distinct from unsupported gradients or ordinal rules. |
| Cynefin, Wardley | Domain/component bodies, labels, titles, connectors and markers. Both have Node ordinal palettes; explicit body fill takes precedence. Wardley additionally consumes axis paint. Diagram-owned notes and specialized decorations keep their own configuration channels. |
| Tree View | Text/node-label, edge/marker and existing highlight-body paint, inherited fonts and measured base font size. A Node body rule styles existing `:::highlight` rectangles; it does not add rectangles to ordinary file labels. |
| Packet | Block-body fill/stroke, byte-label, field-label and title paint, plus font-stack support. Mermaid's Packet variables remain independent property owners; semantic body effects are not implemented. |
| Class, ER, Architecture, Block, C4, Gantt, GitGraph, Journey, Kanban, Mindmap, Pie, Quadrant Chart, Radar, Requirement, Sequence, State, Timeline, XY Chart, ZenUML | Family-owned semantic paint and typography consumers. Targets differ: a Radar axis, a Gantt task and a Sequence actor do not share one universal Node rule. Existing labels/titles and source-owned roles retain their own terminal domains. |
| Sankey, Treemap | Node/section paint, text and inherited fonts, with family-specific palette and source-style ownership. Sankey also consumes exact/cyclic Node fill and stroke rules; compiled themes composite flow bands normally below nodes and labels. A diagram without a visual title does not gain a title merely because a recipe requests it. |
| Info | Version-label paint and inherited font stack. There is no separate visual title or node-body surface. |

Packet layout settings such as `showBits` and `rowHeight` belong under `packet`; colors and role
font sizes belong under `themeVariables.packet`. Root `packet.labelColor` and other root style
keys are ignored. Source theme variables remain subject to the site's secure-key policy.

Within Flowchart, BPMN event rings accept Node fill, stroke and dash patterns while keeping
their notation-specific ring widths. Annotation brackets accept stroke, stroke width and dash
patterns. A fill request on an annotation or a ring-width request remains a residual even when
another property of the same rule applies.

Flowchart's local `edge-label` fill consumes static solid or transparent paint in SVG and HTML
labels, including Markdown. Agentflow and Swimlane reuse those terminal consumers. Shared Text
and local label fills retain authored order; explicit source/configuration colors keep their
property ownership. Unsupported paint and unknown HTML inheritance remain residuals, while
labels absent from the scene do not count as consumers. Inspect actual output alongside the
same-render evidence; supporting one fill domain does not establish all paint or selector domains.

Class's local `edge-label` fill applies static solid or transparent paint to central relationship
labels and cardinalities in SVG and HTML output. It leaves node and note labels unchanged. Shared
Text and local fills follow authored order; explicit configuration and visible HTML colors retain
their ownership. Unsupported selectors, paints and label effects remain residuals when applicable.

Block label colors target the ordinary paragraph in its prepared HTML label. Source colors still
own visible HTML blocks that inherit them; custom HTML outside that paragraph remains an unsupported
paint request when the writer cannot prove that the visible text consumed the theme color.

All families can receive root canvas styling. A diagram-owned background is a separate surface and
can cover the canvas according to source/configuration precedence. A canvas-only verified result
does not establish node, label, line or effect support.

Typed shadows/glows currently have consumers in Flowchart, Agentflow, Usecase, Sequence, State, Swimlane and XY Chart,
with target and geometry restrictions. Flowchart supports its documented basic shapes in Classic
and Neo. Agentflow shares those bounds; Usecase admits bounded Classic body shadows. State's supported body-shadow Value path is Classic; Swimlane's effect path is also
Classic. A coarse conditional support query does not promise the effect under the current look.
Other families can have Mermaid look decorations without consuming compiled theme effects.

Admitted body effects in Flowchart and its shared consumers, State, Usecase and rectangular
Sequence actors keep depth behind connections and the surface/text in front. Depth and surface
have separate emission passes, with one visible surface even for alpha paint. This does not
extend effect admission to additional shapes or looks. State composite titles use the same
normalized measured box for layout and header placement, in native text and HTML labels.

The current source catalog groups 8 built-in design identities with 11 concrete appearances.
They share paint and typography defaults, with selected family-specific rules. Their catalog
design scope is narrower than runtime theme availability. The current Playground library contains
20 designs: Reading and Prism add coordinated Light/Dark examples to Riso, Pop Art, Metro, Carbon Copy,
Signal, Field Notes, Lacquer, Paper Cut, Aurora, Phosphor, Letterpress, Grove, Atlas, Nightfall,
Blueprint, Porcelain, Chalk and Monograph. All library designs use examples and scoped rules for
Flowchart, Sequence, Class, XY Chart, Pie, State, ER and Mindmap. Additional dedicated treatments are
Metro/Git Graph, Signal/Gantt, Carbon Copy/Kanban, Field Notes/Timeline, Blueprint/Architecture,
Prism/Radar/Quadrant Chart/Sankey/Treemap, and Monograph/Requirement/Tree View. The sample inventory
is separate from each design's reviewed family list. The library's family filter and
coverage labels describe this dedicated design scope. Other families can receive shared text, font, palette
or canvas values; those changes do not establish a dedicated design for that family.

The library recipes use scalar Mermaid compatibility values for the Mindmap root (`git0`) and
the Redux Color ER relationship-label backing (`erEdgeLabelBackground`). These are independent
fallback values, not retained palette bindings; explicit source/configuration can replace them.
The current execution accounting reports `compatibility-unverified` for these requests, including
renders of other families. Keep that residual and the resulting rejected target admission visible:
dedicated design scope and a visually reviewed sample do not upgrade the execution result to
`verified` or establish portability. Non-Redux ER bases need separate backing review.

Use discovery and the same render's execution evidence below to inspect an actual document. In
particular, distinguish an SVG that renders successfully, styling that visibly applies, completed
theme accounting and an admitted output target. These are separate observations.

## Discover support in the artifact being used

Read the runtime catalog before selecting a family, preset or output. A grouped preset's `available`
means the artifact admits the complete design, including every declared appearance. Each child
reports that same gate and its exact `concrete_preset_id`; a passing branch cannot make an invalid
pair available. A catalog family treatment of `dedicated` or `base_only` describes design
intent; missing or unknown treatments remain unreviewed. Preview colors and application notes are
discovery aids. They do not certify readability, mechanism application or output portability.

`describe_theme_support` / `describeThemeSupport()` queries one logical family and rendering context.
The version-1 subject is a `rule` target/facet, an `ordinal-palette` target, or a `base-typography`
property. Base typography is distinct from a Text rule; support for one does not grant the other.
Use logical family IDs from discovery, rather than parser aliases or render implementation names.

The [support contract](../../crates/merman-theme-contract/src/support.rs) owns the exact wire shape
and identifiers. Its result preserves the query, `state`, `reason_ids` and `claim_revision`.
The claim revision versions support assertions independently of the wire schema; it is not an
execution receipt. Unknown discovery identifiers resolve conservatively to `unverified`.

| Discovery result | Interpretation |
| --- | --- |
| `conditional` | A family-owned consumer exists, subject to document, selector, value, source or output conditions. |
| Partial support | Reported as `conditional` with `theme-support.public-value-domain-partial`; only part of the queried facet's public value domain has a consumer. |
| `unsupported` | The known public domain has no supported route. A different property on the same rule may still apply. |
| `not-applicable` | The known target/facet does not belong to this family or output contract. |
| `unverified` | The build cannot make a safe claim, including unknown IDs, missing render features or incomplete output qualification. |
| `unconditional` | The contract's complete-domain state; current family-owned positive projections remain `conditional`. |

Output IDs have different scopes. Catalog artifact format `svg` is not a support-query context;
query `standalone-svg` or `browser-svg` explicitly. The current [query implementation](../../crates/merman-render/src/diagram_theme/support.rs)
projects family mechanisms for standalone SVG. Browser SVG remains `unverified` pending terminal
qualification; PNG/JPEG/PDF static queries remain `unverified` because their target owners qualify
concrete output. ASCII has a separate nonvisual theme contract and returns `not-applicable`.
Root Canvas queries remain `unverified`; that does not imply that recipe canvas paint cannot render.

## Read execution evidence and target admission separately

For Web, `renderSvgResult()` returns `{ svg, metadata }` from one execution. Inspect
`metadata.theme_execution_evidence`; string-only `renderSvg()` omits this evidence, and
`svgPlanJson().ready` describes artifact capabilities rather than applied theme facets.
The Rust facade exposes `RenderEvidence` and a target-owned `TargetAdmissionReceipt`.

The current schemas have separate owners:

| Contract | Current version and source |
| --- | --- |
| Editable recipe | `schema_version: 1`, tagged `design` or `complete_spec`; [recipe contract](../../crates/merman-theme-contract/src/theme_recipe.rs). |
| Support query/result and preset catalog | Schema 1; [support contract](../../crates/merman-theme-contract/src/support.rs) and [catalog metadata](../../crates/merman-theme-contract/src/preset_catalog.rs). |
| Binding execution evidence | `version: 2`; [binding projection](../../crates/merman-bindings-core/src/theme_execution_evidence.rs). |
| Qualification record/archive companion | Schema 2; [qualification guide](preset-qualification.md). Production catalog schema stays independent. |

Execution evidence keeps `theme_status` separate from `target_status`, target reasons, font source
and `output_mutated`. Theme `verified` means required root/family work was accounted for in this
execution; it does not certify an arbitrary scene or output. `not_applicable` means no theme evidence
was required, `residual` retains unresolved work or mutation, and `incomplete` means required
accounting did not finish. Output mutation prevents an otherwise verified result from being satisfied.

`font_source` describes the output target's font source, not the text-measurement provider.
SVG can report `none` after host or native measurement because its text still resolves fonts
in the receiving viewer. Theme verification does not prove that named fonts are installed,
every glyph is covered, or every measurement avoided fallback. See
[host measurement evidence](../bindings/HOST_TEXT_MEASUREMENT.md#text-measurement-protocol-summary).

Target `portable` is the target adapter's admission for this operation. `host_dependent`
(**HostDependent**) retains conditions such as host text measurement or system fonts; it is not
portable certification. `rejected` records failed target admission; `unverified` means compatibility
was not observed. BestEffort may return useful output with rejected admission. `require-portable`
requires Portable and returns a structured error otherwise; selecting `resvg-safe` alone does not
set that policy. Successful SVG output cannot qualify PNG, JPEG, PDF or another browser environment.

Diagnostics explain unapplied settings. A rule can consume stroke while retaining an unsupported
radius; a rule-level diagnostic does not declare every facet failed. Source paths are JSON Pointers
relative to the named recipe payload, and generated defaults can have no authored location.
Fully overridden requests should not appear as unapplied. Empty diagnostics do not certify Portable;
a missing list supplies no explanation. Preserve unknown versions, statuses and diagnostic IDs
without upgrading them to support or portability.

## Respect document and output conditions

Source styles and explicit Mermaid configuration retain property-specific precedence. An authored
source theme supplies its materialized paint palette; site/base themes and the recipe's compatibility
theme supply fallbacks. Canvas, typography, geometry and effects retain their independent owners.
Follow the [precedence rules](custom-diagram-themes.md#family-configuration-and-compiled-rules) when
reproducing a render, including where each configuration value originated.

A selected route is not proof of emission: missing labels, unmatched ordinals, source-owned paint,
shape geometry and look selection can change what actually applies. Partial support is local to
the target, property and value; it does not make a whole family supported or unsupported.
For example, Flowchart Classic and Neo apply basic-shape glow/shadow bindings, while hand-drawn
and other shapes can retain residual effects. Theme selection preserves the authored look.
Native fonts, HTML labels, filter composition, clipping and resource
ceilings still require the concrete target's evidence. Keep residuals visible when a sibling facet
applies; do not silently substitute a preset, drop effects or relax budgets.

Flowchart and Agentflow HTML labels can retain source-owned relative font sizes such as `2em`
on several nested ancestors. Their measured and inherited browser sizes can differ. These
occurrences retain a source residual and fail strict portability; an independently applied
theme font family does not certify that source size. Absolute typed role sizes use the prepared
label measurement and its matching writer evidence.

## Qualification covers named scenarios

The built-in production catalog currently leaves all `qualified_cells` empty. Availability,
dedicated design scope, a successful admission inventory and a constructed metadata cell cannot
issue qualification. Fresh runner-owned evidence may project artifact-local cells bound to
`family_id`, `output_id`, `profile_id` and `admission_status`, with matching source, recipe, artifact
and host identities. A downloaded companion needs trusted provenance and matching artifact identity.

The declared native profiles cover Brutalist/Spotless Flowchart, State and Sequence, and Cyberpunk
Flowchart, Sequence and XY Chart, as SVG and PNG. They use system fonts, native SVG labels and
explicit Dagre/Classic conditions where specified; successful observations remain HostDependent.
They do not qualify default Neo, HTML labels, PDF, arbitrary diagrams or other installations.
Use the [qualification runbook](preset-qualification.md) for exact scenes, pixel checks, archive
replay and freshness requirements. Neither private acceptance nor installed-consumer smoke widens
that scope or populates the production catalog automatically.

## Current implementation and verification owners

- [Mechanism classification](../../crates/merman-render/src/diagram_theme/family_mechanism_matrix.rs)
  and its [support projection](../../crates/merman-render/src/diagram_theme/support_manifest.rs)
  own discovery claims. [Discovery tests](../../crates/merman-render/tests/theme_support_discovery_test.rs)
  cover partial domains, unknown IDs, feature availability and output boundaries.
- [Root plans/outcomes](../../crates/merman-render/src/diagram_theme/application.rs) and
  [family programs](../../crates/merman-render/src/diagram_theme/family_program.rs) own their compiled
  request scopes; family writers and [terminal accounting](../../crates/merman-render/src/family/evidence_support.rs)
  own actual application. [Facade coverage tests](../../crates/merman/tests/diagram_theme_coverage.rs)
  check visible consumers and source precedence.
- The [font exchange corpus](../../crates/merman/tests/theme_font_capability.rs) compares its logical
  family set with the runtime parser catalog, so registering a new family also requires a fixture.
  The facade also runs all built-in presets over that corpus and rejects missing family accounting
  or unsupported core paint. These gates do not establish arbitrary selector, effect or output support.
- [Render evidence](../../crates/merman/src/render/evidence.rs) combines root/family outcomes;
  [target admission](../../crates/merman/src/render/target_admission/mod.rs) classifies concrete
  artifacts. [Bindings](../../crates/merman-bindings-core/src/theme_execution_evidence.rs) project
  those outcomes and diagnostics without creating a second certification authority.
- The non-published [qualification runner](../../crates/merman-theme-acceptance/src/preset_qualification.rs)
  owns scoped SVG/PNG semantic and visual checks. Production generation does not run that observer.
  Platform/package owners separately verify installed consumers and artifact identity.
