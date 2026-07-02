---
type: "Progress"
title: "Parser-backed capability matrix gates for new families"
description: "Audit of first-class vs partial Mermaid families for LSP maturity."
timestamp: 2026-06-28T03:30:13Z
tags: ["merman", "lsp", "parser-backed", "capability-matrix"]
source_session: "continuation"
---

# Summary

The shared LSP transport is already mature enough to be honest about family coverage. The remaining
work is no longer transport plumbing; it is filling the last family-level semantic gaps and keeping
the documentation boundary explicit.

Update: the later 2026-06-28 closure pass promoted `Block`, `C4`, and `ZenUML` into the
first-class matrix. This note now records the gate policy and the superseded audit snapshot, not a
current gap list.

# Coverage Snapshot

| Bucket | Families | Status |
| --- | --- | --- |
| First-class LSP | Flowchart, Sequence, State, Class, ER, Mindmap, Gantt, Architecture, GitGraph, Kanban, Radar, Treemap, Block, C4, ZenUML, Ishikawa, Journey, Info, Timeline, Pie, Packet, Sankey, Tree View, Event Modeling, Quadrant Chart, Requirement, Venn, XY Chart | Parser-backed or source-mapped editor facts, capability tests, and protocol coverage exist. |
| Superseded closure cluster | Block, C4, ZenUML | These were the remaining deep-refactor targets in the original audit snapshot; they are now promoted. |
| Internal only | Error | Fallback diagram only; not a product-family commitment. |

# Details

- `docs/lsp/CAPABILITIES.md` was missing Ishikawa even though `parse_pipeline.rs` already routes it
  and `crates/merman-core/src/diagrams/ishikawa.rs` has parser-backed editor facts. That omission
  was an underclaim, not a real implementation gap.
- The capability matrix should stay narrower than `supported_diagrams`. The latter is a broader
  render/projection metadata list, while the matrix is the mature LSP contract.
- Payload-first families (`Info`, `Pie`, `Packet`, and `XY Chart`) are mature enough for
  completion, hover, diagnostics, and semantic indexing, but they intentionally expose fewer
  rename/reference targets because they have few entity-bearing spans.

# Next Action

- Keep the first-class matrix in sync with `parse_pipeline.rs` and the LSP protocol tests.
- Treat new gaps as regressions against the closed current supported set, not as a second broad
  family-coverage plan.

# Citations

- `crates/merman-core/src/family.rs`
- `crates/merman-core/src/parse_pipeline.rs`
- `crates/merman-analysis/src/editor.rs`
- `crates/merman-lsp/src/structure.rs`
- `crates/merman-lsp/tests/capabilities.rs`
