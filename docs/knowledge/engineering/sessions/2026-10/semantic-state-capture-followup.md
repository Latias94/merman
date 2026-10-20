---
type: "Session Handoff"
title: "State capture streaming proposal before U3 confirmation"
description: "Five corrected baselines are frozen; State capture proposal awaits real red and runtime verification."
timestamp: 2026-10-20T12:29:54Z
record_id: "e98727279f4b4d489e75d88190427d02"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "23912f078"
supersedes: "e000255450894f6c93a6707c14e27295"
---

# Summary

The U1-U9 goal remains active. U3 admission is pending; do not start U4 production or mark
the goal complete. This successor records a boundedness repair discovered while the corrected
baseline batch was running. The earlier U3 checkpoint remains historical context.

# Verified State

- Root batch `13678` is live and owns the only Merman Cargo build. Five full clean corrected
  baseline lanes have passed and their manifests were frozen/verified: State/Sequence typed,
  State/Sequence JSON, and State snapshot. Sequence snapshot is running; Flowchart typed follows.
  All reports remain under `target/bench/semantic-boundary/baseline-post-u2-corrected/`.
- The baseline is still immutable `f27ce9c595f59a1315922e38bf46c9ff0629d64f` in
  `F:/SourceCodes/Rust/merman-semantic-u3-base`. The original `3f0f3819a` baseline and its
  frozen executable targets remain untouched as historical evidence.
- The first four max-scale metrics exactly match the preregistration observations. No candidate
  memory or timing admission has run. All `755ff3739` allocation caps remain unchanged.
- Canonical State production remains unchanged. Two test-only additions in `operation.rs`
  target actual post-first-symbol work under fake deadlines: normal repeated relations and one
  `StyleIds` token with 1,024 comma-separated targets. They have not been compiled or run yet.

The existing State collector stages a single-use `Vec<StateEditorEvent>` and then emits all facts
without an intermediate control checkpoint. The representation has no readback, sorting or replay
consumer. The selected U3/R9 repair directly appends the same fact constructors at the original
deferred flush positions, removes the enum/vector/drain, and checks raw comma parts every 128
iterations in both ID-list paths, including empty parts. Grammar lexical events remain required.

The production proposal exists only under ignored target:

- `target/bench/semantic-boundary/u3-state-capture/parse.rs.proposed`
- `parse.rs.diff` and `proposal-metadata.json` in the same directory.
- Original canonical SHA: `816e7d69ec9612b6b39f83d85f3c32f87d137903cdfe18b9d7edd2fc27d7d008`.
- Proposed SHA: `0ae5975bc24fd45a01ad018ae5718763c37ba79aeda40aa91654b76c01a78849`.

Reuse, quality, efficiency, test-value, correctness and adversarial reviewers reported no static
findings. No added/changed comments require a comment pass. All existing and new proof tests are
protected; no assertion was weakened or rewritten. This is not build or runtime verification.

# Open Threads

1. Wait for batch `13678` to finish; do not start another Cargo while it is live. Preserve each
   target because its executable path is part of the frozen manifest. Re-poll the same session
   if an observation times out; a timeout does not establish termination.
2. Run the two new tests against unchanged production and retain genuine red output:
   `test(state_editor_capture_deadline_bounds_event_drain_and_recovers)` and
   `test(state_editor_capture_deadline_bounds_single_token_id_list)`.
   The current implementation is expected to exceed the bounded partial symbol count.
3. After red, verify the canonical/proposed SHAs before applying the proposal. If the canonical
   differs, inspect the intervening diff and preserve external edits. Format and run focused plus
   broad core/projection/snapshot/deep-lifecycle controls and all-target/all-feature Clippy.
   Fix failures without weakening any proof. Commit the verified production/tests as one focused unit.
4. Collect seven full clean candidate memory reports and verified manifests with the unchanged
   bounds. Run the registered fresh `generic_v2` warm/cold typed, known-type JSON and E2E controls
   from the earlier checkpoint and ignored ledger. Admit U3 only after its adjacent evidence passes.
5. Then perform U4 State DB ownership transfer with independent adjacent proof. U5 admission
   function research is captured separately; it does not authorize bypassing U3/U4 gates.

# Next Action

Poll `13678`, retain new formal receipts, then perform the red/apply/green sequence above.
Keep the plan immutable, Cargo serial, and stage only owned paths. No push, PR or release.

# Citations

- `docs/knowledge/engineering/sessions/2026-10/semantic-boundary-u3-candidate.md`.
- `docs/knowledge/engineering/subagents/2026-10/semantic-admission-functions.md`.
- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md` U3, R9, KTD4.
- `target/bench/experiments/semantic-capability-boundaries/experiment.yaml`.
- `crates/merman-core/src/diagrams/state/parse.rs`, `crates/merman-core/src/operation.rs`.
