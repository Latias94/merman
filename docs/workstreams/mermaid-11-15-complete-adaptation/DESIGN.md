# Mermaid 11.15 Complete Adaptation

Status: Active
Last updated: 2026-05-31

## Why This Lane Exists

`merman` now documents Mermaid `11.15.0` as the baseline for the implemented diagram matrix, and
the targeted 11.13-11.15 compatibility slices have landed. That is not the same as full Mermaid
11.15 adaptation. Current evidence still shows stale 11.12-era upstream SVG baselines, red full SVG
DOM parity, and several upstream diagram families without local parser/model/layout/render support.

This lane is the umbrella campaign for closing that gap without turning every new diagram family
into one unreviewable task.

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
  - `docs/alignment/PARITY_HARDENING_PLAN.md`
  - `docs/rendering/UPSTREAM_SVG_BASELINES.md`
  - `tools/upstreams/REPOS.lock.json`
- Related workstreams:
  - `docs/workstreams/mermaid-11-15-baseline-upgrade`
  - `docs/workstreams/generated-default-config-parity`
  - `docs/workstreams/pie-11-15-parity`

## Problem

The baseline has moved to Mermaid `11.15.0`, but the repo still mixes three states:

- code and generated defaults that target selected 11.15 behavior,
- upstream SVG baselines and compare reports that still carry 11.12.3 assumptions,
- and upstream diagram families that are deferred or out of scope rather than implemented.

That makes "11.15 complete" ambiguous and blocks a defensible release claim.

## Target State

This lane closes when:

- implemented diagram matrix SVG baselines are proven against Mermaid `11.15.0`,
- full implemented-matrix DOM parity is green in `parity` mode,
- `parity-root` status is either green or explicitly split with current evidence,
- generated artifacts and alignment checks are green,
- stale 11.12.3 references in active 11.15 parity docs/tool reports are cleaned up or marked
  historical,
- and every upstream 11.15 diagram family outside the implemented matrix has an explicit
  implement/defer/out-of-scope decision or a child workstream.

## In Scope

- Audit and regenerate/check upstream SVG baselines for Mermaid `11.15.0`.
- Update compare report metadata and active parity docs that still imply 11.12.3.
- Triage current `compare-all-svgs --dom-mode parity` failures into stale-baseline drift versus
  real renderer/parser gaps.
- Fix or split residual existing-matrix parity gaps after 11.15 baselines are authoritative.
- Run and record full implemented-matrix parity gates.
- Maintain the decision table for unsupported upstream diagram families.
- Open child workstreams when a diagram family or renderer gap becomes a durable lane.

## Out Of Scope

- Implementing all deferred diagram families directly inside this umbrella lane.
- Broad renderer rewrites that are not needed to close a measured 11.15 parity gap.
- Claiming browser-plugin parity for ZenUML beyond the existing headless compatibility mode.
- Changing release/package versioning unless a follow-on release lane requests it.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Local upstream Mermaid is pinned to `11.15.0`. | High | `tools/upstreams/REPOS.lock.json`; `repo-ref/mermaid/packages/mermaid/package.json` | Baseline regeneration must re-pin before any parity claim. |
| The current full SVG parity failure is dominated by stale upstream SVG baselines, not only renderer bugs. | High | `compare-all-svgs` reports still say Mermaid 11.12.3; marker ID mismatches show old bare IDs upstream versus local prefixed IDs. | If false, M15C-040 expands into renderer fixes rather than baseline refresh. |
| New upstream diagram families should be split into child lanes. | High | Seven upstream directories lack local parser/model/layout/render support. | If all must be implemented in this lane, the workstream becomes too broad to review safely. |
| `parity` should be made authoritative before `parity-root`. | High | Root viewport diffs are noisier and depend on browser bbox behavior. | Running root-first can hide structural mismatches under viewport noise. |

## Architecture Direction

Use this workstream as a campaign ledger, not as a monolithic implementation bucket. Baseline
generation, compare tooling, and existing renderer fixes can be done here while they stay tightly
scoped. New diagram families and large renderer convergence work should be promoted into child
workstreams with their own design, task ledger, and evidence.

The parity claim should remain explicit:

- "Mermaid 11.15 for the implemented matrix" is achievable inside this lane.
- "Full upstream Mermaid 11.15 family parity" requires child family lanes and is not implied until
  those lanes close.

## Closeout Condition

This lane can close when:

- Mermaid 11.15 upstream SVG baselines are authoritative for the implemented matrix,
- `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3` is green
  or any non-green item is split with an accepted non-goal,
- `parity-root` is green or documented as a separate follow-on with fresh evidence,
- `cargo run -p xtask -- check-alignment` and `cargo run -p xtask -- verify-generated` are green,
- relevant package/workspace tests have fresh evidence,
- deferred/out-of-scope diagram family decisions are recorded in `docs/alignment/STATUS.md`,
- and all remaining work is either complete, split, or explicitly deferred.
