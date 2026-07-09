---
type: "Work Log"
title: "U2 detector/admission/capability slice complete"
description: "Mermaid 11.16 detector, capability, and admission visibility verified; LSP parser constraint recorded for U3."
timestamp: 2026-07-09T11:55:18Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Summary

U2 made newly pinned Mermaid 11.16 families visible without falsely admitting parser/render support.
Detector facts now include `swimlane`, `railroad`, `railroadEbnf`, `railroadAbnf`, `railroadPeg`,
`wardley`, and `cynefin` in upstream registration order. `DiagramFamilyCapability` is detector-first:
detector-only families appear with `has_semantic_parser=false` and `has_render_parser=false`.

Admission now treats these families as pinned-baseline `NotAdmitted`, not `NotInPinnedBaseline`.
Current-facing alignment docs report Mermaid `@11.16.0` and no longer describe railroad or cynefin as
absent from the pinned source.

# Verification

- `cargo fmt`
- `cargo nextest run -p merman-core registry detect --no-fail-fast` passed: 41/41.
- `cargo nextest run -p xtask admission --no-fail-fast` passed: 11/11.
- `cargo run -p xtask -- check-alignment` passed.
- `git diff --check` passed.

# Source Notes

`mermaid@11.16.0` is an annotated tag. The tag object is
`5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`; the peeled source commit is
`7c0cafcf42e76bfaf79d0cbbd12edb986612f014`. `tools/upstreams/REPOS.lock.json` should store the
peeled source commit in its `commit` field.

Upstream `swimlanesDiagram.ts` reuses Flowchart via `createFlowDiagram({ defaultLayout:
'swimlane' })`. U3 should preserve this source shape and avoid forking a second swimlane parser.
Parser implementation choices must preserve LSP/editor value: spans, recovery, partial facts, and
completion context. LALRPOP is only a candidate where those properties can be proven, most likely
for railroad dialects rather than swimlane or small line-oriented grammars.
