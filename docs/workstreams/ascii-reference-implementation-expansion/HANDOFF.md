# ASCII Reference Implementation Expansion — Handoff

Status: Active
Last updated: 2026-05-29

## Current State

This lane is opened to govern how `merman-ascii` should learn from `repo-ref/mermaid-ascii` and
`repo-ref/beautiful-mermaid`. The first task records reference commits, license notices, README
links, and the model-driven boundary.

No runtime implementation has been changed yet.

## Active Task

- Task ID: ARI-010
- Owner: planner
- Files:
  - `README.md`
  - `crates/merman-ascii/README.md`
  - `crates/merman-ascii/LICENSES/beautiful-mermaid-MIT.txt`
  - `tools/upstreams/REPOS.lock.json`
  - `docs/workstreams/ascii-reference-implementation-expansion/*`
- Validation: `git diff --check`
- Status: DONE
- Review: pending broader workstream review before implementation tasks
- Evidence: `EVIDENCE_AND_GATES.md`

## Decisions Since Last Update

- `beautiful-mermaid` is a reference implementation, not a spec.
- New ASCII diagram renderers must consume `merman-core` typed models.
- Do not port or duplicate `beautiful-mermaid`'s parser or SVG renderer into `merman-ascii`.
- Class, ER, and xychart are separate vertical slices.

## Blockers

- None for the documentation/provenance task.
- Implementation tasks need workers to inspect the exact typed model fields before coding.

## Next Recommended Action

Start ARI-020 as the first bounded implementation task: render the smallest useful classDiagram
ASCII slice from `RenderSemanticModel::Class`, with snapshots and unsupported-feature diagnostics.
