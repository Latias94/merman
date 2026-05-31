# Mermaid 11.15 Baseline Upgrade

Status: Closed
Last updated: 2026-05-31

## Why This Lane Exists

At lane opening, `merman` documented and tested against `mermaid@11.12.3`, while the local upstream
Mermaid checkout was at `packages/mermaid@11.15.0`. Moving the baseline forward changes parser
semantics, renderer defaults, configuration surfaces, SVG ID behavior, and diagram family coverage.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-upstream-baseline.md`
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0019-generated-default-config.md`
  - `docs/adr/0047-layout-golden-snapshots.md`
  - `docs/adr/0050-svg-viewbox-parity.md`
  - `docs/adr/0062-fixture-derived-overrides.md`
- Existing docs:
  - `docs/alignment/STATUS.md`
  - `tools/upstreams/REPOS.lock.json`
  - `repo-ref/mermaid/packages/mermaid/CHANGELOG.md`
- Related workstreams:
  - `docs/workstreams/flowchart-text-style-parity`
  - `docs/workstreams/architecture-indexed-fcose`
  - `docs/workstreams/root-viewport-derivation`

## Problem

A pure version bump would claim `11.15.0` parity before the Rust parser, semantic models, layout
configuration, SVG output, and coverage docs actually match the new upstream behavior.

## Target State

The workstream can move the documented baseline to `mermaid@11.15.0` only after the selected
existing-diagram compatibility deltas are implemented, validated, and reflected in alignment docs.
New diagram families introduced or present upstream are explicitly supported, deferred, or recorded
as out of scope.

## In Scope

- Existing-diagram deltas from Mermaid `11.13.0` through `11.15.0`.
- First priority:
  - sequence decimal `autonumber` start/increment values,
  - flowchart `datastore` shape and rounded curve behavior,
  - architecture FCoSE knobs and deterministic `randomize`,
  - sankey `nodeWidth`, `nodePadding`, `labelStyle`, and `nodeColors`,
  - xyChart `dataLabelColor` and `showDataLabelOutsideBar`,
  - class hierarchical namespaces and namespace notes,
  - internal SVG ID prefixing.
- A documented scope decision for new diagram families such as `eventmodeling`, `wardley-beta`,
  `treeView`, `venn-beta`, and `ishikawa-beta`.

## New Family Scope Decision

No new diagram family is promoted into this lane's implementation scope. The 11.15 baseline bump
may claim the existing supported diagram matrix plus completed 11.13-11.15 compatibility deltas,
but it must explicitly defer `eventmodeling`, `wardley-beta`, `treeView-beta`, `venn-beta`, and
`ishikawa(-beta)`.

Additional upstream families that are present in the Mermaid 11.15 source tree but not part of the
current local coverage corpus, including `cynefin-beta` and `railroad-*`, are out of scope unless a
later workstream promotes them with a parser/model/layout/render plan.

## Out Of Scope

- Claiming full `11.15.0` baseline parity before evidence gates pass.
- Rewriting unrelated renderers while a targeted compatibility slice is active.
- Adding every upstream diagram family in this lane unless the scope decision promotes it.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| At lane opening, the documented baseline was still `mermaid@11.12.3`. | High | `README.md`, ADR-0001, `REPOS.lock.json` | Baseline docs must be updated after compatibility work, not before. |
| The local upstream checkout contains Mermaid `11.15.0`. | High | `repo-ref/mermaid/packages/mermaid/package.json` | Re-check upstream source before fixture generation if the checkout moves. |
| Existing-diagram deltas are safer first proof slices than new diagram families. | High | Current 23-diagram parity corpus in `docs/alignment/STATUS.md` | New diagram family work should be split if it dominates the lane. |

## Architecture Direction

Keep parser and render changes local to each existing diagram module, and add compatibility tests at
the semantic/layout/SVG layer that directly proves the upstream release delta. Do not update the
global baseline metadata until the selected deltas and fixture updates are complete.

## Closeout Condition

This lane can close when:

- selected `11.13.0` to `11.15.0` existing-diagram deltas are implemented or explicitly deferred,
- new diagram family scope is recorded,
- targeted and package-level gates have fresh evidence,
- baseline docs and lock metadata accurately describe the shipped state,
- and remaining work is split into follow-on lanes.

## Closeout Result

Closed on 2026-05-31 after M15-100. The documented baseline, upstream lock metadata, and local
Mermaid CLI toolchain now target Mermaid `11.15.0` for the implemented diagram matrix. New diagram
families remain explicitly deferred or out of scope in `docs/alignment/STATUS.md`.

One early flowchart assumption was corrected during closeout: Mermaid 11.15 CLI output shows the
non-ELK default flowchart curve still matches `basis`; explicit `flowchart.curve=rounded` remains
supported and tested.

The workspace gate passed on Windows with `CARGO_PROFILE_TEST_DEBUG=0` and low build concurrency,
which avoids MSVC PDB limits in this repository's large test profile.
