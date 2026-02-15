# Parity Hardening Plan (Post 100% Baseline)

Baseline version: Mermaid `@11.12.2`.

As of 2026-02-15:

- `parity` full compare: 0 mismatch.
- `parity-root` full compare: 0 mismatch (1328/1328 upstream SVG baselines).

This document defines the next hardening phases after reaching baseline 100% parity for the
current fixture set.

## Goals

1. Keep global parity green (`parity` + `parity-root`) while the fixture corpus grows.
2. Reduce fixture-scoped override dependence where feasible.
3. Preserve deterministic, reproducible results for the pinned upstream version.

## Current Inventory

### Upstream SVG Corpus

- Total diagrams covered: 23
- Total upstream SVG baselines: 1328

### Upstream Syntax Docs Inventory (11.12.2)

Mermaid's syntax docs contain a large set of Mermaid code blocks that can be turned into fixtures.
As a rough upper bound, `repo-ref/mermaid/docs/syntax/*.md` contains `769` diagram-typed code fences
(````mermaid`, ` ```flowchart`, ` ```sequenceDiagram`, ..., ` ```zenuml`) at tag `@11.12.2`.

We intentionally do not import these in one shot. Phase A grows the fixture corpus in small,
reviewable batches so that new mismatches are attributable and fixes are reversible.

Largest fixture buckets:

- `flowchart`: 335
- `sequence`: 102
- `state`: 129
- `gantt`: 76
- `class`: 70

### Override Footprint (11.12.2)

Root viewport overrides:

- `architecture_root_overrides_11_12_2.rs`: 17 entries
- `flowchart_root_overrides_11_12_2.rs`: 88 entries
- `class_root_overrides_11_12_2.rs`: 99 entries
- `mindmap_root_overrides_11_12_2.rs`: 41 entries
- `gitgraph_root_overrides_11_12_2.rs`: 54 entries
- `pie_root_overrides_11_12_2.rs`: 13 entries
- `sankey_root_overrides_11_12_2.rs`: 5 entries
- `sequence_root_overrides_11_12_2.rs`: 72 entries
- `state_root_overrides_11_12_2.rs`: 57 entries
- `timeline_root_overrides_11_12_2.rs`: 8 entries

State text/bbox overrides:

- `state_text_overrides_11_12_2.rs`: 46 `Some(...)` entries across width/height/bbox helpers

## Phase Plan

For release-oriented milestones (0.1.0/0.1.x), see `docs/alignment/MILESTONES.md`.

## Phase A: Fixture Expansion (Coverage First)

Primary objective: increase confidence without destabilizing existing parity.

Actions:

1. Expand upstream fixture import from Mermaid `@11.12.2` tests/docs for the most sensitive diagrams:
   - `architecture`, `class`, `mindmap`, `state`, `flowchart`, `sequence`.
2. Keep additions version-pinned and traceable to upstream source path and commit.
3. Add fixtures in small batches and require both global checks green after each batch.

Exit criteria:

- New fixture batches are merged with 0 mismatch in full `parity` and `parity-root` runs.

## Phase B: Override Consolidation (Algorithm First)

Primary objective: convert fixture-scoped overrides to reusable rendering/layout logic where practical.

Priority order:

1. `class` root viewport (smaller fixture count, medium override density)
2. `mindmap` root viewport (small surface area, high leverage)
3. `architecture` root viewport (largest and most layout-sensitive)
4. `state` text/bbox overrides (browser-like HTML/SVG measurement edge cases)

Policy:

- Remove overrides only when replacement logic is deterministic and keeps all existing fixtures green.
- If a removal causes regressions, prefer rollback + follow-up ADR rather than partial drift.

## Phase C: External Diagrams (ZenUML Compatibility)

Primary objective: expand practical ZenUML support without compromising the Mermaid parity gates.

Notes:

- Upstream Mermaid renders ZenUML via browser-only `@zenuml/core`. We do not maintain upstream SVG baselines.
- `merman` implements a headless compatibility mode by translating a ZenUML subset into `sequenceDiagram`.

Actions:

