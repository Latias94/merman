# Flowchart 11.15 SVG Convergence

Status: Active
Last updated: 2026-06-01

## Why This Lane Exists

The Mermaid 11.15 complete-adaptation campaign initially showed one stored Flowchart SVG mismatch.
Fresh Mermaid 11.15 evidence proved that the old stored red point was misleading: the MathML
`columnalign` fixture was stale, but full fresh Flowchart comparison exposes hundreds of real DOM
surface gaps. Flowchart now needs its own renderer convergence lane instead of a targeted baseline
refresh.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-upstream-baseline.md`
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0050-svg-viewbox-parity.md`
  - `docs/adr/0062-fixture-derived-overrides.md`
- Existing docs:
  - `docs/rendering/UPSTREAM_SVG_BASELINES.md`
  - `docs/rendering/SVG_CANONICAL_XML.md`
  - `docs/alignment/PARITY_HARDENING_PLAN.md`
- Related workstreams:
  - `docs/workstreams/mermaid-11-15-complete-adaptation`
  - `docs/workstreams/flowchart-text-style-parity`
- Upstream reference:
  - `repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart`

## Problem

Local Flowchart parity output does not yet match Mermaid 11.15's SVG DOM contract. Fresh comparison
against Mermaid 11.15 shows 594 canonical XML mismatches and one unsupported `flowchart-elk`
layout fixture. The dominant gaps are 11.15 DOM envelope semantics, scoped identifiers, marker and
drop-shadow definitions, shape class surfaces, markdown/text row structure, `foreignObject` label
structure, subgraph cluster groups, clickable/tooltip wrappers, and ELK fixture policy.

## Target State

- Local Flowchart SVG DOM matches fresh Mermaid 11.15 output for the supported Flowchart corpus in
  `parity` mode.
- Stored Flowchart upstream SVG baselines are refreshed only after fresh Mermaid 11.15 comparison is
  green or after explicitly documented skips.
- The umbrella Mermaid 11.15 complete-adaptation lane can remove Flowchart from the full
  implemented-matrix failure set.
- Unsupported `flowchart-elk` behavior is documented as an explicit skip, follow-on workstream, or
  supported implementation decision.

## In Scope

- Flowchart SVG parity renderer behavior under `crates/merman-render/src/svg/parity/flowchart`.
- Flowchart-specific generated upstream SVG baselines under `fixtures/upstream-svgs/flowchart`
  after fresh-target gates are green.
- Targeted renderer tests for touched Flowchart surfaces.
- `xtask` compare/generation evidence needed to separate stale baselines from renderer defects.
- Skip or split policy for upstream fixtures that Mermaid 11.15 can render but local layout does
  not support, especially `flowchart-elk`.

## Out Of Scope

- Replacing the layout engine or adding full ELK layout support inside this lane unless the task
  ledger is explicitly expanded.
- Changing parser semantics that are not required by the Flowchart 11.15 SVG DOM comparison.
- Solving Class, XYChart, or other diagram-family parity gaps.
- Bulk-refreshing Flowchart stored SVG baselines before local output matches the fresh 11.15 target.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Mermaid CLI pinned in `tools/mermaid-cli` is 11.15.0. | High | `node -e "console.log(require('./tools/mermaid-cli/node_modules/mermaid/package.json').version)"` printed `11.15.0` during the umbrella lane. | Re-run generation after fixing the pinned toolchain. |
| The old stored Flowchart Math failure was stale baseline drift. | High | Fresh target and local output both included MathML `columnalign`; targeted XY-style baseline refresh pattern passed for the Math fixture after refresh. | Re-open Math fixture comparison before refreshing stored baselines. |
| The 594 fresh Flowchart mismatches are real renderer DOM gaps, not all canonicalizer noise. | High | Representative diffs show missing `outer-path`, markdown row spans, 11.15 defs, scoped ids, and wrapper attributes. | Narrow the canonicalizer before changing renderer behavior. |
| `flowchart-elk` requires an explicit policy. | High | Fresh full compare reports local layout failure: unsupported diagram type for layout: `flowchart-elk`. | Full Flowchart gates will stay red even after DOM convergence. |

## Architecture Direction

Treat Mermaid 11.15 Flowchart output as a renderer DOM contract rather than a loose visual target.
The work should proceed by fresh-target categories: first the shared DOM envelope and identity
surface, then shape/text/label slices, then cluster/click/HTML-label surfaces, and only then stored
baseline refresh. Each slice needs a representative fixture filter against the fresh 11.15 target
before it can be generalized.

Prefer localized Flowchart parity modules over broad shared SVG helper changes unless the same
Mermaid utility boundary is already shared by multiple diagrams. The goal is not to make old stored
baselines pass; the goal is to make local Flowchart output match Mermaid 11.15 and then refresh
baselines as evidence.

## Closeout Condition

This lane can close when:

- supported Flowchart fixtures pass fresh Mermaid 11.15 `parity` comparison,
- stored Flowchart baselines are refreshed only from the green fresh target,
- `flowchart-elk` is supported, skipped, or split with explicit rationale,
- the umbrella M15C-060 evidence links to the final gates,
- and follow-on work is either split or explicitly deferred.
