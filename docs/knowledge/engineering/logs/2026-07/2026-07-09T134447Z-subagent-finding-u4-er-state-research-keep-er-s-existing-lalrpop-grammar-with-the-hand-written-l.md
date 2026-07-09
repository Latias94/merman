---
type: "Memory Event"
title: "Subagent Finding: U4 ER/State research: keep ER's existing LALRPOP grammar with the hand-written l"
description: "U4 ER/State research: keep ER's existing LALRPOP grammar with the hand-written lexer, adding lexer support for question-mark nullable attrib"
timestamp: 2026-07-09T13:44:47Z
event_kind: "Subagent Finding"
---
# Event

U4 ER/State research: keep ER's existing LALRPOP grammar with the hand-written lexer, adding lexer support for question-mark nullable attributes plus backtick/comma attribute tokens instead of replacing parser architecture. For State, keep LALRPOP unchanged and add lexer-level rejection for same-line composite state names containing multiple whitespace-separated words before '{'. This preserves LSP spans/recovery while matching Mermaid 11.16 parser behavior.

# Impact

# Citations
