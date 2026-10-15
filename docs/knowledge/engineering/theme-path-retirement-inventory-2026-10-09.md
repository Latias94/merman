# Theme path retirement inventory

Status: initial planning snapshot, captured October 9, 2026, before the family cutovers;
final deletion closure added October 10 at runtime source `dd79ca584`.

## Final deletion closure

The final source/reference audit at `3a11f46ba` covered all 35 renderer families;
the sharing follow-up at `dd79ca584` changes nine implementation files without
restoring a reader or provider. The pushed branch source at `32e2588e8` contains no
`SvgTheme`, `MermaidThemeAdapter`, `FamilyPaintDefaultPaths`, historical bridge
module or retirement authorization caller. This is a source audit, not proof
that every future arbitrary JSON reader would be detected automatically.

| Deleted closure | Current owner and retained contract |
| --- | --- |
| `merman-render/src/theme.rs`, parity theme family readers and old exports | Core Mermaid derivation remains in `merman-core/src/theme.rs`; family preparation supplies concrete paint, typography, raw CSS and provenance bindings. Parity theme helpers retain only fixed default palettes. |
| `family/parse_defaults.rs`, `FamilyPaintDefaultPaths` and default eligibility plumbing | Family bindings preserve explicit source/site ownership before applying typed fallback; parser/layout configuration stays with its original owner. |
| `svg/parity/class/settings.rs` | Class preparation retains final node/interface, relation, namespace and typography plans; Class CSS/writers consume those prepared facts. |
| `legacy_family_theme_bridge.rs`, `legacy_projection_retirement.rs`, `legacy_tombstones.rs`, private exports | Current route classification and family paint evidence remain; no lazy legacy provider/cache or historical witness dispatch survives. |
| Acceptance `cutover_manifest.rs`, `route_retirement_manifest.rs`, retirement integration target, fixed Class CSS witnesses | Preset qualification, support discovery and actual Block/Class/Flowchart paint tests retain live contracts. Historical authority and CI/release commands are removed together. |
| Renderer `theme_raster_paint.rs`, exporter `raster_paint_cutover.rs`, private facade pair/digest retention | Ordinary native encoding, resource admission, filter receipts and current emitted-paint evidence remain. The unused optional exporter-to-`sha2` dependency edge is removed from workspace and fuzz lockfiles. |

The exact 16 deleted paths are reproducible with
`git diff --name-status f8160267a3e905eb2d9d6c0b0c980d9fa1f07657..32e2588e8 --diff-filter=D`.
Deleted symbols in retained files belong to the same closures above. Historical
source/digests and retained current checks are detailed in the
[gate removal record](verification/2026-10-09-theme-retirement-gate-removal.md).

### Final family owner partition

The table covers the 35 `RenderFamilyKind` variants. Concrete family artifacts
are prepared through `crates/merman-render/src/family/preparation.rs` and
family-local preparation/binding modules. They preserve each family's existing
support limits; this inventory does not qualify additional targets or effects.

| Family/families | Prepared facts consumed by the final writer |
| --- | --- |
| Flowchart, Swimlane, Agentflow | Shared Flowchart artifact retains family-specific palettes, node/cluster/source terminals, prepared class styles, Rough wrapper style, render settings and occurrence-owned edge typography. |
| Sequence | Prepared settings, actor/message/control/rect terminal styles and effect selection; writer geometry and actual emission counts remain downstream. |
| Class | Node/interface visual index, relations, namespaces, labels and typography plans. |
| Mindmap | Node palette and prepared node/edge/typography bindings. |
| State | State style plan supplies final node/edge/class and marker declarations. |
| ER | Entity/row/text/subgraph/relation bindings preserve explicit source ownership. |
| Block | Node/edge/marker/background and typography plans plus ordered, pre-parsed authored class declarations. |
| Requirement | Final actual-node/source and relation terminals. |
| C4 | Element text/paint and prepared relation/default colors. |
| Sankey | Prepared node palette, links and typography; value prefixes and viewport settings remain content/layout configuration. |
| GitGraph | Terminal styles and explicit ownership; title geometry and URL serialization remain downstream. |
| Treemap | Prepared section/leaf/node visuals; fitted font size remains geometry-dependent. |
| Venn | Prepared area visuals; Rough numeric realization remains a backend operation. |
| Usecase | Prepared source/visual forms; label markup and escaping remain serialization. |
| Wardley | Prepared paint binding and root disposition. |
| Error, Info, Zenuml, Architecture, Cynefin, Railroad, Kanban, Gantt, Pie, Packet, Timeline, Journey, Radar, QuadrantChart, XYChart, TreeView, Ishikawa, EventModeling | Existing family-local prepared paint/typography bindings consume compatibility input before writer use. Pie's `highlightSlice` selects interactive classes, not paint ownership. |

