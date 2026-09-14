# Block NodeLabel Fill Cutover Boundary

This record defines the evidence required before retiring Block's remaining `NodeLabel.fill`
legacy projection. It is a migration boundary, not a support claim.

## Status after the terminal repair

This document retains the pre-cutover design and its required evidence. The static NodeLabel
routes now have a per-node typed writer and executable SVG/PNG route authorization; see the
[terminal repair record](2026-09-14-block-node-label-terminal-repair.md) for exact validation scope.
The shared Block provider still serves generic Text and other legacy mechanisms. Its complete
retirement, including remaining probes, is not implied by the NodeLabel route migration.

## Historical owner before cutover

Before cutover, Block `NodeLabel.fill` was consumed through the shared `nodeTextColor` compatibility value.
The Block writer emitted that value through both label families:

- HTML labels: `.label`, `span.nodeLabel`, and `p` inside `foreignObject`;
- SVG labels: `.label text` and the generated `tspan` text sink.

At that point, `BlockNodePaintThemePlan` owned node-shell `fill` and `stroke` only. It did not own
label paint or prove the final text color at each visible label terminal.

## Required cutover evidence

A safe typed replacement must provide all of the following in one source revision:

1. A Block-only static unqualified/`Default` `NodeLabel.fill` matrix route for solid and
   transparent values, with independent route authorization and residual handling.
2. Source/config ownership checks per node and per property. Inline and class-defined text color
   must suppress typed ownership only for the affected label, while node-shell paint remains
   independently eligible.
3. A family-local label paint plan shared by HTML and SVG writers. The plan must resolve the
   winning rule once and expose the final CSS token to both sinks.
4. A writer-owned receipt that checks every non-empty visible label, rejects missing, duplicate,
   or mismatched text terminals, and distinguishes HTML and SVG emission paths.
5. Native SVG and PNG witnesses for solid and transparent colors across the supported Block looks,
   including a mixed source-owned/typed scene.
6. Updated support discovery, route-cutover authorization, KTD17/KTD23 evidence, and retirement
   tests. The legacy provider and its Block probe may be removed only after these receipts pass.

## Non-goals

This cutover does not qualify ordinal, gradient, effect, `Title`, or `ClusterLabel` routes. Empty
labels remain NotApplicable; source-owned labels remain property-local and must not be counted as
Typed Applied.

Until this record's requirements are implemented and independently exercised, the Block legacy
projection remains intentional and must not be removed merely to reduce the inventory count.

## Implementation shape

The evidence model should follow the existing `FlowchartTextSurfacePaintPlan` pattern: resolve the
family request once, retain a family-local receipt, and reconcile the request only after the writer
has observed its terminals. Block must keep its own terminal identity because HTML `foreignObject`
labels and SVG `text`/`tspan` labels do not share the same DOM sink. Reusing the Flowchart plan
itself would erase that distinction and would make a passing CSS check insufficient proof of Block
terminal coverage.

The first implementation slice should therefore land the Block plan and writer receipt together,
with the matrix and cutover-manifest changes in the same revision. A plan that is compiled but not
wired into both writers is incomplete and must not change the route disposition.
