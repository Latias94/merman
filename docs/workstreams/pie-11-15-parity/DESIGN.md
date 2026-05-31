# Pie 11.15 Parity

Status: Closed
Last updated: 2026-05-31

## Why This Lane Exists

The Mermaid 11.15 baseline upgrade left Pie-specific renderer behavior behind the rest of the
existing-diagram matrix. The generated-default-config closeout intentionally removed Pie 11.15 keys
from the committed default config because the renderer did not yet implement the behavior.

While checking those keys, we found one additional 11.15 behavior change: upstream Pie rendering now
uses `d3pie().sort(null)`, so slices render in input order. The local layout still sorts slices by
descending value, which matches older Mermaid behavior but not the current baseline.

## Relevant Authority

- Upstream renderer: `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieRenderer.ts`
- Upstream styles: `repo-ref/mermaid/packages/mermaid/src/diagrams/pie/pieStyles.ts`
- Upstream schema: `repo-ref/mermaid/packages/mermaid/src/schemas/config.schema.yaml`
- Upstream docs: `repo-ref/mermaid/packages/mermaid/src/docs/syntax/pie.md`
- Local renderer: `crates/merman-render/src/pie.rs`
- Local SVG emitter: `crates/merman-render/src/svg/parity/pie.rs`
- Default config ADR: `docs/adr/0019-generated-default-config.md`
- Prior generated-config lane: `docs/workstreams/generated-default-config-parity/`

## Problem

Current Pie support is Stage B capable, but it still assumes a 11.12-era renderer contract:

- visible slices are sorted by descending value instead of input order,
- `pie.textPosition` is effectively hardcoded to `0.75`,
- `pie.donutHole` is not modeled in layout or path emission,
- `pie.legendPosition` is always treated like the right/default layout,
- `pie.highlightSlice` has no slice class or CSS support,
- and the generated default config removes these keys to avoid claiming unsupported behavior.

## Target State

- Pie slice geometry and color assignment match Mermaid 11.15 input-order behavior.
- `pie.textPosition`, `pie.donutHole`, `pie.legendPosition`, and `pie.highlightSlice` are present in
  generated defaults and read from the effective config.
- Default output remains compatible with existing Pie fixtures except where the 11.15 input-order
  behavior requires an intentional baseline change.
- Configured Pie variants are covered by public renderer tests and, where useful, upstream SVG
  fixture comparisons.
- The generated-default-config override manifest no longer removes supported Pie keys.

## In Scope

- Pie layout and SVG renderer changes.
- Generated default config and override manifest updates for Pie keys.
- Targeted Pie tests for order, donut geometry, text position, legend positions, and highlighting.
- Fixture or upstream SVG baseline updates only when they directly prove the Pie behavior.
- Docs and workstream evidence.

## Out Of Scope

- New Mermaid diagram families.
- Replacing the current hand-authored Pie geometry with a D3 runtime dependency.
- Broad theme-system parity beyond existing Pie theme variables and highlight CSS.
- Non-Pie generated-artifact changes.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Mermaid 11.15 renders visible Pie slices in input order. | High | `pieRenderer.ts` uses `d3pie().sort(null)`. | PIE-020 scope changes to fixture-only validation if a later renderer layer re-sorts. |
| Pie config keys are no-op defaults except for configured variants. | High | Schema defaults are `textPosition=0.75`, `donutHole=0`, `legendPosition=right`, `highlightSlice=''`. | Restoring keys may require a broader fixture refresh. |
| Existing local text measurement is sufficient for legend-position viewBox sizing. | Medium | Right-side legend parity already uses measured legend width. | PIE-050 may need extra root override or measurement correction work. |
| Highlight behavior can be represented with static classes and CSS. | High | Upstream uses `highlighted` and `highlightedOnHover` class names plus CSS. | Runtime interactivity may need a browser-level follow-up, but static SVG structure remains useful. |

## Architecture Direction

Keep Pie parity inside the current two-phase model:

- `crates/merman-render/src/pie.rs` owns layout geometry and config-derived positions.
- `crates/merman-render/src/svg/parity/pie.rs` owns emitted paths, classes, and legend DOM.
- `crates/merman-render/src/svg/parity/css.rs` owns Pie CSS additions.
- Generated config stays deterministic through `xtask gen-default-config`.

Prefer small local config helpers over a new Pie-specific abstraction until the second configured
feature proves one is needed.

## Closeout Condition

This lane can close when:

- Pie input-order rendering is proven,
- the four 11.15 Pie config keys are restored in generated defaults,
- configured donut, legend position, text position, and highlight behavior have targeted tests,
- `verify-default-config` is green,
- and any remaining Pie parity debt is explicitly split into a follow-on.

## Closeout Summary

Closed on 2026-05-31. PIE-020 through PIE-060 met the target state:

- visible Pie slices now follow Mermaid 11.15 input order,
- hidden slices reserve color-domain slots like upstream,
- Pie 11.15 config defaults are generated and exposed,
- `textPosition`, `donutHole`, `legendPosition`, and `highlightSlice` are implemented with targeted
  renderer tests,
- and closeout gates are recorded in `EVIDENCE_AND_GATES.md`.

No follow-on is required for the scoped Pie 11.15 behavior.