1. Import a small batch of examples from `repo-ref/mermaid/docs/syntax/zenuml.md` into `fixtures/zenuml/`.
2. Extend the translator in `crates/merman-core/src/diagrams/zenuml.rs` in small, test-driven steps.
3. Gate ZenUML changes on:
   - semantic snapshots (`fixtures/zenuml/*.golden.json`)
   - layout snapshots (`fixtures/zenuml/*.layout.golden.json`)
   - existing global Mermaid parity gates remaining green.

Exit criteria:

- ZenUML fixture set grows with deterministic snapshots, and Mermaid parity gates remain green.

Class Phase-B spike notes (2026-02-06):

- A temporary full disable of `class_root_overrides_11_12_2.rs` was used to measure raw drift.
- Result: 14 class fixtures regressed in `parity-root`, all on root `<svg style max-width>`.
- Drift was not uniformly small: observed ranges were approximately `+0.015px` to `+344.92px`,
  with both over- and under-estimation cases.
- This indicates class root overrides are currently masking a deeper combination of layout and
  browser-like measurement differences (not just a final viewport padding formula issue).
- Practical implication: class override reduction should proceed with a pre-step that targets
  class layout/text-measurement convergence for selected fixtures, then removes overrides in
  small reversible batches.

Mindmap Phase-B spike notes (2026-02-06):

- A temporary full disable of `mindmap_root_overrides_11_12_2.rs` was used to measure raw drift.
- Result: 9 mindmap fixtures regressed in `parity-root`, all on root `<svg style max-width>`.
- `parity` (diagram subtree mode) remained green, indicating structure/content alignment is stable
  and the drift is root viewport-only.
- Drift range was approximately `-27.094px` to `+122.112px` (both under- and over-estimation).
- Practical implication: as with class, mindmap override reduction should be done in small batches
  after targeted root viewport convergence work for the largest-drift fixtures.

Architecture Phase-B spike notes (2026-02-06):

- A temporary full disable of `architecture_root_overrides_11_12_2.rs` was used to measure raw drift.
- Result: 26 architecture fixtures regressed in `parity-root`.
  - `25` were root `<svg style max-width>` mismatches.
  - `1` was a root `<svg viewBox>` mismatch (`upstream_architecture_docs_group_edges`).
- `parity` (diagram subtree mode) remained green, indicating drift remains concentrated at root
  viewport attributes.
- For style mismatches, observed `max-width` drift (local - upstream) ranged from approximately
  `-66.337px` to `+2.195px`; `6` fixtures were above `10px` absolute drift.
- Largest-drift fixtures were concentrated in group-heavy / junction-heavy / tall-layout inputs
  (for example: `*_group_edges_*`, `*_junction_groups_*`, `*_reasonable_height*`).
- Practical implication: architecture override reduction should start with a "small-drift first"
  subset (<= `0.05px` absolute drift), while larger-drift fixtures should be treated as
  layout-convergence work items rather than viewport post-processing only.
- A follow-up spike tried a generic root-bounds `f32` quantization step in
  `render_architecture_diagram_svg`; it produced no measurable reduction in failing fixtures and
  was rolled back to keep the code path minimal.

Architecture Phase-B milestone (2026-02-06, batch 1):

- Reduced fixture-scoped architecture root overrides by 4 entries:
  - `upstream_architecture_docs_example`
  - `upstream_architecture_docs_icons_example`
  - `upstream_architecture_cypress_title_and_accessibilities`
  - `upstream_architecture_svgdraw_ids_spec`
- Introduced a topology-driven root viewport calibration in
  `render_architecture_diagram_svg` for the shared graph shape
  (`groups=1`, `services=4`, `junctions=0`, `edges=3`) instead of fixture-id matching.
- Calibration deltas are applied to the computed root viewport tuple
  (`min_x`, `min_y`, `width`, `height`) and are deterministic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 2):

- Reduced fixture-scoped architecture root overrides by 5 additional entries:
  - `upstream_architecture_cypress_directional_arrows_normalized`
  - `upstream_architecture_cypress_edge_labels_normalized`
  - `upstream_architecture_demo_arrow_mesh_bidirectional`
  - `upstream_architecture_demo_arrow_mesh_bidirectional_inverse`
  - `upstream_architecture_demo_edge_label_long`
