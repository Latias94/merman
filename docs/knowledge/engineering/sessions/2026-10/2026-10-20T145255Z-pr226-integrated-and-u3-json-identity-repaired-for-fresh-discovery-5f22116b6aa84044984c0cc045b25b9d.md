---
type: "Session Handoff"
title: "PR226 integrated and U3 JSON identity repaired for fresh discovery"
description: "Main merged and focused tests pass; deterministic harness regression fixed; immutable adjacent timing must restart with matched sources."
timestamp: 2026-10-20T14:52:55Z
record_id: "5f22116b6aa84044984c0cc045b25b9d"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "88429c11802a0ffc108da7eebee43dd5820d943e"
source_workspace: "C:/Users/Frankorz/.codex/worktrees/3ad8/merman"
supersedes: "481f2756e0c44f6aa6bf09cacf1b75ba"
---

# Summary

The approved U1-U9 goal remains active. U1/U2/U7 are implemented. U3 production correctness
and adjacent allocations pass; required timing is not admitted. U4 production is unapplied;
U5/U6/final U8/U9 remain. No push, PR, publication or release is authorized.

The latest main request is fulfilled: e7b4aa8c5 merges a933b02c1 (PR226) without conflicts.
Its scoped appearance allocation and borrowed CSS identifier changes do not alter the approved
plan. Preserve both on the two sides of new measurements rather than attributing them to U3.

# Verified State

- Merge validation: 21 focused core/render release tests pass, run
  ea0240ed-045a-4b62-bb72-a1a3ebd0da30; all12 svg_internal_id_test tests pass, run
  feeeb7ab-57f3-45eb-80de-bb85d52b27b6. Receipt:
  target/bench/semantic-boundary/main226-integration/receipt.json.
- nextest -j controls test concurrency, not its internal Cargo build. Use CARGO_BUILD_JOBS=1
  explicitly as well. Interrupted attempts are retained, not reported as passing. Matching
  core/render feature unions avoid redundant renderer builds. Do not stop other projects.
- All14 U3 memory reports at d77d87640 versus892e00e47 are frozen and verified. Allocation
  receipt SHA-256: 7f41327069530df729cd249d05f748edf1ff444ea36e2d48f4e6744becece295,
  target/bench/semantic-boundary/u3-confirmation-memory/allocation-receipt.json.
  Fixed755ff3739 caps are unchanged. These remain valid observations of those exact revisions;
  do not rewrite their source identities or claim that they include PR226.
- Timing discovery session37517 ended with contract failure (exit2) before any timing samples.
  parse/state_medium differed in byte hash, with equal source/output lengths; the other five
  rows matched. Preserve u3-timing-d77d87640-892e00e47 and both frozen executables.
- The same frozen executable failed State pre/post identity in six repeated list-only runs.
  Actual fresh State projections then passed all eight complete ManagedSemanticJson equality
  checks but failed the old byte identity. State HashMap iteration enters preserve_order JSON
  objects, managed export preserves that order, and the old benchmark hashed those bytes.
- 88429c118 fixes only benchmark JSON identity outside timed calls. Its shared bytes helper
  uses managed clone, iterative object-key sorting, managed export/drop; arrays, every value,
  original product JSON and exact SVG bytes remain unchanged. Strict comparison remains.
  Three tests really failed before correction (b542886d-243c-4864-82b6-fdd9c02d151d), then all
  passed (8e756c96-bf09-42f7-ba47-92541998cfc6). They cover deterministic key reordering,
  changed arrays/scalars/fields, unchanged original JSON, actual repeated State parses and
  2048-container export. Scoped Clippy -D warnings and cargo fmt checks pass.
- Simplification: reuse/quality/efficiency/test-value found no code changes; removed one redundant
  helper comment. Correctness plus independent Codex adversarial review found no blocking issue.
  Receipt: target/bench/semantic-boundary/u3-state-identity-diagnosis/fix-receipt.json.
  Original corrected pipeline exact-list/discovery is still pending; do not substitute tests
  for that verification or claim a latency result.
