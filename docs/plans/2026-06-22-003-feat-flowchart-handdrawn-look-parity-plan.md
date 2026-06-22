---
title: "feat: Flowchart hand-drawn look parity"
type: feat
date: 2026-06-22
---

# feat: Flowchart hand-drawn look parity

## Summary

Promote Flowchart `look: "handDrawn"` from partial DOM/config support to a source-backed, testable support claim. The default renderer behavior should remain `classic`; `handDrawn` stays opt-in through Mermaid-compatible config and frontmatter.

This plan focuses on Flowchart because it is the diagram family where Mermaid ships the largest hand-drawn fixture suite and where `merman` already has RoughJS path infrastructure.

---

## Problem Frame

`look` is less common than layout selection, but it is a public Mermaid root config with visible Playground and documentation value. Mermaid's Flowchart Cypress suite exercises `look: "handDrawn"` across simple graphs, shape variants, class/style overrides, subgraphs, link handling, `htmlLabels`, `useMaxWidth`, `handDrawnSeed`, and edge animation.

Local support is already more than a pass-through: Flowchart nodes read `handDrawnSeed`, several node shape modules emit rough paths, and focused seed tests prove deterministic output for one Flowchart shape. The support claim is still too broad for users because edge, cluster, fixture admission, and shape-matrix coverage are not audited as a single Flowchart contract.

---

## Requirements

### Config And Support Contract

- R1. Flowchart must accept and consume root `look: "handDrawn"` and root `handDrawnSeed` with Mermaid-compatible precedence.
- R2. `look: "handDrawn"` must remain opt-in; `classic` remains the default for existing render entry points.
- R3. The support matrix must describe Flowchart hand-drawn behavior as a family-specific rendered contract, not as universal `look` support.

### Rendering Behavior

- R4. Hand-drawn Flowchart node shapes must use source-backed RoughJS-compatible visible paths where Mermaid's Flowchart shape helpers do.
- R5. `handDrawnSeed` must make newly covered rough node output deterministic for the same seed and visibly different for different seeds.
- R6. Edges and subgraph clusters must follow upstream source behavior instead of gaining synthetic rough styling; if Mermaid keeps them as ordinary paths/rects, local tests should lock that.
- R7. Style, class, stroke, fill, dash, label color, `htmlLabels`, link, and edge-animation behavior must continue to work under `look: "handDrawn"`.

### Evidence And Admission

- R8. Imported cases from Mermaid's `flowchart-handDrawn.spec.js` must be audited into admitted, deferred residual, or unsupported buckets.
- R9. Broad SVG comparison must use a fixed `handDrawnSeed` for deterministic fixture baselines.
- R10. Browser-dependent residuals such as text measurement, `getBBox()` floats, font rendering, and exact RoughJS path noise must be documented rather than hidden behind broad normalization.

---

## High-Level Technical Design

| Surface | Mermaid source signal | Local target |
| --- | --- | --- |
| Root config | `config.type.ts` exposes `look` and `handDrawnSeed`. | Preserve root config merge and render consumption. |
| Flowchart nodes | `rendering-elements/shapes/*` uses `rough.svg(...)` and `userNodeOverrides(...)` for hand-drawn shapes. | Route every supported Flowchart node shape through the existing rough helper or document the source-backed exception. |
| Node styles | `handDrawnShapeStyles.ts` maps style/class fill, stroke, stroke width, dash arrays, and seed into RoughJS options. | Keep local style compilation aligned with rough path attributes and label style handling. |
| Edges | `dagre-wrapper/edges.js` emits normal SVG paths and markers without a hand-drawn branch. | Do not invent rough edges; lock style, markers, labels, and animation under hand-drawn config. |
| Clusters | `dagre-wrapper/clusters.js` emits normal cluster rects and labels without a hand-drawn branch. | Do not invent rough subgraph boxes; lock class/style/title behavior under hand-drawn config. |
| Fixtures | `flowchart-handDrawn.spec.js` has 49 render cases. | Admit stable headless-compatible cases and document residuals. |

---

## Key Technical Decisions