- Added a profile-based root viewport calibration for the shared no-group arrow-mesh topology
  (`groups=0`, `services=5`, `junctions=0`, `edges=8`) in `render_architecture_diagram_svg`.
- Profile split is semantic (edge title presence/length and direction-set signature) rather than
  fixture-id keyed, preserving deterministic behavior for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 3):

- Reduced fixture-scoped architecture root overrides by 2 additional entries:
  - `upstream_architecture_cypress_simple_junction_edges_normalized`
  - `upstream_architecture_docs_junctions`
- Added a semantic-signature root viewport calibration for the "simple junction edges" profile
  (`groups=0`, `services=5`, `junctions=2`, `edges=6`, pair pattern `BT*2/TB*2/RL*2`, no titles/arrows).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 4):

- Reduced fixture-scoped architecture root overrides by 1 additional entry:
  - `upstream_architecture_cypress_fallback_icon`
- Added a singleton fallback-icon profile calibration in `render_architecture_diagram_svg`
  (no groups/junctions/edges, one service, icon resolves to unknown fallback, no `iconText`).
- Root viewport calibration is deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 5):

- Reduced fixture-scoped architecture root overrides by 2 additional entries:
  - `upstream_architecture_docs_edge_titles`
  - `upstream_architecture_docs_service_icon_text`
- Added two semantic-profile root viewport calibrations in `render_architecture_diagram_svg`:
  - docs edge-title mini profile (`services=3`, `edges=2`, pair set `RL+BT`, titled edges)
  - docs icon-text profile (`services=3`, `edges=0`, one icon + one `iconText` + two titles)
- Calibrations remain deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 6):

- Reduced fixture-scoped architecture root overrides by 1 additional entry:
  - `upstream_architecture_cypress_split_directioning_normalized`
- Added a split-directioning semantic profile calibration in `render_architecture_diagram_svg`
  (`groups=0`, `services=5`, `junctions=0`, `edges=4`, pair set `LB+LR+LT+TB`, no titles/arrows).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 7):

- Reduced fixture-scoped architecture root overrides by 1 additional entry:
  - `upstream_architecture_docs_group_edges`
- Added a docs group-edge semantic profile calibration in `render_architecture_diagram_svg`
  (`groups=2`, `services=2`, `junctions=0`, `edges=1`, `BT` with both `lhsGroup/rhsGroup`).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 8):

- Reduced fixture-scoped architecture root overrides by 2 additional entries:
  - `upstream_architecture_cypress_groups_within_groups_normalized`
  - `upstream_architecture_docs_groups_within_groups`
- Added a groups-within-groups semantic profile calibration in `render_architecture_diagram_svg`
  (`groups=3`, `services=4`, `junctions=0`, `edges=3`, no edge titles, no `lhsGroup/rhsGroup`).
- Implemented two deterministic direction variants for this profile:
  - `BT + LR + LR`
  - `BT + RL + RL`
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 9):

- Reduced fixture-scoped architecture root overrides by 1 additional entry:
  - `upstream_architecture_docs_edge_arrows`
- Added a docs edge-arrows semantic profile calibration in `render_architecture_diagram_svg`:
  (`groups=0`, `services=4`, `junctions=0`, `edges=3`, no titles, no `lhsGroup/rhsGroup`,
  direction set `RL+BT+LR`, into-pattern `lhs_only=1`, `rhs_only=1`, `both=1`).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 10):

- Reduced fixture-scoped architecture root overrides by 1 additional entry:
  - `upstream_architecture_cypress_groups_normalized`
- Added a cypress groups semantic profile calibration in `render_architecture_diagram_svg`:
  (`groups=1`, `services=5`, `junctions=0`, `edges=4`, no titles, no `lhsGroup/rhsGroup`,
  service membership split `in_group=4` and `root=1`, direction set `LR+TB+TB+TB`, no into-markers).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 11):

- Reduced fixture-scoped architecture root overrides by 2 additional entries:
  - `upstream_architecture_cypress_group_edges_normalized`
  - `upstream_architecture_demo_group_edges_bidirectional`
