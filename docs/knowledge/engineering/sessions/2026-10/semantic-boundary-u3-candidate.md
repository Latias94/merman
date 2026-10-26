---
type: "Session Handoff"
title: "Semantic boundary U3 candidate and corrected baseline"
description: "U3 implementation and correctness are committed; corrected adjacent performance evidence remains pending."
timestamp: 2026-10-20T11:57:35Z
record_id: "e000255450894f6c93a6707c14e27295"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "358b2f2c892c74fde6117e4a2f2b5b363789396b"
supersedes: "b2fc559d264546a8a259663b389c33e5"
---

# Summary

The approved U1-U9 goal remains active. U1, U2 and U7 are committed. U3 code and
correctness are committed, but allocation/timing admission remains pending; U4 production
must wait for the adjacent U3 receipts. U5/U6, final installed products and U9 remain pending.
No push, PR, publication or release is authorized. Keep the approved plan immutable.

# Verified State

- `origin/main` was fetched again and remains `b65d2d581`; it is already an ancestor through
  merge `e8a205267`. There are no newer remote changes to merge.
- `3f0f3819a` registers parser memory lanes in the single `tools/bench/corpus.json` authority.
  The parallel parser-only corpus was deleted. Existing probe inputs did not change.
- `755ff3739` preregisters U3 allocation/peak/retained and finite-workload slope caps before
  production observer changes. Do not alter them after candidate observations.
- `93ec9218c` makes the existing strict freezer accept the current paired timeout metadata;
  a real producer integration test and actual State/Sequence typed freezes passed.
- `358b2f2c8` passes private `EditorFactsDemand::Omit/Capture` into Sequence, State, Class,
  ER and Pie construction. Ordinary typed/JSON paths skip collectors and recovery enrichment;
  requested capture retains one construction, canonical output, complete/recovered facts and spans.
  Test-only counters observe actual collector entries and item appends, not branch flags.
- Real red proof run `9c6480a7-ee74-4377-b9f5-45c7fea8748a` failed all seven demand proofs
  before production changes. The initial green run passed 49 focused tests. Expanded testing
  passed 1,980 of 1,981 tests, exposing a prior U2 Flowchart regression described below.
- Two all-feature deep projection cancellation tests passed, nextest
  `bc88df61-b4e4-401b-a044-66ef4d847f4a`. Core all-target/all-feature Clippy with
  `-D warnings` passed. Correctness and adversarial reviews returned no findings.
- Five simplification passes applied one efficiency fix (avoid formatting unused subsequent Pie
  negative-value messages) and one redundant-comment deletion. All proof tests remain intact.

The expanded fixture golden exposed U2's raw reserved-prefix guard rejecting `:::class1`.
`398c61033` corrects it using the pinned generated lexer's ASCII word boundary; `class1` and
`class_1` remain identifiers, while `class%` and `class-tail` retain their reserved-token errors.
The actual fixture's duplicate class order and source spans are asserted. No golden was updated.
The follow-up passed 288 Flowchart/Agentflow/Swimlane and complete fixture snapshot checks,
nextest `c29e69fa-204b-4066-8a51-9ae4a662528a`; nine Node oracle tests and 122 observations passed.
Release differential was repeated after correction: 726 cases, 524 accepted, 202 rejected,
zero mismatches. Its immutable local copy is under the class-prefix regression receipt directory.

# Open Threads

1. The original clean baseline worktree `F:/SourceCodes/Rust/merman-semantic-base` remains pinned
   at `3f0f3819a`; preserve its frozen executable paths and historical reports. The Flowchart
   correction invalidates its use as the final adjacent confirmation baseline.
2. The new clean baseline `F:/SourceCodes/Rust/merman-semantic-u3-base` is pinned at
   `f27ce9c595f59a1315922e38bf46c9ff0629d64f`: parent `3f0f3819a` plus only `398c61033`
   cherry-picked. It contains no U3 production changes or candidate-bound contracts.
3. A serial root-owned Python orchestration session `13678` is collecting seven full clean
   Cargo-built baseline lanes and freezing/verifying each. It started State typed at
   2026-10-20 11:57:07 UTC. Poll this session before any other Cargo work.
   Reports/logs/manifests are under `target/bench/semantic-boundary/baseline-post-u2-corrected/`.
   Targets are `target/bench/semantic-boundary/memory-u3-base-<family-route>-target` so frozen
   executable paths survive later bench resets. Ordinary checks continue to use normal `target`.
   The order is State typed, Sequence typed, State JSON, Sequence JSON, State snapshot,
   Sequence snapshot and Flowchart typed. Stop and investigate the first failed report.
4. Collect the same seven full clean candidate reports after the baseline succeeds. No dirty or
   `--executable` report can admit a candidate. Record exact reductions and unchanged controls.
5. Same-contract public timing controls are registered in the ignored experiment ledger:
   `generic_v2`, confirmation, long preset, eight A/A pairs, maximum 32 AB/BA pairs, 10% AND
   50 microseconds budget, 95% confidence, seed zero and 10,000 bootstraps. Fixtures are
   Sequence/State/Class/ER/Pie/Flowchart medium, separately for warm typed, cold-engine typed,
   known-type JSON and E2E. Use existing shared-target discovery, pin its SHA and reuse executable
   discovery for independent fresh group schedules. Do not reuse timing samples or claim speedup.
6. Only after U3 admission, preregister the adjacent U4 transfer experiment and implement it.
   The read-only U4 proof proposal favors nonempty String/Vec payload buffer identities in private
   StateDb tests: current clones are red while source buffers are alive; moves are green. Do not
   require identical outer Vec storage when element types change. Preserve required semantic graph,
   first root document, notes, links order/One-Many, styles and deep ownership/accounting.

# Next Action

Poll session `13678`; no parallel Cargo. Commit this memory shard, leaving the plan and older
shards unchanged. Update the ignored ledger and a successor receipt after verified outcomes.
Do not mark U3 or the goal complete just because implementation/tests are committed.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`
- `docs/performance/semantic_editor_demand_preregistration_2026-10-20.md`
- `docs/knowledge/engineering/subagents/2026-10/semantic-boundary-seams.md`
- `docs/knowledge/engineering/subagents/2026-10/semantic-config-reads.md`
- `target/bench/experiments/semantic-capability-boundaries/experiment.yaml`
- `target/bench/semantic-boundary/u3-green-facts.log`, `u3-core-contracts.log`,
  `u3-deep-cancellation.log`, `u3-core-clippy.log`
- `target/bench/semantic-boundary/u2-oracle/class-prefix-regression/checks.json`
- `target/bench/semantic-boundary/u2-oracle/class-prefix-regression/differential-confirmation/receipt.json`