- KTD1. Flowchart first, not universal `look`: Flowchart has the strongest upstream fixture signal and existing local RoughJS infrastructure. Other diagram families keep their current partial support claims until they have focused rendered evidence.
- KTD2. Source-backed shape parity over visual imitation: rough output should follow Mermaid shape helper semantics and local `roughr-merman` primitives, not hand-authored jitter or broad pixel tuning.
- KTD3. Edges and clusters are characterization targets: current Mermaid dagre Flowchart code does not roughen edge paths or cluster rects, so local work should preserve that unless source audit finds a different renderer path.
- KTD4. Seed determinism is part of the public contract: every new rough path family covered by tests should use root `handDrawnSeed`, and comparison fixtures should keep using a fixed seed.
- KTD5. Fixture admission is evidence, not decoration: upstream hand-drawn fixtures only become part of the support claim after they render without errors and pass the existing DOM/SVG parity gate with narrow residual handling.
- KTD6. Resource limits still apply: hand-drawn rendering increases path generation work and SVG size, so large fixture admission should reuse the existing render resource budget posture rather than bypassing it for visual mode.

---

## Scope Boundaries

### In Scope

- Flowchart `look: "handDrawn"` rendered SVG behavior for the dagre-backed Flowchart path.
- Flowchart node shape, style, seed, label, subgraph, edge-label, link, and animation coverage reachable from Mermaid's public Flowchart tests.
- Documentation updates that clarify what support is claimed and what remains residual.

### Deferred To Follow-Up Work

- Universal hand-drawn parity for every Mermaid diagram family.
- Pixel-perfect RoughJS output against Chromium screenshots.
- Venn, Ishikawa, Wardley, or other family-specific hand-drawn renderers.
- Any ELK-specific hand-drawn visual parity unless a concrete Mermaid fixture combines Flowchart ELK and `look: "handDrawn"`.

---

## System-Wide Impact

This work affects SVG output, comparison baselines, and config support claims. It should not change parser semantics, layout family detection, default rendering mode, public ABI, or default dependency activation.

The main operational impact is cost: rough path generation can increase CPU time and SVG size. The implementation should verify that hand-drawn fixture admission remains inside the same resource-limit envelope used for normal headless rendering.

---

## Implementation Units

### U1. Audit Mermaid hand-drawn surfaces

- **Goal:** Build a precise source-backed map of what Flowchart hand-drawn mode changes.
- **Requirements:** R1, R3, R4, R6, R8.
- **Dependencies:** None.
- **Files:** `docs/alignment/FLOWCHART_UPSTREAM_TEST_COVERAGE.md`, `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`, `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-handDrawn.spec.js`, `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/handDrawnShapeStyles.ts`, `repo-ref/mermaid/packages/mermaid/src/dagre-wrapper/edges.js`, `repo-ref/mermaid/packages/mermaid/src/dagre-wrapper/clusters.js`.
- **Approach:** Record the upstream hand-drawn fixture inventory and the source surfaces that actually branch on `look`. Treat `repo-ref` as read-only reference material.
- **Patterns to follow:** Existing ELK coverage notes in `docs/alignment/FLOWCHART_UPSTREAM_TEST_COVERAGE.md`.
- **Test scenarios:** Test expectation: none -- this unit produces documentation and audit evidence.
- **Verification:** The audit identifies every `flowchart-handDrawn.spec.js` render case and separates node rough behavior from unchanged edge/cluster behavior.

### U2. Add characterization tests for the support contract

- **Goal:** Lock the visible contract before broad refactoring.
- **Requirements:** R1, R2, R5, R6, R7.
- **Dependencies:** U1.
- **Files:** `crates/merman-render/tests/hand_drawn_seed_svg_test.rs`, `crates/merman-render/tests/look_svg_test.rs`, `crates/merman-render/tests/flowchart_svg_test.rs`.
- **Approach:** Add focused tests for opt-in `look: "handDrawn"`, same-seed determinism, different-seed rough deltas, cluster non-rough characterization, edge non-rough characterization, and style/class propagation under hand-drawn mode.
- **Execution note:** Start characterization-first so later renderer changes cannot blur current behavior and upstream evidence.
- **Patterns to follow:** `flowchart_svg_hand_drawn_seed_controls_visible_rough_paths` and `flowchart_svg_uses_configured_look_for_subgraph_clusters`.
- **Test scenarios:** Render a simple hand-drawn Flowchart and assert `data-look="handDrawn"` reaches node and cluster DOM without leaking `classic`.
- **Test scenarios:** Render the same rough-node fixture twice with `handDrawnSeed: 7` and assert identical SVG.
- **Test scenarios:** Render the same fixture with seeds `7` and `8` and assert visible rough path differences.
- **Test scenarios:** Render a subgraph fixture under hand-drawn mode and assert the cluster remains an ordinary rect while style/class/title output still applies.
- **Test scenarios:** Render an edge style and animation fixture under hand-drawn mode and assert edge path classes, markers, labels, and animation classes remain correct.
- **Verification:** Focused SVG tests fail if hand-drawn mode is merely accepted but not visibly consumed.

