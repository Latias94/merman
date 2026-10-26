---
type: "Session Handoff"
title: "Corrected U3 baseline and bounded State facts ready for confirmation"
description: "Source-backed keyword correction and State capture passed; batch memory harness verified; fresh adjacent confirmation follows."
timestamp: 2026-10-20T13:14:09Z
record_id: "481f2756e0c44f6aa6bf09cacf1b75ba"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "6d0a66c37"
source_workspace: "C:/Users/Frankorz/.codex/worktrees/bb6b/merman"
supersedes: "e98727279f4b4d489e75d88190427d02"
---

# Summary

The U1-U9 goal remains active. U1/U2/U7 are implemented; U3 production and correctness checks
are complete, but its adjacent allocation/timing admission is pending. Do not start U4 production
until those receipts pass. U4-U6 and final U8/U9 remain required. No push, PR or release.

This supersedes the earlier State proposal handoff. All Cargo sessions mentioned there have ended.
The next formal measurements use a newly corrected clean baseline and the existing fixed caps.

# Verified State

- `0b6b4bf8e`: State editor facts stream directly at the existing collection/flush points.
  The private StateEditorEvent enum/vector/drain is removed. Raw comma parts, including empty
  parts, have batched terminal checks. Required lexical events, grammar and DB validation remain.
- Two protected deadline tests first failed against unchanged production with complete deferred
  work: 3,072 symbols/expected slots for repeated relations; 1,025 symbols and 1,027 expected
  slots for a single style-ID token. Real red run `c7ba1d06-b3d0-4f17-b055-f75fe943f54b` is in
  `target/bench/semantic-boundary/u3-state-capture/red.log`.
- Full core/lib, parse_projection_contract, deep_lifecycle and snapshots: **1,996/1,996 pass**,
  run `1363fa78-dfa6-4dab-97ea-c49e2c18d190`, `u3-state-capture/green-contracts.log`.
  Core all-target/all-feature Clippy with `-D warnings` passes in `u3-state-capture/clippy.log`.
  No proof assertion was weakened, deleted or renamed.
- `c8aa6ce42`: complete literal Flowchart keyword boundary correction. Earlier class/reserved
  work missed header rules, which also use Jison ASCII word boundaries. Shared helper now covers
  headers/reserved/end; the shorter flowchart rule still rejects flowchart-elk1. Direction/click
  whitespace and header separators remain unchanged.
- Header correction evidence: 700 pinned oracle observations plus 14 direct Swimlane header
  observations, nine oracle Node tests pass, and current-source release differential **726 cases,
  524 accepted, 202 rejected, zero mismatches**. Immutable correctness artifacts are under
  `target/bench/semantic-boundary/u2-oracle/header-keyword-boundaries/differential-confirmation/`.
  The oracle tooling's unsupported Swimlane decoder path is retained separately from actual
  rejection; no comparator or tooling failure was relabeled as syntax rejection.
- `93193038b`: bounded same-invocation native memory batch build. All lanes preflight through
  existing validation and must share exact recipe/source/environment. One actual Cargo build
  serves independent full fresh-process schedules and unchanged per-lane report schemas.
  Source/environment/executable/tool-version drift is rejected before reuse. A moving stable
  toolchain attribution defect was found in review and fixed after real red tests.
- Root reran both Python suites: **53/53 pass** in 87.981 seconds. Receipt:
  `target/bench/semantic-boundary/u3-native-batch-tests.log`. Tests cover seven independent
  reports/420 probes, failures/preflight/drift, and actual discover/reset-build plumbing with
  mocked subprocesses plus two producer reports independently frozen and verified. This is
  harness verification, not a claim that formal parser samples have run.

# Open Threads

The earlier seven-lane batch `13678` completed and all manifests verified. Its f27 baseline is
now historical because the later header omission changed correctness. Preserve the raw reports
and frozen executable targets; do not rewrite or admit them as the final U3 comparison.

Final corrected baseline:

- Worktree: managed baseline checkout in the `source_workspace` hint above.
- Clean source: `d77d876407f35e6b224d556747ed91c89e7180d8`.
- Parent: f27 plus only harness-only `93ec9218c`, batch harness `93193038b`, and header fix
  `c8aa6ce42`, replayed as 974c3e6ba/56dd27954/d77d87640. It excludes U3 demand and candidate caps.
- Driver, freezer, shared corpus and parse_memory probe are byte-identical between this baseline
  and current root. Existing baseline worktrees at 3f0 and f27 remain untouched.
- Candidate root contains demand `358b2f2c8`, streaming `0b6b4bf8e`, and the same header/harness
  corrections. This handoff is documentation only; pin its final committed HEAD in the ignored
  execution ledger before building. Require both trees clean.

Preregistered memory remains six scales, five matched operation/zero repeats and 10,000 bootstrap
resamples per lane. Use all seven lanes from the existing corpus: State/Sequence typed, JSON and
snapshot, plus Flowchart typed. The fixed `755ff3739` caps are unchanged. Batch flags only remove
redundant identical builds, never samples, bounds, schema or clean-build admission.

Run registered public timing controls afterward with compare_self generic_v2, long preset,
eight calibration pairs, max32 fresh AB/BA pairs, 10 percent AND50us budget,95 percent CI,
seed0 and10,000 resamples; package merman/pipeline/default features plus svg/Rust1.95.0/jobs1.
Six fixtures: Sequence/State/Class/ER/Pie/Flowchart medium. Four separately sampled groups:
parse, parse_cold_engine, compatibility_json_parse, end_to_end. Use frozen shared-target
discovery, pin its SHA before sampling, then reuse executable discovery only. No timing sample
reuse or speedup claim. All settings and live session IDs belong in the ignored execution ledger.

U4 ownership proof design is retained in prior continuity; do not add a generic clone-hook
framework. Prove real nonempty owned-buffer transfer, preserve required graph/document/JSON
work and resource/cancellation/deep-lifecycle contracts, then gather separate U3-to-U4 evidence.

# Next Action

Commit this handoff, verify clean trees, record final source IDs and start one serial full
baseline batch through the existing run_native_memory driver. Freeze and verify each report
separately. Preserve the batch target executable path. Then run the clean candidate batch with
its fixed owner contracts and equivalent toolchain/recipe, followed by registered timing controls.
Investigate failures without moving caps. U3 is admitted only after adjacent receipts pass.

Do not rebuild frozen targets or edit tracked source during a formal measurement. Other projects
may run Cargo; keep this repository's Cargo work serial and do not stop unrelated processes.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`.
- `docs/performance/semantic_editor_demand_preregistration_2026-10-20.md`.
- `docs/knowledge/engineering/subagents/2026-10/semantic-admission-functions.md`.
- `docs/knowledge/engineering/subagents/2026-10/2026-10-20T130052Z-semantic-and-presentation-configuration-ownership-95f1309d69b049bc9747d99802546d64.md`.
- `target/bench/experiments/semantic-capability-boundaries/experiment.yaml` (ignored live ledger).
