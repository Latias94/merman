# ASCII Flowchart Subgraph Title Wrapping — Milestones

Status: Closed
Last updated: 2026-05-30

## Exit Criteria

- Long flowchart subgraph titles wrap automatically instead of widening the group indefinitely.
- Explicit `<br>` and escaped-newline subgraph titles still render as intentional hard breaks.
- Existing subgraph fixtures remain stable unless a wrap change is the intended delta.
- Support docs describe the shipped wrapping boundary and any remaining follow-on.

## Milestones

- M0: DONE. Workstream scoped and evidence anchors frozen.
- M1: DONE. A red wrap contract captured the raw one-line title expansion gap.
- M2: DONE. Wrapped subgraph titles render green and docs reflect the new behavior.
- M3: DONE. Fresh verification supports closeout.
