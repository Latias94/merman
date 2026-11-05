# Diagram Theme Coverage

Use the loaded runtime's discovery and the result of the actual render to assess theme support.
The [authoring guide](custom-diagram-themes.md) explains recipes and source precedence;
[preset qualification](preset-qualification.md) defines the bounded visual checks. This page
explains those evidence boundaries. Machine contracts and runtime source own the capability inventory.

## Discover support in the artifact being used

Read the runtime catalog before selecting a family, preset or output. `available` means the artifact
can compile a preset. A catalog family treatment of `dedicated` or `base_only` describes design
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
- [Render evidence](../../crates/merman/src/render/evidence.rs) combines root/family outcomes;
  [target admission](../../crates/merman/src/render/target_admission/mod.rs) classifies concrete
  artifacts. [Bindings](../../crates/merman-bindings-core/src/theme_execution_evidence.rs) project
  those outcomes and diagnostics without creating a second certification authority.
- The non-published [qualification runner](../../crates/merman-theme-acceptance/src/preset_qualification.rs)
  owns scoped SVG/PNG semantic and visual checks. Production generation does not run that observer.
  Platform/package owners separately verify installed consumers and artifact identity.