### U3. Close node shape rough-path gaps

- **Goal:** Make every Mermaid-reachable Flowchart node shape behave correctly under hand-drawn mode.
- **Requirements:** R4, R5, R7.
- **Dependencies:** U1, U2.
- **Files:** `crates/merman-render/src/svg/parity/flowchart/render/node.rs`, `crates/merman-render/src/svg/parity/flowchart/render/node/roughjs.rs`, `crates/merman-render/src/svg/parity/flowchart/render/node/shapes/*.rs`, `crates/merman-render/tests/hand_drawn_seed_svg_test.rs`, `crates/merman-render/tests/flowchart_svg_test.rs`.
- **Approach:** Compare the local shape matrix against Mermaid's shape helpers. Reuse existing rough helpers for rectangles, polygons, circles, and SVG paths; add helper variants only when a Mermaid call pattern cannot be represented cleanly.
- **Patterns to follow:** Existing rough implementations in `hexagon.rs`, `rounded_rect.rs`, `stadium.rs`, `tag_rect.rs`, `wave_document.rs`, and `window_pane.rs`.
- **Test scenarios:** Render the upstream shape styling matrix under `look: "handDrawn"` and assert representative rough path groups exist for rectangle, rounded rectangle, stadium, hexagon, cylinder, circle, subroutine, and trapezoid shapes.
- **Test scenarios:** Render style-heavy shapes and assert fill, stroke, stroke width, and dash arrays reach the rough fill/stroke paths.
- **Test scenarios:** Render icon/image shapes under hand-drawn mode and assert labels and asset wrappers are still stable.
- **Test scenarios:** Render no-label start/stop/circle-like shapes and assert seed determinism still holds.
- **Verification:** Mermaid-reachable Flowchart shape families either have hand-drawn rough output or a documented source-backed reason for not doing so.

### U4. Preserve edge and cluster semantics under hand-drawn mode

- **Goal:** Ensure hand-drawn mode does not regress non-node Flowchart behavior.
- **Requirements:** R6, R7.
- **Dependencies:** U1, U2.
- **Files:** `crates/merman-render/src/svg/parity/flowchart/render/edge_path.rs`, `crates/merman-render/src/svg/parity/flowchart/render/cluster.rs`, `crates/merman-render/tests/flowchart_svg_test.rs`.
- **Approach:** Keep edge paths and cluster rectangles aligned with Mermaid dagre source unless new source evidence proves they should be rough. Tighten tests around class/style, labels, markers, click/link behavior, and edge animation.
- **Patterns to follow:** Existing edge marker and subgraph style assertions in Flowchart SVG tests.
- **Test scenarios:** Render styled subgraphs from Mermaid hand-drawn fixtures and assert fill/stroke/title color behavior survives.
- **Test scenarios:** Render multi-edge and minimum-edge-length fixtures under hand-drawn config and assert paths remain renderable with correct markers and labels.
- **Test scenarios:** Render `L_A_B_0@{ animation: slow }` and `L_B_D_0@{ animation: fast }` under hand-drawn config and assert the animation classes appear on the expected edge paths.
- **Verification:** Non-node surfaces are intentionally characterized rather than accidentally roughened.

### U5. Admit upstream hand-drawn fixtures

- **Goal:** Convert Mermaid hand-drawn fixture coverage into a maintained parity gate.
- **Requirements:** R8, R9, R10.
- **Dependencies:** U2, U3, U4.
- **Files:** `fixtures/flowchart/*.mmd`, `fixtures/upstream-svgs/flowchart/*.svg`, `crates/xtask/src/cmd/compare/diagrams/flowchart.rs`, `docs/alignment/FLOWCHART_UPSTREAM_TEST_COVERAGE.md`.
- **Approach:** Audit all 49 upstream hand-drawn cases, import missing headless-compatible fixtures, regenerate fixed-seed upstream baselines, and admit stable cases to the Flowchart SVG compare lane.
- **Patterns to follow:** Existing Flowchart upstream fixture naming and ELK exact-call coverage notes.
- **Test scenarios:** Every admitted fixture renders locally without render error.
- **Test scenarios:** Every admitted fixture has an upstream SVG baseline generated with fixed `handDrawnSeed`.
- **Test scenarios:** The Flowchart compare command passes with narrow normalization and no hand-drawn-specific broad masking.
- **Verification:** The coverage document states admitted, deferred, and residual hand-drawn cases with fixture paths.

