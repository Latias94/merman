---
type: "Memory Event"
title: "Verification: U4 semantic/layout golden refresh"
description: "Refreshed TreeView semantic/layout goldens plus stale Architecture and Flowchart semantic goldens after U4 11.16 model changes; core snapshot gate now passes."
timestamp: 2026-07-09T14:42:03Z
event_kind: "Verification"
---
# Event

U4 snapshot refresh: TreeView semantic/layout goldens were updated for the 11.16 node model and
layout fields. The wider `merman-core` snapshot gate then exposed stale Architecture semantic
goldens (`layoutHints: []`) and Flowchart semantic goldens (including subgraph `direction TD` and
earlier 11.16 semantic changes), so those family-local semantic snapshots were refreshed as well.

# Verification

- `cargo run -p xtask -- update-snapshots --diagram treeView`
- `cargo run -p xtask -- update-layout-snapshots --diagram treeView`
- `cargo run -p xtask -- update-snapshots --diagram architecture`
- `cargo run -p xtask -- update-snapshots --diagram flowchart`
- `cargo nextest run -p merman-core fixtures_match_golden_snapshots --no-fail-fast`

# Impact

The core semantic snapshot gate is green after the U4 model changes. TreeView upstream SVG baseline
refresh remains pending because local Puppeteer Chrome is missing; that belongs to U7/browser-tooling
setup rather than semantic snapshot correctness.

# Citations

- `fixtures/treeView/*.golden.json`
- `fixtures/treeView/*.layout.golden.json`
- `fixtures/architecture/*.golden.json`
- `fixtures/flowchart/*.golden.json`
