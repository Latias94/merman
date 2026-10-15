# Merman PR #28 Theme Contribution Ledger

Date: 2026-08-07

Plan: `docs/plans/2026-08-06-001-refactor-portable-diagram-theme-architecture-plan.md`

This ledger preserves the engineering and visual-design value of Merman PR #28 while the
unreleased alpha.4 presentation API is replaced. It is a migration record, not a compatibility
promise for the provisional `merman-modern` identifier or its mixed ownership model.

## Provenance

| Field | Value |
| --- | --- |
| Pull request | Merman PR #28, `add an opt-in modern ELK flowchart profile` |
| Contributor | Ophir LOJKINE (`@lovasoa`) |
| Contribution base | `5bfb76828fc4dd7a2370f027392ae815af48a857` |
| Contributor tip | `687f9ff3f04321482a4b9135edbf5e3bc5ce0530` |
| Merge commit | `029e92f1f7ca3a8dffda08bbc46c7f6daaaf8f31` |

The contribution range contains these eight contributor-authored commits:

| Commit | Subject |
| --- | --- |
| `e5b5ff4a8` | `fix(elk): enforce forced node model order` |
| `605437b7d` | `fix(elk): preflight unsupported processors` |
| `e15daa8a7` | `feat(flowchart): honor neo corner styling` |
| `d2a1de621` | `feat(render): add modern flowchart profile` |
| `c19c6b930` | `fix(flowchart): refine modern edge styling` |
| `30fe4967c` | `fix(flowchart): polish modern geometry` |
| `6868706a6` | `fix(flowchart): align modern ELK endpoints` |
| `687f9ff3f` | `docs(flowchart): add modern profile comparisons` |

Any retained example or derived preset must include this attribution:

> Based in part on the `merman-modern` Flowchart work contributed by Ophir LOJKINE (`@lovasoa`) in Merman PR #28.

## Decision Vocabulary

- **KEEP** preserves a generally correct mechanism in its natural renderer or layout owner. The
  mechanism must not require a presentation profile to become correct.
- **MOVE** preserves a useful capability or design value after relocating it to the typed owner
  that can validate and compose it. The provisional key, fallback, or aggregate is not preserved.
- **DELETE** removes a mixed or misleading contract without a compatibility shim. Evidence and
  attribution remain even when the old API does not.

## Commit Disposition

| Commit | Decision | Migration result |
| --- | --- | --- |
| `e5b5ff4a8` | KEEP | Forced node model order remains ELK layered-layout correctness. It belongs to `merman-elk-layered` and the `merman-layout-elk` option mapping, independent of any visual theme or named profile. |
| `605437b7d` | KEEP | Unsupported-processor preflight remains an atomicity guarantee of the ELK pipeline. The pipeline rejects an unsupported execution plan before mutating caller-owned graph state. |
| `e15daa8a7` | KEEP + MOVE | Keep source-backed Neo rendering behavior. Move visual radius, shadow, and corner values into typed semantic theme geometry/effects; do not retain private Mermaid-config keys as the customization surface. |
| `d2a1de621` | DELETE + MOVE | Delete the mixed `MermanModern` profile and discovery contract. Move `look: neo` and ELK selection to explicit renderer/layout configuration, and move its visual choices to a separately selected theme. |
| `c19c6b930` | MOVE | Move palette, edge paint, label background, and content padding to `DiagramTheme`. A compact-routing policy may survive only as typed Flowchart configuration after independent usefulness is proven. |
| `30fe4967c` | KEEP + MOVE | Keep correct Neo shape sizing, marker placement, and terminal geometry. Move theme geometry to `DiagramTheme`; move genuinely routing-specific corner compaction to typed Flowchart configuration if retained. |
| `6868706a6` | DELETE + MOVE | Delete the current radial-membership projection heuristic and preserve its direct-routing design intent for a future typed Flowchart mode. Mermaid 11.16.1 deliberately retains its cutter-generated endpoint adapter jog, while the contributed heuristic is not correct for every supported shape, self-loop, or compound endpoint. |
| `687f9ff3f` | MOVE | Retain source fixtures and comparison evidence, but rewrite the stale profile invocation as an attributed `pr28-modern-slate` composition example using explicit Neo, ELK, theme, and optional Flowchart configuration. |

## Capability Ownership Ledger

