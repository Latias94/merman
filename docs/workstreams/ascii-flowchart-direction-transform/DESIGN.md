# ASCII Flowchart Direction Transform

Status: Closed
Last updated: 2026-05-30

## Closeout

Closed on 2026-05-30. BT and RL root directions render through public parser-backed ASCII tests,
support docs describe the shipped direction subset, and remaining flowchart work is explicitly
deferred to narrower follow-ons.

## Why This Lane Exists

`merman-ascii` supported flowchart LR and TD/TB-alias output, but rejected BT and RL root directions
with an explicit unsupported-feature diagnostic. The flowchart support matrix already called out
those directions as the next focused compatibility gap.

## Relevant Authority

- ADRs:
  - `docs/adr/0065-ascii-output-boundary.md`
  - `docs/adr/0014-upstream-parity-policy.md`
- Existing docs:
  - `crates/merman-ascii/FLOWCHART_SUPPORT.md`
  - `docs/workstreams/ascii-reference-implementation-expansion/`
- Related workstreams:
  - `docs/workstreams/flowchart-text-style-parity/`

## Problem

The pre-lane flowchart renderer refused root directions outside LR and TD/TB alias. For terminal
output to remain honest, BT and RL needed a real root-direction transform rather than a silent
approximation or parser-level special case.

## Target State

- Parser-backed flowchart tests cover BT and RL root directions.
- BT renders as a true vertical inversion of the TD layout, including edge endpoints and arrow
  orientation.
- RL renders as a true horizontal inversion of the LR layout, including edge endpoints and arrow
  orientation.
- Existing LR and TD output stays stable.
- Subgraph direction overrides, color/style roles, and state diagram rendering remain separate
  follow-ons.

## In Scope

- Flowchart parser-backed BT and RL regression tests.
- Root-direction transform logic in the flowchart ASCII renderer.
- Arrowhead, corner, and label remapping needed for BT/RL honesty.
- Support-doc updates and closeout evidence.

## Out Of Scope

- Subgraph direction overrides.
- ANSI/HTML color roles.
- `classDef`, `class`, and inline node styles.
- State diagram graph rendering.
- New uncommon flowchart shapes.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The typed flowchart model preserves the requested root direction. | High | Existing LR/TD support and `FLOWCHART_SUPPORT.md` already distinguish direction from layout. | Direction support may need a parser or model change. |
| BT can be implemented as a post-layout vertical transform instead of a brand-new layout engine. | Medium | Existing graph layout already computes a stable terminal canvas for LR/TD. | The lane may need to split into layout and drawing sub-lanes. |
| RL can be implemented as a horizontal mirror of the LR flowchart rendering path. | Medium | The support doc already frames RL as a true inversion, not an approximation. | The lane may need deeper route/corner remapping than expected. |

## Architecture Direction

Keep the root-direction transform inside the ASCII flowchart rendering path, not in `merman-core`
or the parser. The shared graph/layout code should keep producing terminal-friendly geometry, while
the direction-specific transform remaps the rendered output and arrow/corner orientation for BT/RL.

That keeps the contract clear:

- parser/model owns the direction request,
- graph layout owns the base LR/TD geometry,
- the ASCII flowchart adapter owns root-direction mirroring,
- and tests prove the public `render_model` output.

## Closeout Condition

This lane can close when:

- BT and RL render through public parser-backed tests,
- focused and package gates pass,
- support docs describe the shipped direction subset,
- and remaining flowchart work is either split or explicitly deferred.
