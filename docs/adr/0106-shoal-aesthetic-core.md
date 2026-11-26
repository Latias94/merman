# ADR 0106: Shoal Aesthetic Core and Layout Architecture

- Status: accepted for the main development line on 2026-11-26
- Relates to: [ADR 0099](0099-model-and-source-boundaries.md), [ADR 0102](0102-native-theme-v2.md)
- Does not change: Mermaid parity backends (`dagre` / `elk`), `#![forbid(unsafe_code)]`, zero-dependency purity of `merman-layout-shoal`, or bounded `WorkControl` guarantees

## Context

`merman-layout-shoal` has established strong engineering invariants: deterministic output, zero `unsafe` code, zero external crate dependencies, bounded cooperative work (`WorkControl`), hybrid adaptive cycle breaking, exterior feedback corridors, mental-map stability via weighted PAVA, and DDLT zero-overlap validation.

However, visual quality evolved primarily through post-hoc passes targeting individual witness fixtures rather than a single top-down aesthetic model. As a result:

1. `merman-layout-shoal` expanded past 36k lines with duplicated obstacle-clearance indices and unpooled temporary buffers in `refine`, `routing`, and `hub`.
2. Solver and painter operate under a fractured contract: unmarked endpoints report synthetic `LEAD` length during layout, `route_paint` speculatively strips solver guards via `expand_terminal_sweeps`, and shape contour insets (notably diamonds and ellipses) are invisible during label and curve placement.
3. Local heuristics conflict on basic flowcharts (for example, `inline_labels` left off on Flowchart splits twin branches into single-gap `Free` vs. multi-gap `SuperGap`/`GapBoundary` curves with mismatched launch tangents and heights, while `sweep_handle_fraction` overshoots `0.50` up to `0.56` and causes vertical control-point inversion).
4. Completed aesthetic capabilities remain partially unwired (`CurveTerminals::Smooth` only on Flowchart; `inline_labels` and `compact_ranks` only on C4; `bus_routing` unused), and `spacing.edge = 4.0` forces ad-hoc `.max(12.0)` floors throughout the solver.
5. High-fan curved graphs (`fan-in-500`, `fan-out-500`) regress behind Node.js Pele because `Curved` gaps exceeding `max_weighed_gap_pieces` fall into `greedy_tracks` (flattening radial curves onto a shared shelf and doubling cubics) and `route_paint` runs full BVH cubic-obstacle subdivision on mathematically clear inter-rank gaps.
6. Non-flowchart families (Architecture, Swimlane, C4) rely on layered Sugiyama placement plus post-projection patches instead of domain-native placement models.

## Decision

Refactor Shoal around a unified **Aesthetic Core** spanning `merman-layout-shoal`, `merman-render-core`, and `merman-render`, without introducing legacy v3/v4 aesthetic compatibility toggles.

### 1. Seven-Level Aesthetic Priority Stack

All layout and routing conflicts resolve strictly in top-down priority order (higher levels supersede lower levels; each rule belongs to one owning stage):

1. **P0 — Geometric Honesty**: zero overlaps, zero obstacle piercings, zero collinear adhesion, and unobstructed arrows and labels (enforced by DDLT).
2. **P1 — Structural Clarity**: minimum crossings, anchored cycle entry and straight spine preservation, and exterior feedback corridors for long back-edges.
3. **P2 — Minimal and Calm Curves**: straight lines over single S-curves over multi-cubic arches over orthogonal detours; every bend must have a geometric cause, with monotone non-inverting Bézier handles.
4. **P3 — Optical Balance and Sibling Uniformity**: centered single connections, symmetric forks/joins, uniform per-face turn start (`face_reach`), matched departure tangents and landing horizons for sibling branches on a shared face, and edge labels anchored at the optical midpoint of the visible shaft (accounting for contour inset and marker depth).
5. **P4 — Typographic and Spatial Rhythm**: proportional spacing across node, rank, and edge separation; compound titles yield to straight trunks by choosing crossing-free positions rather than bending edges or inflating boxes.
6. **P5 — Intent and Mental-Map Continuity**: preserve declaration order and mirrored orientation when crossing-neutral; stabilize unaffected coordinates during interactive edits.
7. **P6 — Restrained Compactness**: inline short edge labels, compact uneven ranks, and aspect-balanced component packing without degrading P0–P4 breathing room.

### 2. Single Geometric Source of Truth

Eliminate solver-painter guesswork across crate boundaries:

