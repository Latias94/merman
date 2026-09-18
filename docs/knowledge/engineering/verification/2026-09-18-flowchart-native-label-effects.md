# Flowchart native label effect verification

Date: 2026-09-18.
Production source baseline: `9d8438ca9` plus this increment.
Scope: the bounded native text consumer in U6 of
`docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md`.

## Change

Flowchart NodeLabel and EdgeLabel effects can reach ordinary native SVG text when
its prepared font catalog supplies glyph paint bounds. The existing text sidecar
retains the measured ink extent, including glyph overhang and line baselines.
The viewport plan and concrete text writer share the materialized filter region
and label translation. The filter encloses only text; label backgrounds, node
shapes and edge paths retain their own paint and effect ownership.

Bindings, winning rules and explicit Clear use the existing family evidence
ledger. Expected filter applications are counted before emission; only the actual
writer records an application. A visible label without a verified consumer retains
a residual. Shape-only source strokes do not suppress a label effect. Structural
styles that can change glyph geometry or introduce unmeasured stroke/filter work
prevent a positive effect receipt.

No dependencies, public wire types, embedded production fonts or new acceptance
framework were added. The support manifest describes conditional effect support
for Flowchart node and edge labels. Public Cyberpunk recipes and qualification
cells are unchanged.

## Verification

Serial Release verification with `CARGO_BUILD_JOBS=1`:

- `merman-render` library and `flowchart_node_effects`, with
  `--no-default-features --features math,layout-cytoscape`: 2,753 passed, two skipped.
- The updated `flowchart_node_effects` target: 32 passed. This second run includes
  the later host-measurement and specialized-placement negative cases and the
  nested filter-region containment assertion.
- `merman::theme_flowchart_edge_effects`, with
  `--no-default-features --features svg,png,pdf,math,layout-cytoscape`: two passed.
  The new label scene produces chromatic text-shadow pixels in PNG, while PNG and
  PDF exports retain complete theme evidence. The existing edge-scene regression
  also passed. PDF pixel fidelity was not measured.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The tests cover text-only filter ownership, binding/rule/Clear selection,
source-only shape strokes, unsupported structural styles, unmeasured host labels,
HTML labels, specialized node placement, native glyph overhang and blank lines,
and filter bounds transformed through nested groups into the final SVG viewBox.

Simplification removed a duplicate RenderPlan bounds field and reused the existing
absent-facet constructor. A separate Codex review completed seven review lenses
with no remaining findings. Thread capacity prevented further independent
reviewers, so those lenses were performed serially by one reviewer; no external
model was used. The review identified the nested viewport test gap, which was
fixed and included in the passing 32-test run. This is scoped source review, not a
release or all-platform approval.

Local logs: `/tmp/merman-label-glow-renderer-final-20260918.log`,
`/tmp/merman-label-glow-integration-final-20260918.log`, and
`/tmp/merman-label-glow-native-final-20260918.log`.
Review receipt: `/tmp/compound-engineering-501/ce-code-review/20260918-195331-71af2969`.
These are local session artifacts.

## Limits and cost

This consumer currently requires prepared native font measurements. The ordinary
host measurement path, HTML/Markdown labels, hand-drawn rendering, Swimlane, and
node shapes with specialized label placement are outside its verified scope.
Supported ordinary node placement covers Process, RoundedRectangle, Diamond,
Circle and DoubleCircle. NodeLabel ordinal selection is supported; EdgeLabel
ordinal effects remain unsupported by the family mechanism matrix.

A cached optional four-number paint bound adds 40 accounted bytes to each prepared
label record; it avoids retaining another copy of the shaped lines. Preparing an
active effect still resolves node theme/source state and materializes its filter.
There is no measured latency or peak-memory claim in this increment. The wider
U10 impact audit remains required.

This does not close U6: public Cyberpunk text glow, host/HTML label behavior and
complete scene acceptance remain open. U7, U8, U9 and the C7a contract remain open.