| Capability or artifact | Decision | Final owner | Required evidence or rejection rationale |
| --- | --- | --- | --- |
| Forced real-node model order during ELK crossing minimization | KEEP | `merman-elk-layered` algorithm, surfaced through `merman-layout-elk` options | Preserve focused algorithm tests such as `forced_model_order_keeps_real_nodes_in_input_order` and `forced_model_order_still_places_dummy_nodes_by_barycenter`. Theme selection must not affect their result. |
| Model-order propagation through the Flowchart ELK adapter | KEEP | Flowchart-to-ELK layout adapter | Keep integration evidence that source order reaches the ELK graph and survives compound-edge ownership/flattening. This is layout correctness, not a preset feature. |
| Unsupported ELK processor rejection before graph mutation | KEEP | ELK pipeline planning/preflight | Preserve unit coverage for `PipelineError::UnsupportedProcessor` and facade coverage such as `source_ported_layout_rejects_unported_routing_before_layout`. Failure must be atomic. |
| Official Mermaid `look: neo` selection | MOVE | Explicit `MermaidConfig` precedence | The host opts in with `look: neo`; a visual theme or behavior convenience must not select it implicitly. Existing Neo renderer tests remain source-parity evidence. |
| Flowchart ELK renderer selection | MOVE | Explicit Flowchart renderer/layout configuration | The host opts in with `flowchart.defaultRenderer: elk` or its typed equivalent. Non-Flowchart operations must not acquire an ELK requirement from theme selection. |
| Redux base plus the contributed slate/purple palette | MOVE | `DiagramTheme` example or future `ThemePreset` | Materialize the values as the attributed `pr28-modern-slate` visual recipe. Do not conditionally inject it when no other theme is selected, and do not let it select Neo, ELK, or an SVG pipeline. |
| Node, cluster, edge, arrowhead, label, and text paint | MOVE | Typed semantic `DiagramTheme` rules | Each value needs a semantic target and fixture witness. Raw `themeVariables` may remain only in the Mermaid-compatibility lane, not as proof that the portable theme model covers the mechanism. |
| Node radius and other layout-affecting shape geometry | MOVE | `DiagramTheme` geometry resolved before layout | Geometry must participate in measurement and bounds before layout freezes. It cannot be a paint-only postprocess or a hidden profile default. |
| Neo shape-specific content padding and sizing | KEEP | Source-backed Flowchart Neo shape adapter | Preserve Neo/classic split tests such as `neo_uses_pinned_shape_specific_content_padding` and `classic_shape_padding_remains_unchanged`. Theme-owned overrides compose through the pre-layout geometry resolver. |
| Edge-label content padding | MOVE | `DiagramTheme` semantic style for Flowchart edge labels | Padding is measurement-affecting and must change prepared label bbox and layout geometry before SVG emission. The private `flowchart.edgeLabelPadding` Mermaid-config key is deleted. |
| Edge-label route or mask clearance | MOVE or DELETE | Typed Flowchart configuration, only if independently useful | Keep separate from content padding. Admit it only with route-level tests proving a useful renderer behavior; otherwise delete the profile-specific clearance tweak. |
| Rounded edge path radius | MOVE | Theme geometry for visual radius; Flowchart routing configuration for route policy | The radius value and the decision to compact a route are separate. Theme compilation owns visual geometry; the renderer owns how a route remains valid around short segments and endpoints. |
| Compact rounded-corner routing | MOVE or DELETE | Typed Flowchart configuration | Retain only if a user can select it without a named theme and tests prove it does not overrun shallow turns. The private `flowchart.compactEdgeCorners` key and profile-only policy are deleted. |
| Marker offsets | KEEP | Flowchart SVG edge geometry | Preserve the source-backed marker shortening behavior independently of any visual preset. Existing marker-offset regressions remain parity evidence. |
| Short terminal marker-stub collapse | MOVE or DELETE | Typed Flowchart routing configuration | This is part of the compact-routing alternative rather than Mermaid's default path. Retain it only with short/long terminal-stub regressions and an independently useful typed routing mode. |
| ELK terminal route projection directly to rendered shape boundaries | DELETE + MOVE | Future typed Flowchart routing configuration | Delete `align_elk_endpoint_adapters_to_route`: its radial-distance membership test assumes a center-star-shaped outline and monotonic inward search, which is not valid for all supported concave/special shapes and does not safely cover self-loops or compound endpoints. Preserve the product intent and comparison fixture. Any replacement must use exact route-segment/visible-outline intersection, fail closed to the Mermaid jog for unsupported cases, and validate finite ordered points after projection. The default jog remains covered by `headless_renderer_keeps_flowchart_elk_cutter_jog_for_straight_shape_edge`. |
| Profile-specific palette fallback detection | DELETE | No replacement | The heuristic existed only to decide whether Redux/slate defaults should be injected. Explicit theme selection makes the fallback and its `site_config` theme-detection branch unnecessary. |
| `PresentationProfile::MermanModern` and `merman-modern` Options JSON value | DELETE | No mixed replacement type | The type combines visuals, renderer choice, look, and routing policy under one name. Alpha.4 is unreleased, so no deprecated forwarding wrapper or alias is retained. |
| Presentation aspect catalog and runtime discovery entry | DELETE | Explicit theme and renderer/layout descriptors | Discovery reports actual theme presets, renderer/layout options, family configuration, and capabilities. It must not advertise a hidden `merman-modern` aggregate. |
| `ResolvedPresentation` / `PresentationRenderPolicy` profile plumbing | DELETE | Existing config precedence plus compiled `DiagramTheme` and typed family configuration | Values move to their real owners. A shallow wrapper that merely carries a profile identity is not a durable abstraction. |
| Profile comparison `.mmd` fixtures and screenshots | MOVE | Attributed product example and regression fixtures | Keep useful diagrams and historical comparisons. Regenerate current outputs from the explicit composition and remove instructions that require the deleted profile. |
| Application annotations, editor overlays, or arbitrary element-ID styling | DELETE from core theme scope | Host application or Mermaid source compatibility lane | PR #28 does not justify application UI state in the portable theme model. Semantic variants and stable ordinal palettes are supported; arbitrary element-ID selectors are not. |

