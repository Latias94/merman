---
type: "Verification Evidence"
title: "U3 E2E retains inconclusive State calibration"
description: "U3 is terminal with 23 confirmed controls and one inconclusive State E2E; U4 discovery continues without retries or protocol changes."
timestamp: 2026-10-22T21:51:57Z
record_id: "8bd4403fbd0b4067a7ed17c40c03ffbe"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "9408f8bb2"
supersedes: "88ab20a946c5455e9b711910da3bff37"
---

# Verification

The U3 end_to_end group completed its unchanged registered schedule. All six fixtures retain
identical input/output identities and both runners completed eight A/A pairs. Five fixtures
were stable and completed ten fresh balanced AB/BA pairs, selected by the existing suite-wide
calibration maximum. The required counts for those rows are eight; State's is ten. Source,
fixture, discovery, executable and preflight/postflight identities and divisor1 were checked.
Protocol/caps/thresholds remain generic_v2/long,30samples,2s/3s,10%AND50,000ns,95%,seed0,
10,000resamples,max32. No remeasurement or retry occurred.

# Result

The command exited3: five confirmed_non_regression, one inconclusive, zero contract failures.
State head A/A failed identity_log_within_margin, identity_absolute_within_margin and
order_absolute_within_margin; base was stable. Head identity interval is -93.956..+12.038us,
log-ratio -0.113472..+0.014686, and order absolute interval -78.460..+37.477us. State retains
the exact49,446-byteSVG identity but no confirmation pairs, admitted means or effect bounds.
This is neither proven regression nor proven non-regression.

U3 is terminal with23confirmed controls and1inconclusive. Its performance acceptance remains
unproven, and48U4/integrated rows remain unexecuted. All five E2E mirrored improvement results
are confirmed_non_improvement; State has no improvement result. No speedup/whole-plan claim.
Do not discard the State row or rerun until green. Parent progress comments initially estimated
eight confirmation pairs; actual schedule is ten, as recorded by the unchanged comparator.

# Evidence

Final root:
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`.
Receipt u3-e2e-confirmation-audit.json SHA-256 is
`05e80b089396f465fe68fcc0c112d6bd7a2c6c5063b97d471e334479302cdf3c`.
Raw JSON/Markdown/log remain in timing-u3/end_to_end. The audit retains failed calibration
checks and all six row classifications. No causal attribution for host noise is established.

# Follow-up

Continue session48174/wrapper52072. E2E comparator74516 is terminal; U4 discovery comparator
18208 is live with rustup child74252 at2026-10-23T05:51:57+08:00. It performs the preregistered
shared-target bench-profile reset/build/freeze, jobs1. Do not restart on observation timeout.
Session98819/waiter61316 remains queued for seven exact-production Clippy selections and
existing owner suites after the full timing wrapper terminates. No other Cargo queue is active.

Seven private cfg files plus Treemap snapshot regression remain uncommitted and runtime-unverified;
format/diff checks pass. After exact lint/tests pass and source commit, run the registered narrow
semantic-parser final candidate refresh with original f036probe/corpus/toolchain and frozen U1
baseline. Retain full product f036receipts and source applicability; no661matrix/all-host rerun.
Current source/reviewer/runner details remain in prior record88ab20a946c5455e9b711910da3bff37.
Goal active; no push/PR/merge/publication/release. Preserve user's untracked xtask fixture.
No subagent is open. Inconclusive performance remains an unmet condition after other work ends.

# Citations

- `docs/reviews/semantic-capability-boundaries-verification.md`.
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`.
- Final root: timing-steps.json, u3-e2e-confirmation-audit.json, production-clippy-registration.json,
  final-lean-refresh-registration.json and run_timing.py.
