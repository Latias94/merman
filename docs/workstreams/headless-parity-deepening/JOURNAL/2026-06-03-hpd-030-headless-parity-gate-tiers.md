# HPD-030 - Headless Parity Gate Tiers

Date: 2026-06-03

## Context

The Architecture child source-phase experiments showed that directly importing a browser/Cytoscape
source formula can improve a couple of root rows while damaging the family. This forces a clearer
first-principles policy for what "Mermaid parity" means in a headless Rust renderer.

## First Principles

Mermaid JS renders through a browser. Some of its final numbers are not stable semantic facts; they
are the result of browser font measurement, SVG `getBBox()`, Cytoscape bbox expansion, FCoSE solver
iteration, and final serialization. `merman` should reproduce Mermaid's behavior where it matters
for users and source semantics, while owning a deterministic headless model for browser-derived
tails.

## Gate Tiers

Hard gates:

- parser and semantic model behavior for implemented families;
- error behavior where upstream fixtures prove it;
- diagram-specific theme/CSS emission and readable visual output;
- structural SVG DOM parity for the implemented matrix;
- no blank output, hidden labels, black blocks, root clipping, or lost semantic colors.

Strong alignment targets:

- source-backed layout topology, parent/child membership, edge endpoints, relative constraints, and
  label wrapping;
- source-backed reusable measurement/root-bounds seams that improve a family without worsening the
  surrounding suite.

Diagnostic sensors:

- `parity-root` `max-width` / `viewBox` numeric tails;
- small browser lattice differences from `getBBox()`, `getComputedTextLength()`, canvas
  `measureText()`, Cytoscape body/label bounds, and FCoSE decimal solution drift;
- residual counts used to shape the queue, not to claim completion percentages.

Non-goals:

- depending on a browser at runtime;
- fixture-keyed or text-table hacks to clear individual root rows;
- copying browser measurement artifacts when they damage the deterministic headless family model.

## Outcome

`parity-root` remains valuable because it finds real source-backed bugs and catches regression
growth. It should not be treated as a blanket mandate to force every browser-derived pixel tail to
zero. A root residual becomes a production fix only when it is source-backed, visible/user-facing,
or explained by a reusable seam that survives family-level verification.