- **Lightweight Contour Hints**: extend `NodeSpec` with an optional zero-dependency analytical contour hint (`ShapeContourHint`: rectangle, rounded box, stadium, diamond aspect/slope, ellipse) so the solver accounts for real port inset and visible shaft length during routing and label placement.
- **Marker Footprint and Turn Distance**. Unmarked endpoints have zero marker footprint and zero
  marker turn distance. Render adapters pass the painter's contact-to-turn distance alongside the
  footprint. A direct layout caller can specify these distances with `EdgeSpec::set_marker_runways`.
  The solver sizes a shared face against its longest runway and emits the turn-start points.
- **Non-Inverting Curve Tension**: replace `sweep_handle_fraction` with a monotone function bounded in `[0.25, 0.48]` (stable near `0.46–0.48` on ordinary spans, relaxing toward `0.25` on wide fan-outs; never exceeding `0.50`).
- **Sibling Departure Symmetry**: remove the `KnotKind::Free` vs. `GapBoundary` tangent split for shared-face sibling branches, and prevent `super_gap` from stretching one twin branch past a sibling's landing horizon when both leave the same face into adjacent corridors.
- **Corridor Safety Proof**. Record a proved clearance radius in `corridor_clearance`, in drawing
  units. The canonical painter can use it for a curved edge when the stroke outset is known and
  smaller than the radius, the box and contour contacts coincide, and the edge has no stitches.

### 3. Single Evolving Native Default and Print-Grade Polish

Shoal evolves as one coherent default style; Mermaid parity layouts remain separate.

- **Proportional Spacing Scale**: recalibrate Shoal native defaults to a coherent ratio (such as `node: 44, rank: 48, edge: 14`, replacing `edge = 4.0`) and remove scattered `.max(12.0)` self-rescue clamps.
- **Universal Feature Activation**: enable `CurveTerminals::Smooth`, `inline_labels`, and `compact_ranks` by default across all directed Shoal families (Flowchart, Class, State, ER, Requirement, Usecase, C4), removing the hardcoded `CurveTerminals::Fixed` in shared `CanonicalRoutes`.
- **Decision Diamond Geometry**: adopt a wide proportional diamond (`~1.6:1`) in native Theme v2 layouts without asymmetric `+0.5 px` polygon bias. Route 2-way decision diamonds by visual grammar: use symmetric lower-shoulder twin S-curves in `Curved` mode (preserving single fluid visual grammar and mental-map stability when a third branch is added) and side-vertex single-bend departure in `Orthogonal` / `Rounded` mode when horizontally unobstructed.
- **Print-Grade SVG Co-Design**: place unpinned compound titles by evaluating `[TopLeft, TopRight, TopCenter]` against top-border crossings before bending edges; add background-blended title halos (`struckTitle`) when unavoidable, cross-fill edge casings (`edgeCasing`), and fill-aware ink selection (`overFills`).

### 4. Domain-Native Kernel Decomposition

Keep one public Shoal IR, `WorkControl`, DDLT verifier, and `route_paint` contract, while decomposing internal placement into compact domain-appropriate kernels:

- **Layered Core**: Flowchart, State, Class, ER, Requirement, Usecase.
- **Grid Core**: Architecture (4-way port integer grid placement + group packing + Shoal orthogonal channel routing, replacing post-Sugiyama alignment patches).
- **Lane Core**: Swimlane (single-corner cross-lane handoffs, lane-edge feedback corridors, and tight empty-rank spacing).
- **Card-Grid Core**: C4 (compact in-boundary card grid with straight-first and elbow-detour routing).

### 5. Complexity Control, Budget Consistency, Performance, and DDLT v3 Gates

- **Correctness and Crossings Edge Cases**: preserve dense layer sentinel semantics (`u32::MAX`) in `prepare_layers` so dropped label sublayers never corrupt `g.inserted_ranks` monotonicity; canonicalize undirected segment endpoints `(a, b)` / `(b, a)` in `flat/crossings.rs` so reverse-collinear segments deduplicate under `compress_shared`; prevent `O(E^2)` degradation on unbundled high-fan edges under `SolveMode::Full`.
- **Proportional `WorkControl` Accounting**: index-accelerated sweeps and queries (`ActiveIntervals::sweep` in `flat/crossings/index.rs`, `hub::Clearance::allows`, and shared BVH traversals) charge `WorkControl` proportional to actual tree nodes visited and hits reported rather than legacy linear/quadratic full-scan counts, so large sparse graphs retain the full benefit of indexing under bounded budgets.
- **High-Fan Routing by Grammar**: when `RoutingStyle::Curved` encounters `ids.len() > max_weighed_gap_pieces`, skip `greedy_tracks` and emit single-cubic radial S-curves. For `RoutingStyle::Orthogonal` / `Rounded` unlabelled high-degree fans (degree >= 6), refactor `flat/bus.rs` onto shared `geom::Bvh<T>` and `face_reach` and enable it by default as the orthogonal comb-bus path (also serving Grid Core architecture buses).
- **Shared Read-Only BVH and Scratch Buffers**. Consolidate the four static obstacle indexes in
  `label_place/clearance.rs`, `ortho/visibility.rs`, `hub.rs`, and `route_paint/clearance.rs` onto
  `merman_layout_shoal::geom::Bvh<T>`. The standalone `route-paint` feature uses a linear scan.
  Use `FxHashMap` for adapter endpoint lookup and defer edge identity materialization on the SVG
  path. Pool `refine`, `routing`, and `hub` scratch buffers in `LayoutScratch` and set the default
  `max_transpose_nodes` to `600`.
