---
type: "Work Progress"
title: "merman-lsp semantic tokens delta implementation"
description: "Work Progress for merman-lsp semantic tokens delta implementation."
timestamp: 2026-06-24T22:59:02Z
tags: ["merman-lsp", "semantic-tokens", "lsp", "progress"]
source_session: "local"
---

# Summary

`merman-lsp` now serves semantic tokens with cached full/delta state instead of full-only
responses. The server advertises delta capability, records previous token snapshots by URI, and
reuses the prior result id when the client asks for `textDocument/semanticTokens/full/delta`.

# Details

- Added `SemanticTokensState` to `DocumentStore` so the last full token response can be reused for
  delta requests.
- Added result-id generation and edit computation helpers in `semantic_tokens.rs`.
- Changed `semantic_tokens/full` to emit a stable result id and cache the returned token data.
- Implemented `semantic_tokens/full/delta` with full fallback when the previous result id does not
  match the cached snapshot state.
- Updated the LSP capability to advertise `SemanticTokensFullOptions::Delta { delta: Some(true) }`.
- Added a smoke test that opens a document, fetches full semantic tokens, edits the document, and
  then requests delta tokens with the previous result id.

# Next Action

Keep delta support conservative for now, then continue the broader mature LSP roadmap work on lint
rules and remaining capability gaps.

# Citations

- [Document store](../../../../crates/merman-lsp/src/document_store.rs)
- [Semantic tokens](../../../../crates/merman-lsp/src/semantic_tokens.rs)
- [LSP server](../../../../crates/merman-lsp/src/server.rs)
- [Smoke tests](../../../../crates/merman-lsp/tests/server_smoke.rs)
