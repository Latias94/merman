# Block NodeLabel Fill Cutover Boundary

This record defines the evidence required before retiring Block's remaining `NodeLabel.fill`
legacy projection. It is a migration boundary, not a support claim.

## Current owner

Block `NodeLabel.fill` is still consumed through the shared `nodeTextColor` compatibility value.
The Block writer emits that value through both label families:

- HTML labels: `.label`, `span.nodeLabel`, and `p` inside `foreignObject`;
- SVG labels: `.label text` and the generated `tspan` text sink.

`BlockNodePaintThemePlan` currently owns node-shell `fill` and `stroke` only. It does not own
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
