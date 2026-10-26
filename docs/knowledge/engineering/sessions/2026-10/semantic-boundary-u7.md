---
type: "Session Handoff"
title: "Semantic boundary U7 checkpoint"
description: "Session Handoff for Semantic boundary U7 checkpoint."
timestamp: 2026-10-20T09:22:54Z
record_id: "59ef6f6f4c4948119d7af55e4e7e7cd2"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "e8a205267"
---

# Summary

The approved U1–U9 refactor remains active on `refactor/semantic-capability-boundaries`.
U1 is committed. U7 is implemented and undergoing final validation; U2–U6 and U9
are not complete. Do not mark the goal complete or publish this branch.

# Verified State

- Plan: `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`.
  Preserve its settled design; record execution in receipts and successor memory shards.
- Local commits: `d0c19b8ef` approved plan; `8628c554a` U1 observation harness;
  `ef27b57b7` unique consumer family catalog; `e8a205267` merges `origin/main`
  at `b65d2d581` (PR #225), without conflicts or loss of the dirty U7 work.
- U1 frozen evidence: `target/bench/semantic-boundary/u1-frozen/`,
  `baseline-memory/`, and `size-base-v2/receipt.json`. The historical equivalent
  Flowchart/Sequence/State stripped consumer is 2,850,304 bytes. PR #225 changes
  the attribution baseline: freeze `e8a205267` before later comparisons and retain
  the old evidence as history, not a causal baseline for the merged implementation.
- PR #225 preservation obligations: shared compiled-theme initialization, pristine
  override/provenance checks, assignment epoch ordering and lazy error precedence;
  source-layer empty sharing must not erase public metadata provenance. Its new
  joint non-regression protocol is not an improvement protocol; U3/U4 use generic_v2.
- U7 removes implicit math/complete aliases from generic facade, Rustdoc, CLI and
  six generic SVG/Web artifact profiles. Official CLI release, C ABI and explicit
  SDK/exhaustive recipes retain math. Generated capability, binding and scoped
  legal projections have been regenerated. No ABI/schema version changes.
- Post-merge performance infrastructure: 154 tests passed. U7 Python contracts:
  103 passed with one platform/tool skip; legal generation and 424 legal projections
  passed. Web full WASM rebuilt, TypeScript contracts passed (42 exports, 55 runtime
  bindings, five entries), and 36 prepared/mount/descriptor/legal/closure tests passed.
- New CLI ambient-union regression failed before the fix: bindings-core math made
  a CLI without its own math feature render formulas. `renderer_from_config` now
  disables that backend for such a CLI. The same Cargo union then passed all nine
  `svg_smoke` tests (nextest run `a3c592bf-d58d-4d93-be83-4c67369f8bd5`).
- The U7 simplification pass found only one duplicate expected feature set and
  redundant manifest comments; these were simplified. Correctness/adversarial
  review findings concerning cfg-gated Ratex, CI binary target and ambient math
  were repaired. Remaining Rust/package checks still need to run.

# Open Threads

- U7 needs xtask matrix/artifact checks, Rustdoc and explicit positive math nextest,
  exact CLI process matrix, remaining Web WASM packages/assembly/installed smoke,
  final generated/legal checks and a focused local commit.
- U2 oracle investigation is delegated to `/root/u2_oracle_scope` (read only).
- U3 facts-demand and U4 State ownership require separate adjacent allocation and
  exact removed-work evidence before U5/U6. No production work for them has landed.
- U5/U6 must split lean semantic/full presentation APIs across all supported families
  and real consumers, preserving current prepared-source lifecycle and native geometry.

# Next Action

Finish U7 checks with serial Cargo and `-j 1`, commit only intended files, then
implement U2 using the pinned differential oracle before U3. Reuse target; never
reset/restore/stash/clean unrelated work. No push, PR, release or publication.

# Citations

- `docs/adr/0097-explicit-math-capability.md`
- `docs/performance/compiled_theme_initialization_macos_2026-10-20.md`
- `target/bench/experiments/semantic-capability-boundaries/experiment.yaml`
- `capabilities/artifact-profiles-v2.json`
