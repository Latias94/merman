---
type: "Work Progress"
title: "Deprecated flowchart htmlLabels quickfix"
description: "Work Progress for Deprecated flowchart htmlLabels quickfix."
timestamp: 2026-06-26T12:29:20Z
tags: ["merman", "lsp", "lint", "quickfix", "analysis"]
source_session: "current-turn"
---

# Summary

`merman.compatibility.config.deprecated_flowchart_html_labels` now projects a preferred quickfix through the analysis payload, CLI-facing diagnostics, and LSP code actions. The fix promotes deprecated `flowchart.htmlLabels` into root-level `htmlLabels` and reuses the shared config/frontmatter rewrite path so the config migration surface stays aligned across analysis, LSP, CLI, and the rule catalog.

# Details

- The analysis rule now attaches `DiagnosticFix` metadata for deprecated `flowchart.htmlLabels` spans.
- The rewrite helper normalizes merged Mermaid config before emitting the frontmatter migration edit.
- The LSP layer projects the fix into standard `textDocument/codeAction` quickfixes.
- The public rule catalog and LSP extension protocol now advertise the rule as fixable.
- Documentation and smoke tests were updated to reflect the new quickfix behavior.

# Next Action

Continue U4 by adding more Mermaid-backed config/compatibility rules with source-span-backed fixes where the rewrite is structurally safe.

# Citations

- [analysis rule](../../../../crates/merman-analysis/src/rules.rs)
- [rewrite helper](../../../../crates/merman-analysis/src/source_config_rewrite.rs)
- [LSP code actions](../../../../crates/merman-lsp/src/code_actions.rs)
- [LSP protocol tests](../../../../crates/merman-lsp/src/protocol.rs)
- [server smoke test](../../../../crates/merman-lsp/tests/server_smoke.rs)
- [extension protocol](../../../../docs/lsp/EXTENSION_PROTOCOL.md)