- October session-shard validation passes. Full wiki validation still reports pre-existing
  historical records without type metadata; no legacy records or rollups were rewritten.

# Open Threads

Fresh managed baseline: use the source_workspace hint above, branch
bench/u3-post-main226-baseline, clean a7142d3a4a03a0d2fff144de2cc4972bd76fd32d.
It starts at the immutable d77 source, merges PR226 as1ed1d6a71 and backports only the harness
repair as a7142d3a4. It excludes U3 demand and streaming changes. Historical bb6b,3f0,f27
worktrees and all frozen memory targets/executables are untouched.

The confirmation driver fingerprints the main bench_source but does not include support modules
in that field. Before fresh discovery verify identical pipeline.rs AND support/json_identity.rs
bytes on both clean revisions; pin both hashes/lengths in the registration and immutable source
manifest. Preserve the same check before reuse. This is an explicit two-source control, not a
generic Rust module/provenance analyzer. BENCHMARKING.md records the limitation.

U5 must retain PR226's per-application scoped presence lookup and delayed path allocation.
Same-value explicit writes still reach full setters; do not cache detector presence or bypass
the full mutation journal. Existing configuration ownership findings remain applicable.
The ignored remaining-observer packet identifies Agentflow eager facts, Architecture trace
vectors and Flowchart editor-only payloads; facts-named C4/Venn helpers also own validation or
canonical construction and must not be skipped wholesale.

U4 proposals and supplemental semantic proofs remain only under ignored
target/bench/semantic-boundary/u4-state-transfer/. Preserve their source hashes. After U3 admission,
run first-root/nested-note/programmatic-hidden-note checks on the old owner, then the actual
buffer-transfer test-only red, apply the consuming move, and verify State/deep/foreign/clone/drop/
cancel/render/ASCII controls. Do not replace required graph/document/getData work with counters.
The existing memory v3 reports do not persist statement/state/document dimensions. U4 needs a
small actual Rust shape sidecar outside measured windows; compatibility JSON bytes are explicit
projection cost, not requested typed output. Avoid Python formulas or a second parser.

# Next Action

Commit this handoff, pin its final candidate HEAD in the ignored ledger, verify the matched
source controls and both clean revisions, and run fresh serial discovery with shared normal
target plus immutable per-side executables. Keep prior failed reports intact. Require all six
selected rows comparable with matching identity, pin the successful report SHA BEFORE samples,
then run the four fresh independent generic_v2 schedules using executable discovery reuse only.

Settings remain long preset,30samples,2s warmup,3s measurement,eight calibration pairs,max32
fresh AB/BA pairs,10percent AND50us budget,95percent simultaneous CI,seed0,10000 resamples,
Rust1.95.0,merman/pipeline/default features plus svg,jobs1. Fixtures: Sequence/State/Class/ER/
Pie/Flowchart medium. Groups: parse,parse_cold_engine,compatibility_json_parse,end_to_end.
No latency improvement claim. On exit2 investigate the contract rather than dropping State,
weakening comparison, reusing failed discovery or moving bounds. U4 waits for U3 admission.

All live phase/session IDs belong in target/bench/experiments/semantic-capability-boundaries/
experiment.yaml. Do not edit tracked inputs or rebuild frozen targets during formal runs.

# Citations

- docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md.
- docs/performance/BENCHMARKING.md and docs/performance/semantic_editor_demand_preregistration_2026-10-20.md.
- Session shard481f2756e0c44f6aa6bf09cacf1b75ba and configuration finding95f1309d69b049bc9747d99802546d64.
- target/bench/semantic-boundary/u3-state-identity-diagnosis/{red-all-nextest.log,green-nextest.log,clippy.log,fix-receipt.json}.
- target/bench/semantic-boundary/u4-state-transfer/{proposal-metadata.json,supplemental-metadata.json}.
- target/bench/semantic-boundary/u5-remaining-editor-facts/remaining-observer-owners.md.
