---
type: "Verification Evidence"
title: "U4 warm parse controls confirmed"
description: "Six adjacent U4 warm controls confirm non-regression; U3 State E2E remains inconclusive and 42 rows await terminal classification."
timestamp: 2026-10-22T22:40:17Z
record_id: "2b43a9d1552740f9a1d35a382343bdba"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "2866ee7ae"
supersedes: "8bd4403fbd0b4067a7ed17c40c03ffbe"
---

# Verification

U4 discovery rebuilt and froze the clean f676075658a86769fd2d55fdc1fbc42803c7707c to
78c0625d14ebfa447d2320d543ca08744bd86ab2 adjacent pair. Six fixtures retain identical input and
consumed output identities; shared pipeline, JSON identity helper, corpus, preflight contract
and lockfile match. Base executable SHA-256 is
324230b8e61ca3031a955bd7248f08ca04d3b295d32fd0d04fa445b909926d80; head is
8a314bf1b1ba09f08a7b21e680b69d98ccabb28ddb9471f85a4c6c7288db66f3.

Warm full parse then completed stable eight-pair A/A on both sides and eight fresh balanced
AB/BA pairs per fixture under unchanged generic_v2/long,30samples,2s/3s,10%AND50,000ns,95%,
seed0,10,000resamples,max32. The parent checked every pair/order, divisor1, exact output and
preflight/postflight, source/fixture/discovery/frozen executable retention and post-sampling
cleanliness. No timing protocol or cap changed.

# Result

Six confirmed_non_regression, zero contract failures/inconclusive rows, exit0. All mirrored
improvement outcomes are confirmed_non_improvement. Five means are slightly larger and remain
in the report; no speedup is admitted. U4 still has18cold/JSON/E2E controls without terminal
classification; integrated has24. Total now29confirmed,1inconclusive U3StateE2E,42pending.
U3's exit3 and State failed A/A remain unchanged; whole-plan performance is unproven.

# Evidence

Final root:
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`.
u4-discovery-audit.json SHA-256 is
b99da2da58cea3f0ea291a982e50e593a7e9d68044a42d6bda3ee8a3273f03c5.
u4-parse-confirmation-audit.json SHA-256 is
103c0382f02e0f0d3bce3095f839d3dc93c6f50f5bb4a81af7f793f2c26d72d4.
Raw reports/logs, confirmation registration and copied executables remain under timing-u4.

# Follow-up

Continue existing session48174/wrapper52072. Warm comparator11336 is terminal; current cold
comparator59344 is live at2026-10-23T06:38:57+08:00 and starts A/A. Session98819/waiter61316
still queues seven exact production Clippy selections and existing lean/editor/full owner
suites after timing. Inspect every per-step exit; wrappers can continue failures.

Seven private cfg files plus Treemap public snapshot test remain uncommitted/runtime-unverified.
Format/diff pass. Source and queued scripts unchanged. Pending-cfg-source-applicability.json
confirms only those eight Rust paths plus known committed examples README prose differ from
f036 product; manifest, lock, capabilities and ABI owner bytes are unchanged. Do not turn old
artifact passes into passes on this pending patch. After lint/tests and source commit, run the
registered exact three-family candidate size/output/closure refresh with frozen U1 baseline.
Full product evidence retains measured f036 identities and scoped applicability; no661matrix
or all-host rerun. Preserve user's untracked xtask fixture. No agent is open.

No push, PR, merge, publication or release. Goal remains active with the State performance
condition unproven even after the remaining queue finishes.

# Citations

- `docs/reviews/semantic-capability-boundaries-verification.md`.
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`.
- Final root: timing-steps.json, u4-discovery-audit.json, u4-parse-confirmation-audit.json,
  production-clippy-registration.json, final-lean-refresh-registration.json.
