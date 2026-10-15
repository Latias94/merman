---
type: "Session Handoff"
title: "Native theme correctness repair checkpoint"
description: "Session Handoff for Native theme correctness repair checkpoint."
timestamp: 2026-10-13T18:07:05Z
record_id: "9d4bb892ee2143d68977a3b7f0b31bc7"
producer_id: "codex-root"
run_id: "native-theme-regression-20261014"
---

# Summary

Continue the maintainer-authorized exhaustive native theme repair. All nineteen changed-output fixtures have source-backed classifications, the reviewed nine-unit plan is committed, and implementation is active. Do not stop at the plan or claim all problems are fixed.

# Verified State

- Main control remains `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`; audited branch `f47abded02b540d7fe563af7bc75f2fa1bb90c51` has nine confirmed regressions. Original timing receipts are unchanged. Its release executable was copied and SHA-256 verified under the original experiment's `frozen-original-head/` before any renewed release build.
- `87cf582bf` fixes U1 appearance evaluation versus ownership: 194 core and seven public facade tests pass.
- `b994d79d7` fixes new N28 pre-freeze Tracking-derived compatibility ownership: two public/owner counterexamples failed first; the corrected broad core selection passes 198/198. Only its minimal patch is committed; U5 performance characterization stays unstaged.
- U2/U3/N27 renderer selection: 201/202 pass, including all Block/Swimlane geometry, terminal encoding, cancellation and XML validation cases. The remaining new source-font test led to N30, which is still being repaired. The full typed-font matrix previously passed 152 cases; strict font admission reason passes separately.
- U4 retired metadata implementation: 216/217 family tests pass and 72/72 comparator tests pass. The one preexisting test failure is N29: its ELK golden was compared under a build without ELK. Eight independent main/audit/current/upstream backend controls prove the production geometry is unchanged. A feature guard is added; the actual ELK test and two Dagre fallback controls pass.
- N26 long CSS cancellation: two real red tests, bounded UTF-8 direct-slice output implemented and verified. N27 literal XML 1.0 validation: five real red tests, three legal controls passed before the fix; same-owner streaming character validation now passes with the broader validator suite.
- N30 legal quoted font strings containing angle brackets are discarded by a CSS/XML-confused normalization guard. Owner-local tests are red. The worker must preserve existing legitimate host `var`/`env` capability and generic inline CSS security checks while fixing the font-specific boundary.

# Open Threads

Mutable inventory now has N01–N30. U5 transaction/COW/empty-authority, U6 consumer-bound family evidence, U7 reference summaries and U8 consuming finalization remain unfinished. U5/U6/U7 workers have tests or designs ready, but performance production edits are deliberately frozen until the corrected control is committed and release-built.

Root alone runs Cargo serially with `CARGO_BUILD_JOBS=1`; workers never run Cargo or Git mutations. Shared file snapshots have been staged carefully: Class render currently contains only U2 in the index; final_validation contains only N27 using `U7-corrected-control-final_validation.rs`, while U7 red tests remain in the working tree. Never stage whole mixed files blindly or revert workers' tests.

Actual U5 characterization red: seven failures and nine controls pass. U6: four work-contract failures and two integrity/projection controls pass. U7: two work-contract failures and two effect/marker/cancellation controls pass. Keep logs; do not confuse these expected unfinished optimization tests with correctness regressions in the corrected control.

# Next Action

Finish N30 and source-font/normalization gates, commit the precise corrected renderer paths, then create/build a frozen corrected-control worktree. Release U5/U6/U7 production independently after that baseline is frozen. Implement U8 after U7 ownership handoff. Run required full SVG modes/strict checks, feature/consumer tests, formatting/lint, simplification and actual final `ce-code-review`; then renew preregistered same-input calibrated A/B and update existing PR 178. Preserve original controls and all unfavorable or inconclusive measurements.

# Citations

- Plan: `docs/plans/2026-10-14-0053-refactor-native-theme-regressions-plan.md`.
- Root causes: `docs/performance/native_theme_root_cause_2026-10-14.md`.
- Inventory/runtime: `docs/workstreams/native-theme-regression-20261014/{issues,WORKSTREAM}.json`.
- Ignored raw evidence: `target/bench/experiments/native-main-comparison-20261013/analysis/implementation/`.
- N28 committed minimal patch; U5/U6/U7 patches and corrected-owner snapshots remain in that ignored directory.
