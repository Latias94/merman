---
type: "Verification Evidence"
title: "merman-lsp semantic tokens delta contract"
description: "Verification Evidence for merman-lsp semantic tokens delta contract."
timestamp: 2026-06-24T22:59:02Z
tags: ["merman-lsp", "semantic-tokens", "lsp", "verification"]
---

# Verification

Verified the semantic-token delta contract with focused formatting and LSP tests after adding the
cached result-id path.

# Result

Passed.

# Evidence

- `cargo fmt --all --check`
- `cargo test -p merman-lsp semantic_tokens::tests::semantic_tokens_delta_result_prefers_edits_over_full_tokens -- --nocapture`
- `cargo test -p merman-lsp server::tests::capabilities_advertise_completion_and_full_sync -- --nocapture`
- `cargo test -p merman-lsp server::tests::lsp_handlers_return_hover_and_symbols -- --nocapture`
- `cargo test -p merman-lsp lsp_service_smoke_serves_semantic_tokens_delta -- --nocapture`

# Follow-up

Full package sweeps remain useful before release, but the changed semantic-token surfaces are
covered by focused unit, handler, and smoke tests.

# Citations

- [Semantic token implementation](../../../../crates/merman-lsp/src/semantic_tokens.rs)
- [LSP server implementation](../../../../crates/merman-lsp/src/server.rs)
- [Semantic token delta smoke test](../../../../crates/merman-lsp/tests/server_smoke.rs)