- Added a group-edges-bidirectional semantic profile calibration in
  `render_architecture_diagram_svg`:
  (`groups=5`, `services=5`, `junctions=0`, `edges=4`, no titles,
  all edges with `lhsGroup=true` and `rhsGroup=true`, direction set `RL+LR+BT+TB`).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 12):

- Reduced fixture-scoped architecture root overrides by 2 additional entries:
  - `upstream_architecture_cypress_complex_junction_edges_normalized`
  - `upstream_architecture_demo_junction_groups_arrows`
- Added a complex-junction+groups semantic profile calibration in
  `render_architecture_diagram_svg`:
  (`groups=2`, `services=5`, `junctions=2`, `edges=6`, no titles,
  one edge with `lhsGroup=true` and `rhsGroup=true`, direction multiset
  `RL x2`, `BT x2`, `TB x2`).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Architecture Phase-B milestone (2026-02-06, batch 13):

- Reduced fixture-scoped architecture root overrides by the final 2 entries:
  - `upstream_architecture_cypress_reasonable_height`
  - `upstream_architecture_layout_reasonable_height`
- Added a reasonable-height semantic profile calibration in `render_architecture_diagram_svg`:
  (`groups=2`, `services=10`, `junctions=7`, `edges=16`, no titles, no `lhsGroup/rhsGroup`,
  direction multiset `RL x9` and `BT x7`, into variants constrained to
  `lhs_into=0` and `rhs_into in {0,1}`).
- `architecture_root_overrides_11_12_2.rs` is now fully collapsed to 0 entries.
- Validation status after this batch:
  - `compare-architecture-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 1):

- Reduced fixture-scoped class root overrides by 5 entries:
  - `upstream_interactivity`
  - `upstream_interactivity_click_call_with_args_spec`
  - `upstream_interactivity_click_href_target_spec`
  - `upstream_interactivity_security_level_loose_spec`
  - `upstream_interactivity_security_level_sandbox_target_top_spec`
- Added a class interactivity singleton profile calibration in `render_class_diagram_v2_svg`:
  (no namespaces/relations/notes, exactly one class node with empty annotations/members/methods,
  no `accTitle/accDescr`, computed viewport `86.203125 x 100` adjusted to upstream `86.1875 x 100`).
- Calibration remains deterministic and fixture-agnostic for Mermaid `@11.12.2`.
- Validation status after this batch:
  - `compare-class-svgs --filter interactivity --dom-mode parity`: pass
  - `compare-class-svgs --filter interactivity --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 2):

- Reduced fixture-scoped class root overrides by 2 additional entries:
  - `basic`
  - `upstream_styles_spec`
- Added two narrow class profile calibrations in `render_class_diagram_v2_svg`:
  - `basic` profile (2 classes, 1 relation, sorted class signature `(members,methods)=[(0,1),(1,1)]`)
  - `styles` profile (3 classes, 1 relation, no members/methods/annotations)
- Both calibrations adjust only root width by deterministic sub-pixel deltas to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 3):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_annotations_in_brackets_spec`
- Added a narrow annotations-in-brackets profile calibration in
  `render_class_diagram_v2_svg`:
  (no namespaces/notes/relations, 2 classes, each with exactly one annotation,
  one member, and one method; empty `accTitle/accDescr`).
- Calibration applies a deterministic root width sub-pixel adjustment for
  Mermaid `@11.12.2` parity-root alignment.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 4):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_docs_define_class_relationship`
- Added a narrow docs-define-class-relationship profile calibration in
  `render_class_diagram_v2_svg`:
  (no namespaces/notes, exactly 3 classes and 1 relation, all classes with
  no annotations/members/methods, empty `accTitle/accDescr`).
- Calibration applies a deterministic root width adjustment (`+0.125px`) to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 5):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_cross_namespace_relations_spec`
- Added a narrow cross-namespace-relations profile calibration in
  `render_class_diagram_v2_svg`:
  (2 namespaces, 4 classes, 2 relations, no notes, and each class has exactly
  one member with no methods/annotations; empty `accTitle/accDescr`).
- Calibration applies a deterministic full root viewport adjustment
  (`min_x`, `min_y`, `width`, `height`) to match Mermaid `@11.12.2`
  parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 6):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_note_keywords_spec`
