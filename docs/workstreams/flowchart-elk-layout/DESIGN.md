# Flowchart ELK Layout - Design

Status: Active
Last updated: 2026-06-14

## Problem

`merman` already renders `flowchart-elk` through a lightweight local ELK subset, but the upstream
Mermaid surface is wider than the current smoke cases. We need a separate lane to decide which
fixtures can be admitted with incremental subset work and which ones actually justify a full ELK
port.

## Intent

Keep the default path small and deterministic. Admit the low-risk ELK cases first, extend the
subset where it clearly buys parity, and only consider a full ELK port if the remaining fixtures
are dominated by hierarchy/order/section semantics that the subset cannot represent cleanly.

## Target State

- The current smoke fixtures remain renderable.
- ELK fixtures are classified by difficulty instead of being treated as one undifferentiated block.
- ELK fixture probes can run explicitly without weakening the default Flowchart parity matrix.
- The lane can answer, with evidence, whether any remaining fixture class truly needs a full port.
- If a deeper port is ever chosen, it stays isolated from the default MIT/Apache workspace surface.

## Scope

- `crates/merman-layout-elk`
- `crates/merman-render/src/flowchart/elk.rs`
- `crates/xtask/src/cmd/compare`
- `crates/xtask/src/cmd/import`
- `repo-ref/mermaid/cypress/integration/rendering/flowchart/flowchart-elk.spec.js`

## Non-goals

- Do not clone the full upstream ELK workspace into the default build just to chase parity.
- Do not promise pixel-for-pixel ELK parity before the fixture classes are split and measured.
- Do not regress the non-ELK Flowchart lane while ELK is being expanded.

## ELK Fixture Tiers

| Tier | Upstream cases | Why it belongs here |
| --- | --- | --- |
| Tier A | `1-8`, `V2 elk - 16`, `1433`, `2388`, `2824`, `6647`, `7213` | Simple smoke cases, basic labels, `diagramPadding`, `useMaxWidth`, title margin, default-node names, clipping, node order, and right-angle edges. These are the right first admission candidates for the lightweight subset. |
| Tier B | `50-76`, `2050`, `58-65`, markdown string cases, `74` multi-edge labels, `6080-6088` | Nested subgraphs, subgraph direction, outgoing links, style/class coverage, multi-edge labeling, and diamond clipping/intersections. These should be solved by subset growth first. |
| Tier C | Any remaining case that depends on `elk.hierarchyHandling`, cross-cluster rewrite/backfill, edge sections/ports, or exact order/crossing semantics | This is where a full port starts to make sense. If the fixture only fails because the subset cannot express the needed hierarchy model, it belongs here. |

## Fixture Admission Map

| Batch | Candidate fixtures | Expected work |
| --- | --- | --- |
| A0 - current smoke | `render_svg_returns_svg_for_flowchart_elk`, `headless_renderer_renders_flowchart_elk_svg`, and the already active non-ELK fixture `upstream_cypress_flowchart_elk_spec_render_with_stylized_arrows_063` | Keep default renderability and avoid confusing spec-file provenance with actual `layout: elk` coverage. |
| A0.5 - explicit probes | `upstream_html_demos_flowchart_elk_flowchart_elk_001` and later selected Tier A imports | Run with `--include-elk-probes`; do not admit to default parity until DOM shape and layout geometry pass. |
| A1 - simple ELK | Upstream `1-elk` through `8-elk`, `1433-elk`, `2388-elk` | Import/probe simple `flowchart-elk` and `layout: elk` cases, then admit only the fixtures whose DOM and geometry drift is understood and closed. |
| A2 - routing basics | `4-elk`, `2824-elk`, `7213` | Tighten edge length, clipping, and right-angle orthogonal routing without introducing full ELK sections. |
| B1 - hierarchy basics | `50-57.x`, `66-74` nested/outgoing subgraph cases | Improve parent/cluster handling and cross-subgraph edges. |
| B2 - local direction | `2050-elk`, direction-specific nested cases `66-72` | Make `Node.direction` affect nested layout instead of only using the graph direction. |
| B3 - labels/style pressure | `58-65`, `74` multi-edge labels, markdown string cases, `76` unicode HTML labels | Verify the ELK adapter preserves label measurement and SVG surfaces already covered by the normal Flowchart renderer. |
| B4 - shape intersections | `6080`, `6088-1` through `6088-6` | Decide whether diamond/cluster clipping can remain a local geometry improvement. |
| C - port decision | Residuals that require ELK hierarchy handling, edge sections/ports, or exact crossing minimization | Use this batch to justify, or reject, a full-port experiment. |

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The current lightweight backend is enough for the first smoke batch. | High | `render_svg_returns_svg_for_flowchart_elk`, `headless_renderer_renders_flowchart_elk_svg`. | Reclassify the smoke fixtures before widening the lane. |
| Nested subgraph direction and hierarchy are the main non-smoke pressure points. | High | `flowchart-elk.spec.js` cases `50-76`, `2050`, `6080-6088`. | If they collapse cleanly into the subset, a full port is not needed yet. |
| A full port is only justified if the remaining fixture classes need ELK hierarchy semantics we cannot model locally. | High | `repo-ref/mermaid/packages/mermaid-layout-elk/src/render.ts` and the upstream ELK spec surface. | Keep the lane subset-first and isolate any deeper port as a separate decision. |

## Architecture Direction

Prefer explicit, typed subset growth:

1. carry Flowchart direction and label data through the adapter;
2. keep the local graph model small and deterministic;
3. admit fixtures in batches that prove real benefit;
4. only introduce a deeper ELK port boundary if the fixture evidence says the subset has hit a wall.

## Closeout Condition

This lane can close when:

- the fixture tiers are documented and stable,
- the smoke batch is covered by the lightweight backend,
- the remaining cases are either admitted by subset growth or explicitly parked as full-port territory,
- and the repository has a clear answer on whether a full ELK port is actually warranted.
