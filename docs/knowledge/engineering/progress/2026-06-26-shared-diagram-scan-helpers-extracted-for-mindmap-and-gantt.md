---
type: "Work Progress"
title: "Shared diagram scan helpers extracted for mindmap and gantt"
description: "Work Progress for Shared diagram scan helpers extracted for mindmap and gantt."
timestamp: 2026-06-26T13:42:59Z
tags: ["merman", "shared", "scan", "mindmap", "gantt", "refactor"]
source_session: "local"
---

# Summary
Extracted the duplicated low-level scan helpers used by `mindmap` and `gantt` into `crates/merman-core/src/diagrams/scan.rs`, so both families now share line-ending stripping, case-insensitive prefix checks, and indent splitting.

# Details
This refactor deliberately stayed below family semantics. It only consolidated reusable scanner primitives so the parser-backed facts seam stays cleaner for the next round of deeper parsing work.
`mindmap` and `gantt` are both wired to the shared helpers now. If either family keeps showing recovery gaps, span drift, or completion plumbing issues, the same seam is available for further fearless refactors.

# Next Action
Continue pressure-testing the `mindmap` / `gantt` parser-backed facts path and look for the next place where a shared parse model or stricter syntax structure would reduce duplication or improve correctness.

# Citations
- [scan.rs](../../../../crates/merman-core/src/diagrams/scan.rs)
- [mindmap parse](../../../../crates/merman-core/src/diagrams/mindmap/parse.rs)
- [gantt parse](../../../../crates/merman-core/src/diagrams/gantt/parse.rs)
- `cargo test -p merman-core mindmap_editor_facts -- --nocapture`
- `cargo test -p merman-core gantt_editor_facts -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