Final source inspection distinguishes preparation-time resolution from lawful
downstream work: geometry-dependent effect realization, HTML/native safety,
resource/cancellation checks, residual reporting, escaping, references and actual
emission receipts remain. No universal CSS interpreter, global cache, fixture
allowlist or replacement compatibility provider was introduced.

The current matrix invariant enumerates typography, rule facets, ordinal palettes
and effect routes and rejects `LegacyCompatibility`; it is not a constant-empty
historical inventory. It does not detect every possible reader bypassing that
matrix. The complementary source/reference audit and family owner tests establish
the current deletion boundary. Public package checks also reject retired imports.
The [measurement report](../../performance/theme_path_retirement_2026-10-10.md)
and its separate initial/shared-source archives identify each validation revision.

## Initial inventory

This inventory records the initial source owners and work partition for the approved unified
theme lowering refactor. Its file lists and obligations describe that starting point, not the
current implementation or executable CI gates. Removed paths remain below as historical evidence.
See the [retirement gate removal record](verification/2026-10-09-theme-retirement-gate-removal.md)
for the deleted migration authority, and the [theme guide](../../rendering/custom-diagram-themes.md)
for current user interfaces. Public Mermaid input, ordered core derivation, source filtering,
and configuration serialization remain supported.

## Visual readers

Lower family compatibility input once; remove consumers and reader definitions after complete family cutover.

Initial reference files: 36. Counts describe files, not semantic routes.

- `crates/merman-render/src/eventmodeling.rs`
- `crates/merman-render/src/quadrantchart.rs`
- `crates/merman-render/src/radar/axis_paint.rs`
- `crates/merman-render/src/radar/text_paint.rs`
- `crates/merman-render/src/radar/theme.rs`
- `crates/merman-render/src/svg/parity.rs`
- `crates/merman-render/src/svg/parity/block/render.rs`
- `crates/merman-render/src/svg/parity/class/css.rs`
- `crates/merman-render/src/svg/parity/css.rs`
- `crates/merman-render/src/svg/parity/er/render.rs`
- `crates/merman-render/src/svg/parity/eventmodeling/render.rs`
- `crates/merman-render/src/svg/parity/flowchart/css.rs`
- `crates/merman-render/src/svg/parity/flowchart/render/cluster.rs`
- `crates/merman-render/src/svg/parity/flowchart/render/edge_label.rs`
- `crates/merman-render/src/svg/parity/ishikawa/render.rs`
- `crates/merman-render/src/svg/parity/journey/render.rs`
- `crates/merman-render/src/svg/parity/kanban/render.rs`
- `crates/merman-render/src/svg/parity/mindmap/render.rs`
- `crates/merman-render/src/svg/parity/radar/render.rs`
- `crates/merman-render/src/svg/parity/requirement/render.rs`
- `crates/merman-render/src/svg/parity/sequence/actor_effect.rs`
- `crates/merman-render/src/svg/parity/sequence/actor_shapes.rs`
- `crates/merman-render/src/svg/parity/sequence/css.rs`
- `crates/merman-render/src/svg/parity/sequence/frames.rs`
- `crates/merman-render/src/svg/parity/theme.rs`
- `crates/merman-render/src/svg/parity/theme/families.rs`
- `crates/merman-render/src/svg/parity/theme/tests.rs`
- `crates/merman-render/src/svg/parity/timeline/render.rs`
- `crates/merman-render/src/svg/parity/tree_view/render.rs`
- `crates/merman-render/src/svg/parity/treemap/render.rs`
- `crates/merman-render/src/svg/parity/util.rs`
- `crates/merman-render/src/svg/parity/venn/render.rs`
- `crates/merman-render/src/theme.rs`
- `crates/merman-render/src/tree_view/config.rs`
- `crates/merman-render/src/xychart.rs`
- `crates/merman-render/src/xychart/series.rs`

## Historical retirement

Retire private authorization, callers, tests and release wiring together; preserve compact historical evidence.

Initial reference files: 8. Counts describe files, not semantic routes.

