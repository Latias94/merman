# Headless Parity Deepening - Handoff

Status: Active
Last updated: 2026-06-02

This workstream opens the post-11.15 structural-parity phase.

Current priority order:

1. HPD-050 Architecture-first layout engine audit
2. HPD-060 semantic/render unification pilot
3. HPD-070 unsupported-family rubric

Immediate next task:

- HPD-010, HPD-020, HPD-030, and HPD-040 are done.
- Next executable slice should be HPD-050 Architecture-first layout engine audit. HPD-040 created
  the first shared measurement/root-bounds seams, so the next leverage point is to audit
  Architecture residuals through Mermaid source-backed input and bounds-feeding evidence.

Current repository reality to preserve:

- Structural `parity` is green for the implemented matrix.
- `parity-root` remains the active residual front.
- Honest top residual buckets are currently Flowchart `61`, Architecture `26`, Sequence `27`,
  Class `12`, Timeline `3`, Journey `2`.
- Sequence left-of wrapped note width semantics were improved in commit `cd9f02ff`, but a small
  root-width residual remains and should not be overfit without stronger evidence.
- Architecture remains the highest-value `manatee` / input-model audit target.
- This lane is not a license to drive every residual to zero with constants. Its purpose is to
  improve baseline truth, residual governance, and shared seams so later fixes are explainable.
- HPD-020 outcome to preserve:
  - `crates/merman-core/src/baseline.rs` owns the pinned Mermaid tag/version plus the explicit
    legacy generated suffix.
  - `Engine::default()` and live registry constructors now use
    `for_pinned_mermaid_baseline()` / `pinned_mermaid_baseline_*`.
  - xtask importers, bench entrypoints, and baseline report labels no longer present
    `default_mermaid_11_12_2*` as the active baseline truth.
  - Historical generated filenames still carry `11_12_2`; that is now explicit legacy provenance,
    not implied active baseline truth.
- HPD-030 outcome to preserve:
  - residual governance now uses six categories: source-backed behavior gap, generated measurement
    gap, browser lattice tail, stale baseline/override, solver/phase residual, and scope boundary.
  - Flowchart is currently dominated by browser lattice tails; Architecture is the main
    solver/phase front; Sequence and Class are the main generated-measurement fronts.
  - Counts remain queue-shaping hints only. They are not progress percentages and should not be
    used to justify fake completion claims.
- HPD-040 outcome to preserve:
  - `svg_emitted_bounds` is now `svg/parity` infrastructure, not State-owned code.
  - Sequence note final wrap/measure logic is centralized and reused by layout, root-bounds, and
    SVG rendering.
  - No new override growth or ad hoc constants were introduced.
  - The Sequence long leftOf note root-width residual remains open (`570px` deterministic local vs.
    `566px` upstream; headless vendored report `585px` vs `566px`) and should not be overfit.
