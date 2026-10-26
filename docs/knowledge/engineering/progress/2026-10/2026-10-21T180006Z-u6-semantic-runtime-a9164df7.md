---
type: Work Progress
title: Give semantic parsing an explicit operation runtime
description: Freeze semantic parse runtime authority and restore ambient contexts across all outcomes.
timestamp: 2026-10-21T18:00:06Z
record_id: a9164df70c5841a59cd154765abbe026
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: 721af5ba4
---

# Problem And Contract

The semantic facade previously inherited any thread-local runtime operation.
An enclosing operation could therefore change generated GitGraph identities or
Gantt dates even though the semantic parser selected no such policy.

The proof-first public-facade test reproduced this before the production fix:
default IDs `0-93ba65d, 1-f55760d` became
`0-abd9973, 1-0749751` under an ambient seed. RED nextest run
`348418f3-991a-4aa5-9f42-e38b77c5e9dc` failed the actual result assertion.
The initial test-authoring compile typo was corrected before this semantic RED;
it is not the behavior-change evidence.

`SemanticParser` now owns a deterministic-default `RuntimePolicy`. Its
`with_runtime_policy`, `with_operation_context`, `runtime_policy` and
`begin_operation` methods follow the existing full engine's contracts. Auto and
known-type controlled parsing share one private input operation: first check
cancellation, then materialize runtime authority, then preprocess/detect/construct
inside the existing restoring RAII context. No second parse, new runtime adapter,
global cache or generic request framework is introduced.

This preserves pre-cancel before runtime failure and runtime failure before
preprocessing/detection. Captured-context replay preserves frozen values.
The existing context guard restores an enclosing operation on success, parse
failure, cooperative cancellation and unwind; the established panic test remains
unchanged.

# Verification

Existing tests remain intact. New public-facade coverage exercises both auto and
known-type paths for independent/replayed random seeds, Gantt missing-year dates,
missing-reference fallback, selected timezone, exact invalid-token spans,
re-entry after errors/cancellation and enclosing-context restoration. Gantt
construction counters remain one per request. The full-feature seed test compares
the semantic result with the existing full engine. A private runtime test uses
the actual unavailable clock adapter to distinguish pre-cancel/runtime/detection
precedence without a production test hook.

All Cargo commands ran serially with two build jobs and two test threads in the
existing F-drive checkout and shared target.

| Recipe | Result | Nextest run |
| --- | --- | --- |
| Original ambient regression after production fix | 1 passed | `3cb522a1-b730-4137-9bed-91da913456f6` |
| Core `--no-default-features --features all-diagrams --lib` | 483 passed | `8e728fa4-a02e-43a7-8022-5615266645b7` |
| Core `--all-features --lib --test parse_projection_contract --test deep_lifecycle --test editor_semantic_facts` | 2,183 passed | `f7820b9c-2944-4757-8d03-8f2b7d2e4480` |
| Core with no features, runtime precedence test | 1 passed, 232 filtered | `5bb10cca-773b-483b-9b23-5b1791063915` |

All selected tests passed without skips. Production
`cargo clippy -p merman-core --all-features --lib --jobs 2 -- -D warnings`,
`cargo fmt --all -- --check` and `git diff --check` passed. Lean recipes retain
existing feature-narrow warnings; they are not claimed as strict lint passes.
Logs are `D:/merman-u5-temp/semantic-runtime-*.log`.

Independent read-only correctness review reported no findings after checking
both entry points, error/cancel precedence, the single family construction,
source remapping, restored nested contexts and public API parity. The reviewer
ran no Cargo and wrote no files; verification above is root-owned.

# Continuity

This is the runtime prerequisite for canonical editor snapshots and actual lean
consumer migration. It does not complete U5 editor-facts isolation or U6 consumer
migration. U8 products/generated/legal migration and U9 integrated final-byte
and removed-work evidence remain required. No performance improvement is claimed.

Remote main remains included at `3119c614bf8c43d49cfe17ded0a53ec2e7fa983c`.
The goal is active. No push, PR, publication, release or new worktree occurred.
