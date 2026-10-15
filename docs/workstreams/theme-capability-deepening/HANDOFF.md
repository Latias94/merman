# Theme Capability Deepening - Handoff

Status: Closed
Last updated: 2026-08-25

> Historical closeout note (2026-08-25): this handoff records the June 2026 lane closure. The
> later portable-theme convergence work has already superseded its pending-commit recommendation;
> current status and follow-on gates live in
> [`docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md`](../../plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md).

## Current State

This lane is the explicit follow-on to the completed `theme-parity` workstream.

What is frozen:

- the problem is no longer Mermaid 11.15 theme-name or preset parity;
- the active issue is render-side theme architecture depth and capability locality;
- the lane is anchored to `ARCH-013` plus ADR-0068;
- host styling remains outside the default renderer boundary.

## Source Coverage

| Source | State | Evidence | Impact | Action |
| --- | --- | --- | --- | --- |
| User goal and autonomy request | COVERED | active Codex goal + current lane docs | execution is explicitly authorized | proceed |
| `AGENTS.md` repo rules | COVERED | repo root `AGENTS.md` | June closeout followed the then-current confirmation rule | preserved as historical evidence |
| Root `CONTEXT.md` | DEFERRED | `ARCH-001` records it as missing | not blocking this bounded theme lane | continue with ADRs/workstreams as context |
| Theme parity prior lane | COVERED | `docs/workstreams/theme-parity/*` | provides split-follow-up boundary | treat as parent lane |
| Theme/render architecture audit | COVERED | `ARCH-013` in `docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md` | justifies the deeper render-side seam | implement |
| Host styling boundary | COVERED | ADR-0063 + ADR-0064 | prevents theme-capability work from leaking into product CSS policy | preserve |
| Upstream Mermaid theme/config boundary | COVERED | `repo-ref/mermaid` references in CONTEXT.jsonl | keeps core compatibility boundary stable | preserve |
| `beautiful-mermaid` reference | COVERED | `repo-ref/beautiful-mermaid` references in CONTEXT.jsonl | informs render-side semantic role placement | adapt, do not copy wholesale |

## Active Task

- Task ID: none
- Owner: none
- Files: none
- Validation: closeout gate recorded in `EVIDENCE_AND_GATES.md`
- Status: CLOSED
- Review: lane closed without claiming full theme-system completion.
- Evidence: `docs/workstreams/theme-capability-deepening/EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- Opened a new lane instead of reviving `theme-parity`, because that workstream already closed with
  `split-follow-up`.
- Chose `theme-capability-deepening` over a parity-themed slug to make the goal explicit.
- Added ADR-0068 so the new render-side theme seam is documented before broader migration.
- Completed TCD-020 by introducing `PresentationTheme` for the first high-duplication SVG/CSS
  consumers.
- Completed TCD-030 by moving XyChart plot palette resolution into a renderer-owned
  `chart_palette` helper while leaving core `themeVariables` untouched.
- Completed TCD-040 by verifying the existing HPD-080 public renderability smoke through the real
  integration-test command. No new public fixtures were added because existing coverage already
  proves the relevant visible paths.
- Completed TCD-050 by reviewing the lane, correcting stale gate commands, running fresh closeout
  verification, and recording closeout/follow-on boundaries.

## Blockers

- None recorded for this closed lane. The June closeout's commit-confirmation note is historical;
  it is not an active instruction for the later convergence work.

## Historical Next Action

The June handoff recommended committing the closed lane after maintainer confirmation. That action
has since been superseded by later theme-architecture commits. Do not reopen this lane; open a
narrower follow-on only for a specific remaining raw-theme family, binding/playground surface, or
host styling policy.
