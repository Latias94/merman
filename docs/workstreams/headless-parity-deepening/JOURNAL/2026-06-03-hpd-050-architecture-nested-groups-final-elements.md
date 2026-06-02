# HPD-050 - Architecture Nested Groups Final Elements

Task: HPD-050 Architecture-first layout engine audit

## What Changed

Re-audited `stress_architecture_nested_groups_002` with the enhanced Architecture browser probe and
local group-rect debug output.

No renderer behavior changed in this slice.

## Source Evidence

Pinned Mermaid source at `repo-ref/mermaid` commit `41646dfd43ac83f001b03c70605feb036afae46d`
keeps the relevant rules explicit:

- `architectureRenderer.ts` gives Cytoscape `.node-group` padding from
  `db.getConfigField('padding')`.
- `svgDraw.ts` renders group rectangles from final `node.boundingBox()` and writes
  `x = x1 + iconSize / 2`, `y = y1 + iconSize / 2`, `width = w`, and `height = h`.

So a valid fix for this row needs to affect the final Cytoscape/compound bbox phase. It should not
change SVG group-rect drawing or replace configured padding with another proxy.

## Evidence

Current focused local compare remains an expected root-only failure:

- `target/compare/architecture_nested_groups_hpd050_current_debug.md`
- upstream max-width `727.924px`, local max-width `730.424px`
- upstream viewBox `727.924x622.658`, local viewBox `730.424x622.658`

Fresh browser probe:

- `target/compare/arch_nested_groups_probe_hpd050_final_elements.json`

Browser final group bboxes:

- `platform` `node.boundingBox().x1=-126.19219118912804`, `w=459.15408506809246`,
  `h=542.6580855100466`
- `runtime` `node.boundingBox().x1=-79.69219118912804`, `w=365.65408506809246`, `h=182`
- `data` `node.boundingBox().x1=-84.69219118912804`, `w=376.15408506809246`, `h=182`

Pinned upstream SVG group rects match those bboxes after adding `iconSize / 2 = 40` to `x1`:

- `platform` rect `x=-86.19219118912804`, `w=459.15408506809246`
- `runtime` rect `x=-39.69219118912804`, `w=365.65408506809246`
- `data` rect `x=-44.69219118912804`, `w=376.15408506809246`

Current local SVG group rects:

- `platform` rect `x=-81.9421911891281`, `w=458.65408506809246`
- `runtime` rect `x=-38.442191189128096`, `w=365.65408506809246`
- `data` rect `x=-40.442191189128096`, `w=375.65408506809246`

Browser final service positions are:

- `ingress=(-274.96189387896436,-121.82904275502335)`
- `svc1=(2.807808810871961,-121.82904275502335)`
- `svc2=(203.46189387896442,-121.82904275502335)`
- `db=(2.807808810871961,155.82904275502324)`
- `obj=(203.46189387896442,155.82904275502324)`

Current local SVG service positions are all shifted about `+1.25px` on X and match Y:

- `ingress=(-273.7118938789644,-121.82904275502335)`
- `svc1=(4.057808810871904,-121.82904275502335)`
- `svc2=(204.71189387896436,-121.82904275502335)`
- `db=(4.057808810871904,155.82904275502324)`
- `obj=(204.71189387896436,155.82904275502324)`

Local group debug reports the existing configured-padding path:

- `runtime` content `(4.057808810871904,-121.82904275502335)-(284.71189387896436,-24.82904275502335)`,
  `pad=42.5`, final width `365.65408506809246`.
- `data` content `(2.0578088108719044,155.82904275502324)-(292.71189387896436,252.82904275502324)`,
  `pad=42.5`, final width `375.65408506809246`.
- `platform` unions inset child groups, uses `pad=42.5`, and emits final width
  `458.65408506809246`.

## Classification

This row remains a nested compound-bounds phase residual. It is not evidence for changing:

- Mermaid source-backed SVG group rect translation,
- configured Architecture padding,
- root viewport finalization,
- edge path emission.

The local service placement is uniformly shifted by about `1.25px` on X, while nested group width
propagation differs by `0.5px` on the `data` and `platform` groups. That pattern is too small and
too phase-dependent for a safe global rule. Keep this row classified until a reusable Cytoscape
compound-bound model explains it together with the other Architecture queue rows.