### U6. Update support claims and resource-risk evidence

- **Goal:** Make the public support statement match the implementation evidence.
- **Requirements:** R3, R9, R10.
- **Dependencies:** U5.
- **Files:** `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`, `docs/alignment/FLOWCHART_UPSTREAM_TEST_COVERAGE.md`, `docs/alignment/STATUS.md`, `crates/merman-render/tests/hand_drawn_seed_svg_test.rs`.
- **Approach:** Upgrade Flowchart `look: "handDrawn"` from partial evidence to a Flowchart-specific rendered support claim only after tests and fixture admission pass. Mention CPU/SVG-size risk and the existing render resource limit posture.
- **Patterns to follow:** The wording used for Flowchart ELK public config coverage in `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md`.
- **Test scenarios:** Render a larger shape-matrix fixture under `look: "handDrawn"` and assert it completes through the same render entry point as normal Flowchart SVG tests.
- **Verification:** Documentation no longer overclaims universal `look` support and gives users a clear Flowchart hand-drawn support statement.

---

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| RoughJS path parity drifts from Mermaid | Users see different hand-drawn shapes from Mermaid | Port source call patterns and preserve focused shape tests. |
| Fixture comparison becomes noisy | Broad parity gates become flaky | Use fixed `handDrawnSeed` and document narrow residuals instead of broad normalization. |
| Edges or clusters get incorrectly roughened | Local output diverges from Mermaid despite looking more hand-drawn | Treat source audit as authoritative and characterize unchanged surfaces. |
| SVG size and CPU cost increase | Large hand-drawn diagrams can stress render budgets | Keep `handDrawn` opt-in and reuse existing render resource-limit checks. |
| Support claim expands across families accidentally | Users expect unsupported diagrams to match Mermaid hand-drawn output | Keep the docs family-specific until each renderer has focused evidence. |

---

## Acceptance Examples

- AE1. Given a Flowchart with `%%{init: {"look": "handDrawn", "handDrawnSeed": 1}}%%`, when it renders twice, then the resulting SVG is deterministic for rough node paths.
- AE2. Given the same Flowchart rendered with `handDrawnSeed: 1` and `handDrawnSeed: 2`, when it includes a rough-supported node shape, then visible rough path data changes.
- AE3. Given a styled subgraph rendered with `look: "handDrawn"`, when the SVG is inspected, then subgraph style and title behavior match Mermaid's ordinary cluster path instead of a fabricated rough cluster.
- AE4. Given a Flowchart edge animation fixture rendered with `look: "handDrawn"`, when the SVG is inspected, then the expected edge animation classes remain attached to the edge paths.
- AE5. Given an imported `flowchart-handDrawn.spec.js` fixture admitted to the matrix, when the Flowchart SVG comparison gate runs with fixed seed, then it passes without broad hand-drawn-only normalization.

---

## Sources And Research

- `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-handDrawn.spec.js` has the upstream Flowchart hand-drawn render cases.
- `repo-ref/mermaid/packages/mermaid/src/config.type.ts` defines root `look` and `handDrawnSeed`.
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/handDrawnShapeStyles.ts` defines Mermaid's rough node style and seed mapping.
- `repo-ref/mermaid/packages/mermaid/src/rendering-util/rendering-elements/shapes/drawRect.ts` shows hand-drawn node rough rendering.
- `repo-ref/mermaid/packages/mermaid/src/dagre-wrapper/edges.js` shows Flowchart edge path emission without hand-drawn roughing.
- `repo-ref/mermaid/packages/mermaid/src/dagre-wrapper/clusters.js` shows Flowchart cluster rect emission without hand-drawn roughing.
- `crates/merman-render/src/svg/parity/flowchart/render/node/roughjs.rs` contains existing local rough path helpers.
- `docs/alignment/CONFIG_FRONTMATTER_SUPPORT.md` currently marks `look` and `handDrawnSeed` as partial support.
- `docs/alignment/FLOWCHART_UPSTREAM_TEST_COVERAGE.md` tracks imported Flowchart upstream fixtures and the Flowchart ELK coverage model to mirror for hand-drawn admission.
