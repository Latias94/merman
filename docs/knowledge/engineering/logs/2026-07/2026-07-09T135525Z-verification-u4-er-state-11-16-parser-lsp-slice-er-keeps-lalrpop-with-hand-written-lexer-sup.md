---
type: "Memory Event"
title: "Verification: U4 ER/State 11.16 parser/LSP slice: ER keeps LALRPOP with hand-written lexer sup"
description: "U4 ER/State 11.16 parser/LSP slice: ER keeps LALRPOP with hand-written lexer support for comma/dot attribute types, backtick-escaped type/na"
timestamp: 2026-07-09T13:55:25Z
event_kind: "Verification"
---
# Event

U4 ER/State 11.16 parser/LSP slice: ER keeps LALRPOP with hand-written lexer support for comma/dot attribute types, backtick-escaped type/name spans, and nullable '?' type suffix; State keeps LALRPOP and adds lexer-level exact diagnostic for same-line multi-word composite state names before '{'. Verified: cargo nextest run -p merman-core mermaid_11_16_attribute --no-fail-fast; cargo nextest run -p merman-core state --no-fail-fast; cargo nextest run -p merman-core er --no-fail-fast; git diff --check.

# Impact

# Citations
