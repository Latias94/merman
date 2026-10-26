---
type: "Work Progress"
title: "Lean definition gates pending serial verification"
description: "Seven private definition gates are tightened from actual warning/callsite evidence; U3 discovery passes and timing continues before exact lint and owner tests."
timestamp: 2026-10-22T20:01:07Z
record_id: "4245194f2c874d0585799b3b8ad87a31"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "323983b47"
supersedes: "31e097ad62fc4cc586eea7fc7177390c"
---

# Summary

The complete U1-U9 objective remains active; no push, PR, merge or publication. Product evidence
at f0365e6a4 remains separate from the uncommitted definition-gate cleanup described here.

# Details

Actual final-source semantic-size and CLI/LSP build logs expose private definitions with no lean
consumers. Seven files now contain a bounded cfg-only patch:

- Event Modeling and Ishikawa common-field render sanitizers, Treemap's strict render helper,
  and builtin render-identity helper compile only with presentation. Semantic sanitization
  stays at its existing owner and callsites.
- Treemap's intermediate editor-fact field and initializer compile only for compatibility JSON
  plus editor facts, matching the sole combined reader. Semantic capture extracts requested
  facts before this conversion; no successful grammar is replayed.
- The numeric compatibility helper/import compile for tests, presentation or Usecase's canonical
  JSON number normalization. Managed mutable JSON serves presentation and canonical ZenUML
  serialization. Parser position offset compiles for presentation or canonical Agentflow's
  diagnostic remapping.

Worker `01a1652d-8651-7542-8eba-af3f1ad7274f` owned only the four family/catalog files and is
closed. The parent owned JSON/source-map files. No agent ran Cargo. Existing full-feature
behavior is unchanged, and no lint allowance or public abstraction was added. Format and diff
checks pass; tests and exact production lint have not run. Do not commit or admit the code yet.

Read-only reviewer `01a16535-4439-71d2-9c58-03027f170139` found that the first mutable-JSON
gate omitted canonical ZenUML serialization. The parent corrected it to include diagram-zenuml
before verification and added ZenUML to the existing owner-test filter. Other new gates had no
findings. The reviewer is closed; no Cargo was run by either agent.

External final evidence root is
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`.
It retains `lean-definition-gates-unverified.patch` and
`lean-definition-gates-preverification.json`. Historical U6 owner logs were copied from D: to
`historical-owner-logs/` for warning attribution; they are not new final-source passes.

U3 clean discovery completed: six comparable fixtures, no contract errors, equivalent output
identities, independently built and frozen executables. Base SHA-256 is
`50cef60753cb7fc4e8dcf6006cbca2f0167cc07449a75cf5d5b63eed18d99b03`;
head is `1e0216cc1a9599be57f1a00cd21dc9d1cb1158103ee8c0224fff33f491f42ed8`.
The discovery report SHA is pinned in `timing-u3/confirmation-registration.json`.
`u3-discovery-audit.json` is a discovery-only result, not timing admission.

# Next Action

Session 48174 / wrapper PID 52072 runs the unchanged timing schedule. U3 warm `parse`
subsequently completed: all six rows confirm non-regression, with eight fresh balanced pairs
after stable A/A, zero contract failures and zero inconclusive rows. All mirrored improvement
results are confirmed_non_improvement; no speedup or whole-plan claim. The complete row/pair,
output-identity, postflight and protocol audit is `u3-parse-confirmation-audit.json`, SHA-256
`adf9790743e74da5328491af6427fe94c21437c45bf3d775f0864ce59ab0fcba`. The other 66 rows remain
pending. The wrapper has advanced to U3 parse_cold_engine base A/A; re-inspect the current
comparator child instead of assuming the completed warm comparator PID58164 is still live.
Do not restart. Sampling uses the historical clean worktrees, unaffected by current uncommitted
source. Shared target remains the isolated timing target, one Cargo job.

Session 98819 / waiter PID 61316 waits for wrapper PID 52072, then runs exact production Clippy
and existing cleanup owner tests, including the actual LSP stdio release binary in the existing
language-tool selection. The runner registration was amended before execution to
record the actual source patch hash in addition to HEAD. It then runs format plus lean,
editor-facts and full compat/editor tests for Event Modeling, Ishikawa, Treemap, JSON and source
mapping, including the existing full projection contract. No separate Cargo queue is active.
Inspect every recorded exit even if a wrapper returns zero. Preserve failures and classify every
registered timing row; no protocol/cap/fixture/threshold changes are authorized.

If cleanup changes final-source artifact bytes or consumed behavior, retain earlier receipts at
their measured source and refresh affected final controls after focused verification. Existing
normalization, prepared lifetime, transport schema and official archive claims must not silently
transfer across a behavior change. Goal completion remains unproven.

# Citations

- `docs/reviews/semantic-capability-boundaries-verification.md`
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`
- Final evidence `u3-discovery-audit.json`, `lean-definition-gates-preverification.json`,
  `production-clippy-registration.json`, `run_production_clippy.py`, `verify_gate_cleanup.py`
  and `historical-owner-log-retention.json`.
