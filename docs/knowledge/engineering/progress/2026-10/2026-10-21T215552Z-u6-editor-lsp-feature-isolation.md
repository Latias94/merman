---
type: Work Progress
title: Remove presentation dependencies from editor and LSP products
description: Keep standalone editor and LSP builds on canonical semantics and editor facts without requiring presentation or compatibility JSON.
timestamp: 2026-10-21T21:55:52Z
record_id: aa9970ea-8228-46f7-98cd-37a1c83d368f
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: c8ae8b2fb
---

# Product Feature Boundaries

`merman-editor-core` and `merman-lsp` now request only core `editor-facts` as
their normal direct core feature. Their parser-backed completion and language
capabilities no longer depend on presentation or legacy JSON. The completion
test exercises `SemanticParser` directly and verifies both a parsed outcome and
complete editor facts. The LSP capability test still requires semantic and
editor parsers, while no longer requiring the legacy JSON/editor combined
callback.

The extension protocol example reports `renderParser: false` for the standalone
lean LSP package. Its text clarifies that an embedding which also selects core
presentation can report `true`; the serialized field and protocol version are
unchanged. The protocol test compares render availability against the core
capability catalog rather than assuming every language parser is a render parser.

# Independent Consumer Check

An external temporary Cargo workspace depends on both products with default
features disabled and all diagram selectors enabled. Offline Cargo metadata
resolved all 34 diagram selectors and `editor-facts` for core, with
`presentation`, `compat-json`, and `test-support` absent. Analysis also resolved
without `compat-json`. `cargo check --locked --offline` succeeded for that
consumer. This verifies the ordinary dependency closure outside the repository
workspace; the temporary consumer is stored under `D:/merman-u5-temp`.

# Verification

All 465 editor-core and LSP lib, binary, and integration tests passed with
`cargo nextest run --locked -p merman-editor-core -p merman-lsp --all-features
--lib --tests --test-threads 2 --no-fail-fast`. The full workspace format check,
`git diff --check`, and strict Clippy over all editor/LSP targets passed.

The lean dependency closure still emits 14 existing `merman-core` compiler
warnings (mostly feature-dependent dead code, plus unused-mut/assignment). A
whole-product `-D warnings` invocation therefore stops in that dependency. They
remain open core feature-boundary cleanup; this change does not hide them with
crate-wide lint allowances. A strict lint over the editor/LSP targets succeeds,
and a full-feature core/analysis strict lint passed in the preceding U6 unit.

# Remote Main

Fetched `origin/main` at `71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b`. It is an
ancestor of the working branch, which was 53 commits ahead and zero behind
before this unit's commit. No new main commit required another merge, and no
remote changes affected this feature-boundary unit.

# Remaining Work

This completes only the editor/LSP dependency-edge unit. Diagnostics-only
recovery capture, ASCII and facade consumers, final package-feature naming and
alignment, and final size/performance evidence remain for the active plan.
