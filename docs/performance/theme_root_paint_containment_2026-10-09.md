# Neo source paint containment — 2026-10-09

## Failure

The unchanged Chromium root viewport oracle reported 11 new blocking cases among
3,712 compared SVGs: one Flowchart and ten Sequence diagrams. The full DOM
structure/parity/parity-root gate passed, because numeric root extents are
browser-dependent diagnostics except for deterministic root canaries.

The Sequence overflow came from the `g rect.rect` box frames, not participant
rectangles. For `sequence/box_variants`, the box bottom was at 216 SVG units;
the one-pixel outline and emitted zero-blur shadow translated by four units
painted through 220.5. The root ended at 217. Flowchart's zero-padding title and
subgraph fixture omitted cluster outline and ordinary Neo rectangle shadow
paint. Font-dependent title extents were a separate browser residual.

## Correction

- Sequence box emission and source paint preparation share one rectangle geometry
  function. Root construction merges the default box shadow envelope in both
  immediate and deferred viewport paths.
- Flowchart merges known cluster outline and default Neo rectangle shadow paint
  after applying user diagram padding. Title placement and the geometric layout
  remain independent of this additional paint envelope.
- The source shadow displacement is shared with the affected filter writers.
  No comparator rule, residual receipt, or upstream SVG was changed.
- Classic padding contracts and explicit source filters retain their existing
  ownership. These bounds do not certify arbitrary CSS filters or host fonts.

## Validation

The focused browser loop first reproduced all 11 blocking cases. After the
correction, those same fixtures had no structural paint violations, and the
unchanged oracle classified them as upstream-inherited diagnostics. Flowchart
and Sequence owner tests passed 294/294, with one configured skip. The full
three-mode SVG gate passed. Native preset and legacy projection admission checks
passed 8/8. The full Chromium corpus passed with 3,712 fixtures, zero blocking
containment failures, 54 browser-owned diagnostics, 2,344 upstream-inherited
cases, one existing exact residual, and zero unused residuals.
