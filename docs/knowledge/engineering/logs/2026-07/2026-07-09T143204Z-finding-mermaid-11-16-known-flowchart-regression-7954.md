---
type: "Memory Event"
title: "Finding: Mermaid 11.16 known Flowchart regression #7954"
description: "Mermaid issue #7954 reports a 11.16.0 Flowchart subgraph layout regression versus 11.15.0; baseline refresh should classify it as upstream-known rather than local drift."
timestamp: 2026-07-09T14:32:04Z
event_kind: "Finding"
---
# Event

Mermaid issue #7954 reports a Flowchart layout regression introduced in Mermaid 11.16.0: an edge
between elements inside separate subgraphs can produce incorrect placement, while the same diagram
is reported to render correctly in 11.15.0. During 11.16 golden refresh, do not use this issue to
justify local ad hoc layout tuning. Treat imported examples as upstream-known-regression fixtures
unless a source-backed Merman workaround is deliberately chosen.

# Impact

Golden refresh should separate three buckets: expected 11.16 source parity changes, local Merman
regressions, and pinned-upstream regressions. Issue #7954 belongs to the third bucket until upstream
Mermaid fixes or changes it.

# Citations

- https://github.com/mermaid-js/mermaid/issues/7954
- `docs/alignment/MERMAID_11_16_KNOWN_UPSTREAM_REGRESSIONS.md`