- HPD-050 in-progress outcome:
  - Architecture's FCoSE node `BoundsExtras` adapter is now a named helper
    (`architecture_fcose_node_bounds_extras`) with direct unit coverage.
  - The layout view no longer carries group title state, matching the current source-backed rule
    that group titles do not affect the pre-layout `eles.boundingBox()` relocation center.
  - The focused batch5 long-title residual stayed unchanged at upstream `542.926px` vs local
    `547.926px`; this pass was boundary cleanup, not a hidden root-width tune.
  - A second source-backed bounds slice fixed Architecture edge-label root bounds:
    `createText()` local y-range is now transformed for X/Y edge labels instead of being treated as
    a centered AABB, and compound label bottom now uses the source-backed `fontSize + 1px` rule.
  - This made `stress_architecture_batch4_init_small_icons_061`,
    `stress_architecture_batch4_init_fontsize_wrap_063`, and
    `stress_architecture_edge_label_corner_cases_012` root-green without adding root overrides.
  - Full Architecture structural parity is still green; Architecture `parity-root` now has `26`
    mismatches. The remaining top tails are still `junction_fork_join_026` (`+13.976px`),
    `batch5_long_titles_and_punct_076` (`+5.000px`), and `html_titles_and_escapes_041`
    (`+5.000px`).
  - A follow-up `junction_fork_join_026` audit found no new source-input mismatch, but corrected
    the earlier baseline-drift reading. Current local service positions match the saved Mermaid
    old debug probe `target/compare/arch_junction_fork_join_probe_m15rv089.json` to floating-point
    noise; however, a fresh Edge-backed `check-upstream-svgs` run reproduces the stored upstream
    fixture exactly. Treat this row as a probe-harness / CLI-harness divergence plus solver/phase
    residual candidate; do not tune manatee against the saved debug probe alone.
  - A follow-up bounds-seam cleanup removed the unused renderer-side `initial_center` / pre-layout
    group bbox model and renamed the old generic compound padding helper to
    `architecture_svg_group_bbox_padding_px(...)`. The remaining `batch5_long_titles` and
    `html_titles` `+5px` tails are confirmed group/service Cytoscape bbox measurement residuals;
    do not close them by globally removing the final SVG group bbox extra.
  - A probe-harness correction updated
    `tools/debug/arch_fcose_browser_probe_fixture_025.js` to be explicit that it is a manual
    diagnostic reconstruction, not an authoritative Mermaid CLI render. It now mirrors xtask's
    deterministic page prelude more closely and reads shipped Architecture FCoSE config fields, but
    it still does not reproduce the stored CLI fixture for `junction_fork_join_026`. Important:
    the installed `mermaid@11.15.0` dist used by `tools/mermaid-cli` does not contain the later
    repo-ref `withSeededRandom` Architecture seed path, so do not change `manatee` to that
    `mulberry32` behavior unless the baseline package changes.
  - The local `repo-ref/mermaid` checkout is currently on `develop` at
    `9bae92cd3214f9ec99369ab314ef41ffb283f6b6`, while `tools/upstreams/REPOS.lock.json` pins
    Mermaid to `41646dfd43ac83f001b03c70605feb036afae46d`. For any source-backed HPD-050 claim,
    use `git -C repo-ref/mermaid show 41646dfd43ac83f001b03c70605feb036afae46d:<path>`, the
    installed `tools/mermaid-cli` dist, or fresh `check-upstream-svgs` output. Do not treat the
    current repo-ref working tree as baseline truth without this check.
  - A follow-up Cytoscape bbox phase audit enhanced the debug probe to expose pre-layout
    `labelWidth`, `labelHeight`, `labelBounds`, `bodyBounds`, `autoWidth`, `autoHeight`, and
    `autoPadding`. For `batch6_init_fontsize_icon_size_wrap_093`, the probe explains the remaining
    `-2.5px` row: leaf service `node.boundingBox()` differs from the child contribution used by
    `updateCompoundBounds()`. An exploratory global production formula made that row exact but
    expanded the full Architecture root mismatch count from `26` to `47`, so it was rejected and
    reverted. The next real fix needs a phase-specific bbox model rather than a single global group
    padding/label-width rule.
  - The first safe follow-up to that finding is now landed: Architecture service bounds estimate
    fields are named by phase (`emitted_icon_bounds`, `svg_root_bounds`,
    `cytoscape_group_child_bounds`). This did not change behavior: structural Architecture parity
    is green, and `parity-root` remains the expected 26 mismatches.
  - `repo-ref/dagre` and `repo-ref/graphlib` are now present and checked out to the pinned
    lockfile commits, so dugong/graphlib source-backed audits no longer have to proceed from stale
    assumptions.
  - `dugong-graphlib` now has an explicit upstream coverage ledger in
    `docs/dugong/GRAPHLIB_UPSTREAM_TEST_COVERAGE.md`. The first direct Graphlib source-test slice
    ports the exposed helper algorithms (`components`, `findCycles`, `preorder`, `postorder`) and
    tightens missing-root traversal behavior to panic like upstream Graphlib throws.
  - `tools/dagre-harness/run.mjs` now imports `dagre-d3-es` from the installed
    `tools/mermaid-cli/node_modules` baseline and is executable again. A focused State `basic`
    Dagre layout comparison reported zero node and edge delta.
  - The next dugong-adjacent audit should target the public Graphlib `Graph` API subset used by
    `dugong` and Mermaid-facing renderers. Do not spend HPD-050 budget implementing unused
    shortest-path algorithms unless a real Mermaid/Dagre path needs them.
  - That public Graphlib `Graph` API audit has started. The first `graph-test.js` slice now covers
    options, labels, node/edge basics, named multiedges, compound parent moves, root children, and
    remove-node cleanup. `set_parent_ix(...)` now enforces the upstream tree invariant by panicking
    if a parent assignment would create a cycle. The non-compound `setParent(...)` throw remains an
    open Rust API-shape decision.
  - The next public Graph API slice covers source-backed edge/adjacency queries: `sinks`,
    predecessor/successor/neighbor queries, `isLeaf`, `inEdges`, `outEdges`, `nodeEdges`, and
    remove-edge neighbor count behavior. New Rust API seams are limited to `sinks`, `is_leaf`, and
    `node_edges_between`, because those map to existing Graphlib behavior and real graph consumers.
    Missing-node `undefined` returns and JS chainable mutators remain documented Rust/JS API-shape
    differences, not hidden parity claims.
  - A follow-up Graphlib edge-invariant slice now matches upstream's non-multigraph named-edge
    guard: named edge insertion panics on simple graphs, and named lookup/removal does not alias the
    unnamed edge. Renderer graph construction that uses edge names is already multigraph-based, so
    this should be treated as a source-backed invariant fix rather than a rendering tune.
  - ARCH-022's first Dagre reference adapter slice is now landed. The Rust-side input schema,
    Rust/JS output comparison, JS harness invocation, and compound-edge normalization now live in
    `crates/xtask/src/cmd/debug/dagre_reference.rs`; `compare-dagre-layout` remains State-only and
    acts as a graph producer/command wrapper. Basic, composite, and internal-cluster State Dagre
    comparisons all reported zero node and edge delta after the extraction. Do not broaden this to
    other diagrams until a real Dagre-backed residual audit needs that producer.