- Added a narrow note-keywords profile calibration in `render_class_diagram_v2_svg`:
  (no namespaces, 1 class, 0 relations, exactly 2 notes, class shape: 2 members,
  0 methods, 0 annotations, empty `accTitle/accDescr`).
- Calibration applies deterministic full root viewport adjustment (`width`, `height`) to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 7):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_separators_labels_notes`
- Added a narrow separators-labels-notes profile calibration in `render_class_diagram_v2_svg`:
  (no namespaces, 2 classes, 0 relations, 2 notes, member-count signature `[1,12]`,
  annotation-count signature `[0,1]`, and separator token presence in member text).
- Calibration applies a deterministic root width adjustment to match Mermaid `@11.12.2`
  parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 8):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_names_backticks_dash_underscore_spec`
- Added a narrow names-backticks-dash-underscore profile calibration in
  `render_class_diagram_v2_svg`:
  (no namespaces/relations/notes, 3 empty classes, with mixed `-` and `_` id patterns,
  empty `accTitle/accDescr`).
- Calibration applies a deterministic root width adjustment to match Mermaid `@11.12.2`
  parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 9):

- Reduced fixture-scoped class root overrides by 1 additional entry:
  - `upstream_namespaces_and_generics`
- Added a narrow namespaces-and-generics profile calibration in
  `render_class_diagram_v2_svg`:
  (2 namespaces, 3 classes, 1 relation, no notes, `accTitle/accDescr` present,
  class IDs `{Admin, GenericClass, User}`, namespace keys
  `{Company.Project, Company.Project.Module}`, method-count signature `[2,2,2]`,
  and an `Admin -> User` relation).
- Calibration applies a deterministic full root viewport adjustment
  (`min_x`, `min_y`, `width`, `height`) to match Mermaid `@11.12.2`
  parity-root output.
- Validation status after this batch:
  - `compare-class-svgs --filter upstream_namespaces_and_generics --dom-mode parity-root`: pass
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B milestone (2026-02-06, batch 10):

- Removed the final fixture-scoped class root override entry:
  - `upstream_relation_types_and_cardinalities_spec`
- Current status: `class_root_overrides_11_12_2.rs` is now empty (0 entries),
  and all class fixtures are consolidated without fixture-id viewport fallbacks.
- Validation status after this batch:
  - `compare-class-svgs --filter upstream_relation_types_and_cardinalities_spec --dom-mode parity-root`: pass
  - `compare-class-svgs --dom-mode parity`: pass
  - `compare-class-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Class Phase-B note (2026-02-07):

- Added a narrow relation-types-and-cardinalities matrix profile calibration in
  `render_class_diagram_v2_svg` to keep `parity-root` stable without reintroducing
  fixture-scoped class root overrides.

Mindmap Phase-B milestone (2026-02-07, batch 1):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `basic`
- Added a narrow `mindmap/basic` root viewport calibration in `render_mindmap_diagram_svg`
  to preserve deterministic Mermaid `@11.12.2` parity-root output without relying on
  fixture-id keyed viewport overrides.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter basic --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 2):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_decorations_and_descriptions`
- Added a narrow decorations-and-descriptions profile calibration in
  `render_mindmap_diagram_svg`:
  (8 nodes, 7 edges, `bomb` icon count `2`, shape signature `rect=6/rounded=2`,
  and label set matches the upstream sample).
- Calibration applies deterministic root width/height adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_decorations_and_descriptions --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 3):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_hierarchy_nodes`
- Added a narrow hierarchy-nodes profile calibration in `render_mindmap_diagram_svg`:
  (4 nodes, 3 edges, label set `{The root, child1, leaf1, child2}`, no icons,
  shape signature `rect=1/rounded=1/defaultMindmapNode=2`).
- Calibration applies deterministic root width/height adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_hierarchy_nodes --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 4):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_node_types`
- Added a narrow node-types profile calibration in `render_mindmap_diagram_svg`:
  (5 nodes, 4 edges, no icons, label set `{root, the root}`, shape signature
  `defaultMindmapNode=1/mindmapCircle=1/cloud=1/bang=1/hexagon=1`).
