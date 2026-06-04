# HPD-050 - Architecture Render-Path Probe

Date: 2026-06-04
Task: HPD-050 Architecture-first layout engine audit

## Context

`stress_architecture_junction_fork_join_026` remained the largest Architecture root residual, but
older evidence had an important split: the manual ArchitectureDB/FCoSE browser probe was closer to
local Rust output, while fresh `check-upstream-svgs` reproduced the stored upstream fixture exactly.
That made the manual probe useful for phase inspection but unsafe as the target geometry.

## Change

Added `tools/debug/arch_render_path_probe_fixture.js`.

The script runs `mermaid.render(...)` through the installed Mermaid CLI browser environment and
patches the installed Mermaid `11.15.0` IIFE in memory. It records Architecture Cytoscape stages
from the bundled renderer path:

- `layout-before-run1`
- `layoutstop-run1-before-segments`
- `layoutstop-run1-after-segments-before-run2`
- `cy-ready-before-resolve`
- `draw-after-layout-before-svg-emission`
- `draw-after-position-nodes-before-viewbox`

This is intentionally different from `arch_fcose_browser_probe_fixture_025.js`, which parses with
Mermaid and manually rebuilds Cytoscape/FCoSE inputs outside the actual render path.

## Findings

The render-path probe for `stress_architecture_junction_fork_join_026` reproduced the stored
upstream SVG facts exactly:

- viewBox:
  `-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375`
- max-width: `2808.126708984375`
- group `left`: `1788.5571178808743 x 1649.1539928009868`

The captured stage split is also explicit now:

| stage | graph bbox w/h | `left` group bbox w/h |
|---|---:|---:|
| `layoutstop-run1-before-segments` | `2743.102 / 2465.033` | `1805.888 / 1630.544` |
| `cy-ready-before-resolve` | `2729.127 / 2477.535` | `1788.557 / 1649.154` |
| `draw-after-layout-before-svg-emission` | `2729.127 / 2477.535` | `1788.557 / 1649.154` |

The `draw-after-layout-before-svg-emission` stage matches the final stored SVG group rectangles.
So the stored fixture is still authoritative, and the remaining `junction_fork_join_026` split is
not a stale-baseline problem.

The actual render path uses the installed bundled Mermaid/Cytoscape/FCoSE stack:

- `mermaid@11.15.0`
- `cytoscape@3.33.4`
- `cytoscape-fcose@2.2.0`
- nested `cose-base@2.2.0`
- nested `layout-base@2.0.1`

Note that top-level `cose-base` / `layout-base` package reads are not enough to identify the code
used by bundled Architecture rendering.

## Outcome

No production renderer behavior changed.

`junction_fork_join_026` is now narrowed to a manatee-vs-bundled-Cytoscape/FCoSE internal phase
residual. Future junction work should instrument the bundled render path or build a reference
harness against the same nested FCoSE/Cose stack. Do not tune manatee against the manual
ArchitectureDB reconstruction probe when it disagrees with this render-path evidence.

## Verification

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; node tools\debug\arch_render_path_probe_fixture.js stress_architecture_junction_fork_join_026 > target\compare\architecture-render-path-probe-hpd050\stress_architecture_junction_fork_join_026.render-path-probe.json`
- Read the generated probe JSON and compared `renderedFacts` with `storedFacts`.