- `crates/merman-render/src/diagram_theme/legacy_family_theme_bridge.rs`
- `crates/merman-render/src/diagram_theme/legacy_projection_retirement.rs`
- `crates/merman-render/src/diagram_theme/mod.rs`
- `crates/merman-render/src/lib.rs`
- `crates/merman-theme-acceptance/src/lib.rs`
- `crates/merman-theme-acceptance/src/route_retirement_manifest.rs`
- `crates/merman-theme-acceptance/tests/legacy_projection_retirement.rs`
- `crates/merman/src/theme_acceptance.rs`

## Post-detection default ownership

Transfer property ownership semantics to binding; delete compensating default-path override only after equivalent behavior exists.

Initial reference files: 9. Counts describe files, not semantic routes.

- `crates/merman-core/src/config/mod.rs`
- `crates/merman-core/src/config/overlay.rs`
- `crates/merman-core/src/lib.rs`
- `crates/merman-core/src/tests/detect.rs`
- `crates/merman-render/src/er/theme.rs`
- `crates/merman-render/src/family.rs`
- `crates/merman-render/src/family/parse_defaults.rs`
- `crates/merman-render/src/gitgraph/theme/node_paint.rs`
- `crates/merman-render/src/requirement/relation_paint.rs`

## Semantic deletion conditions

- Every current family is assigned to a concrete lowering unit, including requests with no typed recipe.
- Raw CSS emission, optional native/measurement interpretation, clear versus absent state, importance and declaration provenance survive lowering.
- Core configuration and parser/layout settings retain their current owner; a visual reader deletion does not imply deleting all JSON configuration reads.
- Unsupported owning source declarations still suppress typed fallback and produce existing residual behavior.
- Each removed compatibility API is traced through published exports, production feature closures, test closures, private acceptance, and release workflows.
- Current structural and behavior checks replace migration-specific authorization; security/resource/native validation is retained.
- No performance claim follows from reference counts or lines removed.

## Core construction closure

Repository caller inspection distinguishes the retired provider path from live ownership capture:

- `ThemeCompatibilityPlan::try_new`, `ThemeFamilyCompatibilityOverlayBuilder`, the contribution receipt and lazy resolver are constructed only by tests. Their dispatch and test fixtures can retire together.
- The provider-free plan constructor remains the compiler entry point. Recipe identity, bounded compatibility admission, Mermaid derivation and public internal-error classification remain supported.
- GitGraph node paint and ER text/line/row paint still consume pre-materialization default eligibility. Transfer these decisions before deleting capture or its explicit-config input.
- Requirement requests default eligibility but reads general explicit ownership instead. Remove the request alongside its family migration, while preserving source/site precedence.
- TreeView's fallback contribution query has no production producer after lazy-provider retirement. Remove that query with its obsolete evidence closure.
- Diagnostic fixtures in the facade, analysis and bindings must continue to test internal-error sanitization through a supported error constructor after provider fixtures retire.

## CI owners at plan capture

At plan capture, the Ubuntu `build-test` job in `.github/workflows/ci.yml` ran full-workspace nextest, architecture doctests, private theme acceptance, preset qualification, native export, theme authoring and Flowchart geometry/effects. Pull-request host runners separately executed the historical retirement integration target. U5 subsequently removed that obsolete host target with its test and release wiring; consult the removal record above rather than restoring this initial gate list.

Tests named `block_title_legacy_projection`, `class_edge_label_background_legacy_projection` and `flowchart_marker_legacy_projection` protect current emitted paint and precedence behavior; a historical filename alone does not authorize deleting those assertions. Font admission, FFI consumers, release-mode evidence rejection and the published acceptance boundary protect independent current contracts.

This inventory is source-backed planning evidence, not a claim that any CI or Cargo check has passed for the refactor.

## Initial complete family partition

The source of truth is all 35 variants of `RenderFamilyKind`, including C4 and Treemap. U3 owns Class, C4, Gantt, Pie, Kanban, Timeline, Journey, QuadrantChart, Venn, Radar, XYChart, TreeView and Treemap (13). U4 owns Flowchart, Swimlane, Agentflow, Sequence, State, Mindmap, ER, Block, Requirement, EventModeling, Ishikawa, Architecture, GitGraph, Sankey, Railroad, Error, Info, Cynefin, Wardley, Zenuml, Packet and Usecase (22). These sets are disjoint and cover the source enum.

At plan capture, typed preparation plans existed for many families, but that alone did not establish a terminal cutover: common CSS and several family writers still independently read effective configuration. Class was the first bounded preparation unit; its initial raw CSS binding and static paint cache did not yet remove typed/source ownership branches or the shared Info CSS reader. This paragraph records the initial obligations, not the completion status of subsequent family cutovers.