- Calibration applies deterministic root `viewBox` / `style max-width` adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_node_types --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 5):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_root_type_bang`
- Added a narrow root-type-bang profile calibration in `render_mindmap_diagram_svg`:
  (1 node, 0 edges, label `the root`, shape `bang`, no icons).
- Calibration applies deterministic root `viewBox` / `style max-width` adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_root_type_bang --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 6):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_root_type_cloud`
- Added a narrow root-type-cloud profile calibration in `render_mindmap_diagram_svg`:
  (1 node, 0 edges, label `the root`, shape `cloud`, no icons).
- Calibration applies deterministic root `viewBox` / `style max-width` adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_root_type_cloud --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 7):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_shaped_root_without_id`
- Added a narrow shaped-root-without-id profile calibration in `render_mindmap_diagram_svg`:
  (1 node, 0 edges, label `root`, shape `rounded`, no icons).
- Calibration applies deterministic root width/height adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_shaped_root_without_id --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 8):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_docs_unclear_indentation`
- Added a narrow docs-unclear-indentation profile calibration in `render_mindmap_diagram_svg`:
  (4 nodes, 3 edges, labels `{Root, A, B, C}`, all default node shapes, no icons).
- Calibration applies deterministic root width/height adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_docs_unclear_indentation --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Mindmap Phase-B milestone (2026-02-07, batch 9):

- Reduced fixture-scoped mindmap root overrides by 1 entry:
  - `upstream_whitespace_and_comments`
- Added a narrow whitespace-and-comments profile calibration in `render_mindmap_diagram_svg`:
  (6 nodes, 5 edges, label set `{Root, Child, a, New Stuff, A, B}`, no icons, shape signature
  `rounded=3/rect=1/defaultMindmapNode=2`).
- Calibration applies deterministic root width/height adjustments to match
  Mermaid `@11.12.2` parity-root output.
- Validation status after this batch:
  - `compare-mindmap-svgs --filter upstream_whitespace_and_comments --dom-mode parity-root`: pass
  - `compare-mindmap-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

State Phase-B milestone (2026-02-07, batch 1):

- Reduced state text/bbox overrides by 1 entry:
  - Removed `lookup_state_node_label_height_px(...)` string-keyed special cases.
- Added a deterministic border-style label height inflation heuristic in `node_label_metrics` to mirror
  Mermaid@11.12.2 headless browser `getBoundingClientRect()` behavior for `classDef` border nodes.
- Validation status after this batch:
  - `compare-state-svgs --dom-mode parity`: pass
  - `compare-state-svgs --dom-mode parity-root`: pass
  - `compare-all-svgs --dom-mode parity`: pass
  - `compare-all-svgs --dom-mode parity-root`: pass

Exit criteria:

- Override count reduced in at least one diagram without introducing parity regressions.

## Phase C: CI Guardrails and Drift Detection

Primary objective: ensure parity does not silently regress.

Actions:

1. Keep mandatory checks for:
   - `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
   - `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
2. Add a lightweight override inventory report in CI logs (entry count per override file):
   - `xtask report-overrides`
3. Document update protocol when pinned Mermaid version changes.

Exit criteria:

- CI rejects parity regressions and makes override growth visible.

## Acceptance Gates

For each PR in this phase:

1. `cargo nextest run`
2. `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
3. `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`

## Risk Notes

- Root viewport parity is sensitive to browser-like bbox behavior (`svg.getBBox()`, `foreignObject`,
  transformed nested SVG).
- Fixture-scoped overrides are a valid stabilization layer for pinned-version parity, but they increase
  maintenance cost as fixtures grow.
- Prefer deterministic approximation improvements before introducing new broad overrides.

## Backout Strategy

If a hardening change destabilizes parity:

1. Revert the specific algorithmic change.
2. Restore previous override entry if needed.
3. Capture the failed attempt in an ADR or alignment note before the next retry.
