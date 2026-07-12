# Mermaid 11.16 Known Upstream Regressions

This file records pinned Mermaid 11.16.0 behaviors that should be treated as upstream-originated
when refreshing baselines or triaging compare failures. These are not excuses for local drift: each
entry needs source or issue evidence and should be revisited when the pinned Mermaid baseline moves.

## Flowchart Subgraph Layout Regression

- Upstream issue: <https://github.com/mermaid-js/mermaid/issues/7954>
- Opened: 2026-07-08
- Reported scope: Mermaid 11.16.0 Flowchart layout regression where an edge between elements inside
  different subgraphs can produce incorrect placement; the reporter states the same example works in
  11.15.0 and regresses in 11.16.0.
- Local policy: do not add broad layout magic numbers or comparator normalization to hide this class
  of upstream bug. If the fixture is imported, tag it as an upstream-known-regression case and keep
  the expected behavior tied to the pinned 11.16.0 baseline until Mermaid fixes it or Merman adopts a
  source-backed workaround with an explicit decision record.
