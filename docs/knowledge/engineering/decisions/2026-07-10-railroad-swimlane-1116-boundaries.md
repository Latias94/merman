---
type: Decision
title: Railroad and swimlane 11.16 support boundaries
timestamp: 2026-07-10T00:45:19+08:00
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,railroad,swimlane,flowchart
---

# Decision

Admit the Railroad variants as `CompatibilityOnly` after adding typed parser projection, layout
goldens, and a local SVG renderer. Keep Swimlane parse-only until the source-backed swimlane layout
path is implemented.

# Context

Mermaid `@11.16.0` added `railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`,
`railroad-peg-beta`, and `swimlane-beta`. Railroad has a self-contained recursive SVG renderer, so
it is practical to port as a compatibility renderer before upstream SVG baselines exist. Swimlane is
implemented upstream through Flowchart semantics plus dedicated swimlane layout utilities; rendering
it through ordinary local Flowchart/Dagre would be misleading.

Mermaid issue https://github.com/mermaid-js/mermaid/issues/7954 tracks a separate upstream 11.16.0
Flowchart regression where arrows between subgraphs are broken. That regression should be treated as
upstream-known behavior during 11.16 fixture triage.

# Alternatives Considered

- Keep Railroad parse-only until U7 SVG baselines: rejected because the local source-backed renderer
  is now small enough and covered by layout/SVG smoke tests.
- Promote Swimlane by reusing ordinary Flowchart render output: rejected because it would erase the
  upstream swimlane layout semantics and hide missing work.
- Locally fix Flowchart subgraph arrows relative to 11.15 behavior: rejected for this migration
  slice because #7954 is an upstream 11.16.0 regression boundary, not a local parity miss.

# Consequences

- Railroad can be used in local headless rendering and registry metadata, but it is not part of the
  primary SVG comparison matrix until upstream SVG baselines and a family compare command exist.
- Swimlane remains explicit parse-only evidence, avoiding accidental unsupported rendering.
- Future Flowchart fixture import should annotate #7954-affected cases instead of adding brittle
  comparator normalization.
