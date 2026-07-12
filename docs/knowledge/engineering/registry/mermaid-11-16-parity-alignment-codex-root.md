---
type: "Work Registration"
title: "Mermaid 11.16 parity alignment"
description: "Registration for Mermaid 11.16 parity alignment."
timestamp: 2026-07-09T11:13:56Z
status: "active"
last_seen: 2026-07-10T03:26:18+08:00
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
git_commit: "cca2ce09e562"
latest_link: "TreeView SVG DOM serialization now matches the Mermaid 11.16 baseline shape; main parity DOM comparison is green."
---

# Scope

Align Merman from mermaid@11.15.0 to mermaid@11.16.0 as a headless, source-backed Mermaid implementation. Scope includes baseline metadata, configuration and theme semantics, existing diagram family deltas, new 11.16 family admission, fixture/baseline refresh, SVG DOM parity, and cleanup of obsolete 11.15-only code.

# Current Claim

U1 baseline authority is committed. U2 detector/admission/capability work is verified locally:
11.16 detector ids are visible for swimlane, railroad variants, wardley, and cynefin; detector-only
families are explicit capability records; admission marks them pinned-baseline `NotAdmitted`, not
absent. U5 now includes compatibility-only renderers for Cynefin and Railroad, while Swimlane is
documented as parse-only until its dedicated upstream layout backend is ported. Current-facing docs
and code comments now advertise Mermaid `@11.16.0`; remaining `11.15.0` hits are historical or
generated-name provenance. TreeView SVG serialization now matches upstream 11.16 DOM ordering and
class surface under the main parity comparator. Mermaid `mermaid@11.16.0` is an annotated tag: tag object
`5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`, peeled source commit
`7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

# Latest Links

- Plan: `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
- Mermaid 11.16 tag object: `5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`
- Mermaid 11.16 peeled source commit: `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`
- Previous baseline commit: 41646dfd43ac83f001b03c70605feb036afae46d

# Handoff

Perform the final plan-level DoD audit before marking the Mermaid 11.16 goal complete. Parser design
must continue preserving LSP/editor value: source spans, recoverable diagnostics, partial AST/facts,
and completion-friendly statement context. Swimlane should remain parse-only until a source-backed
port of Mermaid's dedicated swimlane layout utilities exists. Mermaid issue #7954 is an open
11.16.0 upstream regression for arrows between subgraphs; treat it as pinned-baseline risk rather
than a semantic behavior to preserve. Broad root viewport residuals remain outside the main parity
DOM gate and need a source-backed root sizing decision before policy acceptance.

# Citations

- `repo-ref/mermaid` tag `mermaid@11.16.0`
- `repo-ref/mermaid` tag `mermaid@11.15.0`
- `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
