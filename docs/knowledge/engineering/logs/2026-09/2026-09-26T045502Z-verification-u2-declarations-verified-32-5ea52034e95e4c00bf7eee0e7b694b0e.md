---
type: "Memory Event"
title: "Verification: U2 declarations verified: 32 catalog-owned selectors forwarded through 16 owning"
description: "U2 declarations verified: 32 catalog-owned selectors forwarded through 16 owning/adapting crates; low-level defaults remain empty, facade/CL"
timestamp: 2026-09-26T04:55:02Z
record_id: "5ea52034e95e4c00bf7eee0e7b694b0e"
producer_id: "codex-selectable-diagrams"
event_kind: "Verification"
---

# Event

U2 declarations verified: 32 catalog-owned selectors forwarded through 16 owning/adapting crates; low-level defaults remain empty, facade/CLI/rustdoc defaults request all-diagrams, xtask/fuzz roots opt in explicitly. Five isolated active dependency graphs in target/diagram-feature-graphs passed cargo tree --offline checks: no-family core, Gantt core, language-only facade (no renderer), Flowchart+Gantt SVG, and widened core with Flowchart-only renderer. Unfiltered cargo metadata includes optional inactive nodes, so active cargo tree edges are the graph evidence. cargo nextest run -p merman-core --lib --features all-diagrams -E "test(family::)" --build-jobs 2 --test-threads 2 passed 8 tests, run ddaaf82c-4cca-47a6-b934-8ebbd214874c. U3 proof-first: new diagram_features integration test with no default features failed because supported_diagrams still includes Sequence (3 other tests passed), run 03e674a8-4f46-4015-a61f-2d8b23c10fe0. This failure is expected until implementation gates land; test remains outside the U2 commit. U3-U8 remain pending.

# Impact

# Citations
