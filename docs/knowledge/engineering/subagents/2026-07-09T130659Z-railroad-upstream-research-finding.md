---
type: "Subagent Finding"
title: "Railroad upstream parser research"
description: "Subagent research confirmed the railroad implementation should remain hand-written for the LSP/editor contract."
timestamp: 2026-07-09T13:06:59Z
producer_id: "codex-root"
subagent_id: "/root/railroad_upstream_research"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
git_commit: "75a4beba5"
---

# Finding

The railroad family has four Mermaid 11.16 detector/header pairs:
`railroad-beta` -> `railroad`, `railroad-ebnf-beta` -> `railroadEbnf`,
`railroad-abnf-beta` -> `railroadAbnf`, and `railroad-peg-beta` -> `railroadPeg`.
All four dialects map naturally into the shared upstream-shaped AST node set:
`terminal`, `nonterminal`, `sequence`, `choice`, `optional`, `repetition`, and `special`.

# Recommendation

Do not switch this slice to LALRPOP now. A hand-written lexer plus recursive descent parser gives
direct control over token spans, partial recovery, expected syntax, rule/nonterminal references,
and lossy editor facts after parse errors. LALRPOP remains a future spike only if it proves no
regression in span plumbing or recovery quality.

# Disposition

Adopted in commit `75a4beba5 feat: parse railroad semantics`. Render/layout admission remains
deferred until typed renderer support and SVG fixtures are ported.
