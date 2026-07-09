---
type: "Work Registration"
title: "Mermaid 11.16 parity alignment"
description: "Registration for Mermaid 11.16 parity alignment."
timestamp: 2026-07-09T11:13:56Z
status: "active"
last_seen: 2026-07-09T11:55:18Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
git_commit: "53e7bf364e971e047c849f9cb87b588913bda4b3"
latest_link: "U2 detector/admission/capability slice verified; Mermaid 11.16 peeled source commit: 7c0cafcf42e76bfaf79d0cbbd12edb986612f014"
---

# Scope

Align Merman from mermaid@11.15.0 to mermaid@11.16.0 as a headless, source-backed Mermaid implementation. Scope includes baseline metadata, configuration and theme semantics, existing diagram family deltas, new 11.16 family admission, fixture/baseline refresh, SVG DOM parity, and cleanup of obsolete 11.15-only code.

# Current Claim

U1 baseline authority is committed. U2 detector/admission/capability work is verified locally: 11.16 detector ids are visible for swimlane, railroad variants, wardley, and cynefin; detector-only families are explicit capability records; admission marks them pinned-baseline `NotAdmitted`, not absent. Mermaid `mermaid@11.16.0` is an annotated tag: tag object `5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`, peeled source commit `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

# Latest Links

- Plan: `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
- Mermaid 11.16 tag object: `5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`
- Mermaid 11.16 peeled source commit: `7c0cafcf42e76bfaf79d0cbbd12edb986612f014`
- Previous baseline commit: 41646dfd43ac83f001b03c70605feb036afae46d

# Handoff

Continue with U3. Parser design must preserve LSP/editor value: source spans, recoverable diagnostics, partial AST/facts, and completion-friendly statement context. Swimlane should be treated as a Flowchart layout variant unless source evidence says otherwise; evaluate LALRPOP mainly for railroad dialects and only if span/recovery quality beats the current hand-written style.

# Citations

- `repo-ref/mermaid` tag `mermaid@11.16.0`
- `repo-ref/mermaid` tag `mermaid@11.15.0`
- `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
