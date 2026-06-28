---
type: "Session Handoff"
title: "Merman LSP capability boundary audit"
description: "Audit handoff after reconciling first-class LSP maturity claims with parser-backed family coverage."
timestamp: 2026-06-28T00:00:00Z
tags: ["merman", "lsp", "capability-matrix", "session-handoff", "audit"]
source_session: "019ef370-dae4-7382-b0df-bbdb9ebe2d1b"
git_branch: "feat/diagnostics-analysis-contract"
---

# Summary

Superseded update: a later 2026-06-28 closure pass promoted `Architecture`, `Block`, `C4`,
`GitGraph`, `Kanban`, `Radar`, `Treemap`, and the supported headless `ZenUML` subset into the
first-class matrix. This handoff now records the earlier audit snapshot only; use
`docs/lsp/CAPABILITIES.md` and `docs/knowledge/engineering/current-state.md` for the current
boundary.

At the time of this snapshot, the LSP maturity boundary became explicit: the first-class matrix in
`docs/lsp/CAPABILITIES.md` tracked the families that already had parser-backed editor facts and
protocol coverage, while the then-out-of-matrix families were documented as `Needs refactor`,
`Partial`, or `Internal only`.

# Verified State

- `parse_pipeline.rs` routes `ishikawa` and `eventmodeling` through parser-backed editor-facts
  entry points.
- `ishikawa`, `treeView`, and `eventmodeling` already have parser-backed editor-facts tests.
- `docs/lsp/CAPABILITIES.md` now carries the mature first-class matrix plus an explicit coverage
  boundary for out-of-matrix families.
- `docs/lsp/README.md` and `docs/lsp/DIAGNOSTIC_PROTOCOL.md` now point readers at the matrix as
  the real maturity contract.
- `crates/merman-lsp/tests/capabilities.rs` now guards both the mature matrix rows and the
  boundary rows.

# Open Threads

- No current family-coverage closure thread remains for the supported set from this snapshot.
  `error` remains internal-only, and `ZenUML` remains scoped to the supported headless subset even
  though its LSP editor facts are now source-mapped.

# Next Action

Keep the capability matrix and the parser-backed family inventory in sync. Treat new supported
family regressions as failures against the closed current matrix rather than as a new broad
family-coverage planning pass.

# Citations

- `docs/lsp/CAPABILITIES.md`
- `docs/lsp/README.md`
- `docs/lsp/DIAGNOSTIC_PROTOCOL.md`
- `crates/merman-core/src/parse_pipeline.rs`
- `crates/merman-core/src/diagrams/ishikawa.rs`
- `crates/merman-core/src/diagrams/eventmodeling.rs`
- `crates/merman-core/src/diagrams/tree_view.rs`
- `crates/merman-lsp/tests/capabilities.rs`