## Attributed Composition Target

`pr28-modern-slate` is the required traceable composition name. It is initially a documented
example and may later become a first-party theme preset only after the semantic theme fixtures
cover every retained visual mechanism. Promotion does not remove or weaken the attribution.

The example is a composition of independent inputs:

1. Explicit Mermaid configuration selects `look: neo`.
2. Explicit Flowchart renderer/layout configuration selects ELK.
3. A `DiagramTheme` supplies the contributed slate/purple palette, semantic paint, label styling,
   and theme-owned geometry.
4. Optional typed Flowchart configuration selects compact routing, clearance, or exact
   direct-to-boundary routing only after those controls survive their independent correctness and
   usefulness reviews.
5. The caller selects an SVG output policy independently.

The example is not a public aggregate type, is not an implicit fallback, and is not a runtime
discovery entry named `merman-modern`. Selecting its visual theme must not change renderer, layout,
or output-policy behavior.

## Removal Contract

The following names and encodings are removed rather than deprecated:

- `PresentationProfile::MermanModern`;
- the `merman-modern` presentation/profile request value;
- profile/aspect descriptors that expose the mixed aggregate;
- conditional Redux/slate defaults;
- private Mermaid-config customization keys such as `flowchart.edgeLabelPadding` and
  `flowchart.compactEdgeCorners`;
- the private radial-membership `align_elk_endpoint_adapters_to_route` heuristic;
- examples and documentation that imply one profile owns theme, renderer, layout, and output.

Removal does not erase the contribution. The algorithms, correctness fixes, fixtures, visual
recipe, provenance hashes, and attribution remain independently reviewable.

## Migration Gates

This ledger is satisfied only when all of the following are true:

- forced model order and unsupported-processor preflight pass without any presentation selection;
- the default ELK path retains Mermaid 11.16.1's cutter-generated endpoint jog without any theme
  or presentation selection;
- the radial-membership endpoint projection heuristic is absent from runtime code;
- any future direct-to-boundary mode uses exact visible-outline intersection and passes source,
  target, marker, self-loop, compound, and representative special-shape regressions before it is
  exposed through typed Flowchart routing configuration;
- every retained PR #28 visual mechanism maps to a typed theme capability and fixture witness;
- `pr28-modern-slate` renders from explicit Neo, ELK, theme, and optional Flowchart inputs and
  carries the required attribution;
- selecting the visual theme alone does not select ELK, Neo, compact routing, or an SVG pipeline;
- no public Rust, CLI, binding, Options JSON, runtime catalog, example, or current-facing document
  exposes `PresentationProfile::MermanModern` or a `merman-modern` aggregate;
- the historical source/fixture evidence remains traceable to the contribution base, contributor
  tip, merge commit, and the eight contributor-authored commits above.
