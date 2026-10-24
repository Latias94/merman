---
type: Work Progress
title: Separate diagnostic recovery capture and retain canonical Flowchart CSS
description: Request recovery diagnostics without ordinary rich editor observations and share authored subgraph style provenance with terminal consumers.
timestamp: 2026-10-21T23:06:45Z
record_id: 392bd5f8-1f3b-423c-9108-5ab5f81fd406
producer_id: codex-root
run_id: semantic-capability-boundaries-u6
related_plan: docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md
git_branch: refactor/semantic-capability-boundaries
git_commit: c0b824993
---

# Recovery Capture

The private family demand now distinguishes omission, recovery, and rich capture.
`SemanticParser::capture_diagram_recovery_snapshot_controlled_sync` uses the
existing snapshot result without adding a public demand enum. Default diagnostics
analysis, including its cancellable entry point, requests recovery; editor and
generation requests retain rich capture. Explicit custom Engine analysis remains
on its full compatibility adapter.

On ordinary exact-mapped inputs, family observers skip rich symbol, directive,
and expected-syntax construction before materializing their arguments. Required
validation, parser warnings, recovery diagnostics, source-config evidence, and
one family construction remain. Counter tests cover the removed observations;
the admitted formal corpus compares recovery and rich diagnostics, completeness,
family semantics, and config evidence while retaining the previous full oracle.

Preprocessing edits require a bounded exception. For example, removing an inline
directive from `A%%{wrap}%%B` can make an editor span cross a source-map gap. The
existing rich path diagnoses dropped spans. A conservative source-map predicate
therefore requests the rich observer during that same construction when a map can
drop spans, then remaps and clears navigation facts. This preserves the diagnostic
without reparsing. No allocation or latency improvement is claimed for this
exception, or for the operation as a whole.

Document diagnostics now retain a candidate batch per diagram before projection.
This preserves primary-error ownership across multiple invalid Markdown fences.
Tests compare the complete payload from rich projection, ordinary diagnostics,
and cancellable diagnostics. Cancellation and fresh-token recovery pass; the
fixed-checkpoint test does not prove cancellation specifically during the second
fence or final projection phase.

# Authored Flowchart Styles

Canonical Flowchart models now own subgraph-vertex CSS provenance. The full render
context shares that owner; ASCII style resolution reads it directly. The fact is
not serialized into the existing public model JSON. Empty graphs do not allocate
its optional shared map, and capture is limited to IDs that name canonical groups.

The existing declaration-ordinal ownership and replacement semantics remain:
vertex CSS replaces the group source rather than merging it, and class assignment
before a vertex exists differs from assignment afterward. Detached models retain
their group fallback and explicit overlay behavior. Resource checks charge the
canonical strings once, with exact and N-minus-one controls; the full context no
longer charges a duplicate CSS owner.

# Verification

All Cargo operations used at most two build jobs and the shared external
`D:/merman-u5-temp/verification-target`, with incremental compilation disabled.
No worktree or additional repository target was created.

| Selection | Result | Nextest run ID |
| --- | --- | --- |
| core, analysis, ASCII, editor, LSP; all features; lib and tests | 4,467 passed, zero skipped | `22991b99-988e-4532-bf3b-952d57d9bdce` |
| core, analysis, editor, LSP; all families and editor facts, without presentation/compat JSON | 1,306 passed, zero skipped | `4e7ee637-34d6-4a68-80d1-b606a7dc644b` |
| core; all families, without editor facts or presentation | 475 passed, zero skipped | `d4f54528-9bb2-415d-8137-fdafd9eac15a` |
| renderer lib plus seven selected Flowchart/AgentFlow/typed API test binaries | 3,457 passed, one existing manual benchmark skipped | `ff4ef5c0-a2d1-452a-9b03-1a64e7c1fed4` |
| Journey, two Gantt cases, and Class note-index test after lint-only initializer cleanup | Four passed, 3,665 filtered out | `d258a1f4-18b8-4807-96d2-a7d75d228963` |

Strict Clippy passed for all targets of the five full-feature core/analysis/ASCII/
editor/LSP packages with `--keep-going -- -D warnings`. Workspace format and
`git diff --check` passed. Two independent read-only recovery reviews and the
Flowchart owner review found no correctness defects. The test review recorded
the cancellation-phase limitation above.

Feature-specific warning cleanup is still open: the all-family editor-without-
presentation library emits ten production warnings, and the standalone renderer
selection without editor facts emits eighty core warnings. These configurations
pass their tests but are not claimed to pass strict Clippy. No broad allowances
or reduced test selection hide those warnings. Full-feature Clippy also exposed
three existing test-only initializer issues and a needless temporary vector;
their behavior-preserving simplifications were checked by the four focused tests.

Logs are retained externally under `D:/merman-u5-temp/u6-recovery-*`. The skipped
renderer test is `unsupported_palette_discovery_workload_benchmark`, already
marked as a manual benchmark; no new ignore was added.

# Continuation

`origin/main` through `71da4cf6fbbf4fcf27fb0465cc9169f6fd6ebd8b` remains an ancestor
of this branch. The remote update is already included and does not change this
unit's implementation plan. Nothing was pushed or published.

U6 remains open: complete ASCII family views and overflow fallback, facade and
prepared-source ownership, and host consumers still need migration. U8 naming
and generated/package alignment and U9 final gates, allocation evidence, and the
final same-contract stripped-size comparison remain required. The independent
three-family probe migration has separate runtime-output evidence and is not a
final artifact-size measurement.
