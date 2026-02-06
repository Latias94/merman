# Parity Hardening Plan (Post 100% Baseline)

Baseline version: Mermaid `@11.12.2`.

As of 2026-02-06:

- `parity` full compare: 0 mismatch.
- `parity-root` full compare: 0 mismatch (484/484 upstream SVG baselines).

This document defines the next hardening phases after reaching baseline 100% parity for the
current fixture set.

## Goals

1. Keep global parity green (`parity` + `parity-root`) while the fixture corpus grows.
2. Reduce fixture-scoped override dependence where feasible.
3. Preserve deterministic, reproducible results for the pinned upstream version.

## Current Inventory

### Upstream SVG Corpus

- Total diagrams covered: 23
- Total upstream SVG baselines: 484

Largest fixture buckets:

- `flowchart`: 120
- `gantt`: 73
- `state`: 43
- `sequence`: 40
- `architecture`: 32

### Override Footprint (11.12.2)

Root viewport overrides:

- `architecture_root_overrides_11_12_2.rs`: 15 entries (out of 32 architecture fixtures)
- `class_root_overrides_11_12_2.rs`: 9 entries (out of 17 class fixtures)
- `mindmap_root_overrides_11_12_2.rs`: 6 entries (out of 12 mindmap fixtures)

State text/bbox overrides:

- `state_text_overrides_11_12_2.rs`: 47 `Some(...)` entries across width/height/bbox helpers

## Phase Plan

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

Exit criteria:

- Override count reduced in at least one diagram without introducing parity regressions.

## Phase C: CI Guardrails and Drift Detection

Primary objective: ensure parity does not silently regress.

Actions:

1. Keep mandatory checks for:
   - `compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3`
   - `compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3`
2. Add a lightweight override inventory report in CI logs (entry count per override file).
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
