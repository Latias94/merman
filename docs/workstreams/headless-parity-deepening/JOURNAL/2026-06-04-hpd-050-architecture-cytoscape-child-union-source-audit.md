# HPD-050 Architecture Cytoscape Child Union Source Audit

Date: 2026-06-04

## Summary

Audited the installed Mermaid 11.15 / Cytoscape 3.33.4 source path behind Architecture service
child-union bounds. This explains the stable child-union height split and the separate final
`node.boundingBox()` expansion, but it does not justify a production formula change yet.

No code, renderer output, fixture, or baseline behavior changed.

## Source Findings

- Mermaid Architecture services are Cytoscape nodes with `width` / `height` set from
  `architecture.iconSize`, `label` from service title, and `.node-service` style width/height bound
  to those data fields.
- Mermaid sets `compound-sizing-wrt-labels: include` for nodes, `text-valign: bottom`,
  `text-halign: center`, and `font-size` from `architecture.fontSize`.
- Mermaid group SVG rectangles are drawn from `node.boundingBox()`, then `x1` / `y1` are shifted by
  `halfIconSize` before emission.
- Cytoscape `updateCompoundBounds()` computes parent content from
  `children.boundingBox({ includeLabels: true, includeOverlays: false, useCache: false })`.
- Cytoscape child label bounds use `labelWidth`, `labelHeight`, `text-halign`, `text-valign`,
  `text-margin-*`, `text-background-padding`, outline/border width, and a hardcoded
  `marginOfError = 2`.
- Cytoscape stores node `bodyBounds` separately and expands them by `1px`.
- Cytoscape default final `boundingBox()` expands the whole final bbox by another `1px`, but the
  non-default child bbox used by `children.boundingBox({ includeLabels: true })` unions the stored
  body and label boxes directly instead of applying that final expansion again.

## Mapping To Current Evidence

The source path matches the latest child-union reports:

- Browser child union is `bodyBounds` union `labelBounds.all`, not final `node.boundingBox()`.
- Browser final service `node.boundingBox()` is the child union plus a separate final 1px expansion.
- The observed local-vs-browser service child height split (`dy=+1`, `dh=-2`) is exactly the kind
  of body/label child-union phase difference that can be canceled by the later final group
  expansion.
- The direct group-width residual still depends on browser `labelWidth`. Cytoscape obtains that
  value from renderer label measurement, so the source formula alone does not turn local
  deterministic text widths into browser canvas widths.

## Rejected Production Shortcut

Do not implement a body-border or group-padding change from this audit alone:

- Body and label child-union source rules would need to be paired with final group expansion.
- The focused width rows also need browser-faithful `labelWidth`; local deterministic metrics still
  drift by service.
- Exact browser label-width lookup was already rejected as a narrow shortcut unless it is backed by
  a durable Architecture measurement seam and family-level verification.

## Evidence

- `repo-ref\mermaid` at `41646dfd43ac83f001b03c70605feb036afae46d`
- Installed `tools\mermaid-cli\node_modules\mermaid` version `11.15.0`
- Installed `tools\mermaid-cli\node_modules\cytoscape` version `3.33.4`
- `repo-ref\mermaid\packages\mermaid\src\diagrams\architecture\architectureRenderer.ts`
- `repo-ref\mermaid\packages\mermaid\src\diagrams\architecture\svgDraw.ts`
- `tools\mermaid-cli\node_modules\cytoscape\dist\cytoscape.cjs.js`
- `target\compare\architecture-delta-service-child-union-hpd050`

## Verification

- Source reads of Mermaid Architecture service/group renderer paths.
- Source reads of Cytoscape `updateCompoundBounds()`, `updateBoundsFromLabel(...)`,
  `boundingBoxImpl(...)`, and node label coordinate calculation.
- Confirmed installed package versions: Mermaid `11.15.0`, Cytoscape `3.33.4`.
- Confirmed pinned Mermaid checkout: `41646dfd43ac83f001b03c70605feb036afae46d`.

## Residual Boundary

The next production-capable seam is an Architecture service label measurement seam that can provide
browser-faithful `labelWidth` without fixture-specific lookup, then pair it with the source child
union and final group expansion phases. Until that exists, keep `parity-root` rows diagnostic.