- **DDLT v3 Aesthetic Gates**: extend aesthetic validation with quantitative geometry gates for `label_optical_balance_ratio`, `max_s_curve_handle_ratio <= 0.50`, `max_s_curve_flatness <= 3.5` (preventing pancake sweeps in shallow inserted-label slits and guard-heavy compound gaps, with interior cluster gaps preserving local sweeps), `max_corner_arm_px <= 16.0` on bounded orthogonal/grid routes, `max_corner_arm_ratio <= 3.05` (preventing unbounded right-angle curved corners from bowing long straight corridors into banana arcs), `micro_kink_count == 0` across `<= 2.05 px` offsets on curved, rounded, and step routes (enforcing pitch-aware de-jitter and boundary alignment), `face_turn_coherence`, `sibling_departure_symmetry` (enforced across both inline labels and forced label sub-ranks by anchoring sibling horizon to the first physical stop), and `large_fan_cubic_ratio <= 1.05`. Enforce these gates across the real product matrix (Mermaid default and Native Theme v2 `studio`, `Curved` and `Rounded` grammars, and flat, nested-compound, Architecture Grid Core, and C4 Card-Grid families).

## Implementation notes

- Compound groups reserve perpendicular wall inset at least equal to `corner_clearance` whenever an unpinned outer corridor hugs a group boundary, eliminating forced border-stitch Z-bends when `cluster_padding < corner_clearance`.
- Grid Core and Card-Grid align perpendicular group-border crossings to the inner terminal's outward lead corridor (avoiding hook overshoots), respect `node.anchor_y` when preserving straight aligned direct connections against off-axis or occluded peers, and adapt `TitlePosition::TopAuto` across `[TopLeft, TopRight, TopCenter]` before routing.

- Adjacent bands containing only real nodes in an unlabelled flat DAG can certify their gap
  independently. The Flowchart painter builds the obstacle inventory and index on the first
  fallback query.
- Architecture and C4 share obstacle queries, group stitches, and the gutter fallback. The layered
  and Architecture kernels share the orthogonal comb geometry and its corner guards. After the
  ordinary routes are complete, an Architecture scope with a single unlabelled fan on opposing
  faces can adopt a shared crossbar using surplus work. The grid router requires an unmarked hub,
  compatible hub offsets, distinct leaf tracks, and clearance for every span, corner, and leaf
  marker. It replaces the whole fan together.
- For Neo masking, the canonical Flowchart painter measures lines and cubics in memory as it emits
  them, with the same integration tolerance and float conversion as SVG path parsing.
  Rounded paths and rewritten paths retain the SVG-based length calculation where needed.
- The native SVG painter applies fill-aware ink and casings to Flowchart and Agentflow edges when
  it can resolve solid fills and the applicable stroke style. Swimlane compacts complete bands and
  checks specialized handoffs and returns against occupied geometry before committing them.

## Consequences

- Existing Shoal SVG and visual snapshots (including release showcase graphics) will be refreshed deliberately as each wave lands; no legacy Shoal v3 style toggle will be carried forward.
- Large fan-in/fan-out SVG rendering (`fan-in-500`, `fan-out-500`) is expected to eliminate redundant cubics and BVH traversals, targeting full performance parity or superiority over Node.js Pele across all corpus scales while shrinking solver complexity.
- Implementation proceeds in five verifiable waves (Wave 0: P0 correctness fix, unified BVH, proportional `WorkControl`, `crossings` edge cases, and adapter hot-path slimming; Wave 1: single geometric source of truth, performance inversion fix, scratch pooling, and DDLT v3 gates; Wave 2: spacing scale, universal smooth/inline defaults, and grammar-aligned decision diamonds; Wave 3: adaptive compound titles, halos, and cross-fill edge casings; Wave 4: domain-native Grid/Lane/Card-Grid kernels).
- Once execution completes, superseded research drafts under `docs/research/` should be removed per `docs/README.md`.
