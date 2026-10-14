---
type: Session Handoff
title: Native theme regression implementation
description: Verified diagnosis and reviewed plan; first three repair units active
timestamp: 2026-10-13T17:17:44Z
record_id: 67b50404a5884fe1a8bf338e9b5af55e
producer_id: codex-root
run_id: native-theme-regression-20261014
related_plan: docs/plans/2026-10-14-0053-refactor-native-theme-regressions-plan.md
git_branch: refactor/theme-ownership-performance
git_commit: bf2af00a0b23d34a801ebe022b603f290b34eeef
---

# Summary

The maintainer requested exhaustive subagent root-cause and changed-output diagnosis, then an executable fearless-refactor repair plan and continuation. Planning checkpoint `bf2af00a0` is committed; production repairs are active and incomplete.

# Verified State

- Native benchmark fixed main `3a4b8c393939b69372a8f05a2b971f7cdf6ae529` versus theme `f47abded02b540d7fe563af7bc75f2fa1bb90c51`: nine regressions independently confirmed after stable A/A, nineteen same-output discovery rows, nineteen output-different coverage rows.
- Seven investigation agents plus family-integrity cross-review classify all nineteen changed outputs; public native probes and pinned Mermaid/Chromium establish 25 high-confidence inventory items.
- The whole-family font probe runs 152 Safe/A&B × pipeline cases: Safe all succeed; A&B causes XML errors in 28 fixture variants, in both parity and resvg-safe.
- Four fresh document reviewers return clean results; no external coding tools are used.
- U1 core six tests and one facade regression observed failing before production changes. Initial broadened green has 178 pass and one obsolete Sequence source-color expectation to reconcile with pinned source; eight tests unrun due to fail-fast.
- U2 host/source XML test observed failing before production changes. Whole-family facade assertion initially needed comma-whitespace correction; do not claim that assertion failure itself proves XML behavior.
- U3 Block six focused tests observed failing as expected; production geometry repair is now authorized.

# Open Threads

Three fresh workers own U1 core selection, U2 terminal CSS XML and U3 Block/Swimlane repairs. Root alone runs Cargo serially. Shared CSS files must explicitly hand off from U2 to U3; Block render is exclusive to U3. U4–U9 remain unimplemented. Do not ship or claim all fixes complete.

The engineering-memory whole-bundle validator still fails on historical missing frontmatter/legacy records. The new registration is valid; leave old records untouched rather than expanding this task to migrate history.

# Next Action

Finish U1 green with source-backed Sequence expectation, integrate exact owned paths after passing tests. Complete U2 and U3 red/green verification and coordinate shared file handoff. Then implement remaining inventory units, independently review the final diff, verify full relevant gates and renew calibrated native performance comparisons. Preserve original raw timing receipts and do not weaken resource/cancellation/admission or Class terminal identity checks.

# Citations

- [Plan](../../../plans/2026-10-14-0053-refactor-native-theme-regressions-plan.md)
- [Root-cause checkpoint](../../../performance/native_theme_root_cause_2026-10-14.md)
- [Runtime](../../../workstreams/native-theme-regression-20261014/WORKSTREAM.json)
- [Inventory](../../../workstreams/native-theme-regression-20261014/issues.json)
- Ignored evidence: `target/bench/experiments/native-main-comparison-20261013/analysis/implementation`.
