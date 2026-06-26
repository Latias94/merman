---
type: "Work Progress"
title: "Mindmap editor facts now preserve prior symbols on later invalid node input"
description: "Work Progress for Mindmap editor facts now preserve prior symbols on later invalid node input."
timestamp: 2026-06-26T13:10:05Z
tags: ["merman", "mindmap", "gantt", "lsp", "refactor"]
source_session: "local"
---

# Summary
Closed the first `mindmap` recovery gap in the parser-backed editor facts seam.

# Details
Later invalid node lines no longer clear earlier symbols in recovery mode, so LSP/editor consumers keep prior outline/completion material even when a later line is malformed. `gantt` was rechecked on the same slice and did not need a similar recovery fix.

# Next Action
Continue the mature roadmap by tightening family capability gates and then deepening the shared `mindmap` / `gantt` parser-backed fact seam first.

# Citations
