---
type: "Work Progress"
title: "2026-06-24 LSP completion seam refinement"
description: "Work Progress for 2026-06-24 LSP completion seam refinement."
timestamp: 2026-06-24T02:53:06Z
tags: ["merman-lsp", "lsp", "completion", "diagnostics", "markdown"]
source_session: "019ef370-dae4-7382-b0df-bbdb9ebe2d1b"
---

# Summary

Completed the follow-up seam refinement for `merman-lsp`: diagnostics now use the shared Markdown URI helper, completion covers headers, directions, operators, directives, shapes, and node IDs, and the server smoke test verifies current-version diagnostics through the public `tower-lsp` request path.

# Details

- `merman-analysis` now owns the shared LSP URI helper for Markdown detection.
- `merman-lsp` completion logic is driven by snapshot/context helpers instead of scattered string checks.
- `server` now reuses the shared Markdown helper rather than guessing extensions locally.
- `completion`, `document_store`, and `server_smoke` tests cover plain `.mmd` and Markdown fence paths.

# Next Action

Decide whether the next slice should be lint plumbing, richer completion metadata, or a deeper LSP snapshot seam for hover/symbol work.

# Citations

- [LSP completion foundations plan](../../../plans/2026-06-24-001-feat-lsp-completion-foundations-plan.md)
- [merman-lsp crate](../../../../crates/merman-lsp/src/server.rs)
- [analysis LSP helpers](../../../../crates/merman-analysis/src/lsp.rs)
